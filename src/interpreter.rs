use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Read as IoRead, Seek, SeekFrom, Write};
use std::rc::Rc;

/// Shared output buffer that implements Write.
#[derive(Clone)]
pub struct SharedOutput(Rc<RefCell<Vec<u8>>>);

impl Default for SharedOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedOutput {
    pub fn new() -> Self {
        Self(Rc::new(RefCell::new(Vec::new())))
    }

    pub fn into_string(self) -> String {
        let bytes = self.0.borrow().clone();
        String::from_utf8(bytes).unwrap_or_default()
    }
}

impl Write for SharedOutput {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

use crate::ast::*;
use crate::builtins::BuiltinRegistry;
use crate::environment::{EnvRef, Environment};
use crate::error::RuntimeError;
use crate::value::Value;

enum ControlFlow {
    Normal,
    ExitFor,
    ExitDo,
    ExitSub,
    ExitFunction(Value),
    Goto(Label),
    Gosub(Label),
    Return,
    End,
    Retry,
    Continue,
}

struct ExceptionInfo {
    extype: i32,
    extext: String,
}

#[derive(Clone)]
struct UserSub {
    params: Vec<Param>,
    body: Vec<LabeledStmt>,
    is_static: bool,
}

#[derive(Clone)]
struct UserFunction {
    name: String,
    params: Vec<Param>,
    body: Vec<LabeledStmt>,
    is_static: bool,
}

struct FileHandle {
    _access: FileAccess,
    reader: Option<BufReader<File>>,
    writer: Option<BufWriter<File>>,
    eof_flag: bool,
}

pub struct Interpreter {
    pub dialect: crate::Dialect,
    gosub_stack: Vec<usize>,
    env: EnvRef,
    builtins: BuiltinRegistry,
    subs: HashMap<String, UserSub>,
    functions: HashMap<String, UserFunction>,
    print_col: usize,
    print_row: usize,
    screen_width: usize,
    screen_height: usize,
    current_fg: Option<u8>,
    current_bg: Option<u8>,
    data_values: Vec<DataItem>,
    data_pos: usize,
    data_label_pos: HashMap<String, usize>,
    output: Box<dyn Write>,
    input: Box<dyn BufRead>,
    file_handles: HashMap<i64, FileHandle>,
    // Random number generator state
    rng_state: u64,
    last_rnd: f64,
    static_vars: HashMap<String, HashMap<String, Value>>,
    current_static_vars: HashSet<String>,
    type_defs: HashMap<String, Vec<crate::ast::TypeField>>,
    array_type_map: HashMap<String, String>,
    source_dir: Option<std::path::PathBuf>,
    interactive: bool,
    screen_buffer: Vec<Vec<u8>>,
    current_exception: Option<ExceptionInfo>,
    last_det: f64,
    /// Tracks array dimensions from DIM: name -> vec of (lower, upper) per dimension.
    array_dim_info: HashMap<String, Vec<(i64, i64)>>,
}

impl Drop for Interpreter {
    fn drop(&mut self) {
        for (_, fh) in self.file_handles.drain() {
            if let Some(mut w) = fh.writer {
                let _ = w.flush();
            }
        }
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        let mut interp = Self::with_io(
            Box::new(io::stdout()),
            Box::new(io::BufReader::new(io::stdin())),
        );
        interp.interactive = true;
        interp
    }

    pub fn with_io(output: Box<dyn Write>, input: Box<dyn BufRead>) -> Self {
        Self {
            dialect: crate::Dialect::Ansi,
            gosub_stack: Vec::new(),
            env: Environment::new_global(),
            builtins: BuiltinRegistry::new(),
            subs: HashMap::new(),
            functions: HashMap::new(),
            print_col: 0,
            print_row: 1,
            screen_width: 80,
            screen_height: 25,
            current_fg: None,
            current_bg: None,
            data_values: Vec::new(),
            data_pos: 0,
            data_label_pos: HashMap::new(),
            output,
            input,
            file_handles: HashMap::new(),
            rng_state: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            last_rnd: 0.0,
            static_vars: HashMap::new(),
            current_static_vars: HashSet::new(),
            type_defs: HashMap::new(),
            array_type_map: HashMap::new(),
            source_dir: None,
            interactive: false,
            screen_buffer: vec![vec![b' '; 80]; 25],
            current_exception: None,
            last_det: 0.0,
            array_dim_info: HashMap::new(),
        }
    }

    pub fn run_source(&mut self, source: &str) -> Result<(), Box<dyn std::error::Error>> {
        let detected = crate::detect_dialect(source);
        if detected == crate::Dialect::QuickBasic {
            self.dialect = detected;
        }
        let tokens = crate::lexer::Lexer::with_dialect(source, self.dialect).tokenize()?;
        let mut parser = crate::parser::Parser::with_dialect(tokens, self.dialect);
        let program = parser.parse_program()?;
        self.run_program(&program)?;
        Ok(())
    }

    pub fn run_file(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = std::path::Path::new(path);
        let canonical = std::fs::canonicalize(path).map_err(|e| RuntimeError::General {
            msg: format!("Cannot open '{}': {}", path.display(), e),
        })?;
        self.source_dir = canonical.parent().map(|p| p.to_path_buf());
        let source = std::fs::read_to_string(&canonical).map_err(|e| RuntimeError::General {
            msg: format!("Cannot read '{}': {}", canonical.display(), e),
        })?;
        self.run_source(&source)
    }

    pub fn run_program(&mut self, program: &Program) -> Result<(), RuntimeError> {
        // Pre-scan: collect labels, DATA statements, SUB/FUNCTION definitions
        self.prescan(&program.statements);

        // Execute top-level statements
        self.exec_top_level(&program.statements)?;
        Ok(())
    }

    fn prescan(&mut self, stmts: &[LabeledStmt]) {
        for (i, ls) in stmts.iter().enumerate() {
            if let Some(label) = &ls.label {
                self.env.borrow_mut().register_label(label, i);
            }
            match &ls.stmt {
                Stmt::Data(items) => {
                    if let Some(label) = &ls.label {
                        self.data_label_pos
                            .insert(label.to_string(), self.data_values.len());
                    }
                    self.data_values.extend(items.clone());
                }
                Stmt::SubDef(sub) => {
                    self.subs.insert(
                        sub.name.clone(),
                        UserSub {
                            params: sub.params.clone(),
                            body: sub.body.clone(),
                            is_static: sub.is_static,
                        },
                    );
                }
                Stmt::FunctionDef(func) => {
                    self.functions.insert(
                        func.name.clone(),
                        UserFunction {
                            name: func.name.clone(),
                            params: func.params.clone(),
                            body: func.body.clone(),
                            is_static: func.is_static,
                        },
                    );
                }
                Stmt::TypeDef { name, fields } => {
                    self.type_defs.insert(name.clone(), fields.clone());
                }
                // Recurse into nested blocks to find labels and DATA
                Stmt::If(if_stmt) => {
                    self.prescan(&if_stmt.then_body);
                    for (_, body) in &if_stmt.elseif_clauses {
                        self.prescan(body);
                    }
                    if let Some(else_body) = &if_stmt.else_body {
                        self.prescan(else_body);
                    }
                }
                Stmt::For(for_stmt) => self.prescan(&for_stmt.body),
                Stmt::WhileWend { body, .. } => self.prescan(body),
                Stmt::DoLoop(do_stmt) => self.prescan(&do_stmt.body),
                Stmt::SelectCase(sel) => {
                    for case in &sel.cases {
                        self.prescan(&case.body);
                    }
                    if let Some(else_body) = &sel.else_body {
                        self.prescan(else_body);
                    }
                }
                Stmt::WhenException { body, handler } => {
                    self.prescan(body);
                    self.prescan(handler);
                }
                _ => {}
            }
        }
    }

    /// Execute a nested block of statements (inside IF, FOR, DO, WHILE, SELECT CASE, SUB, FUNCTION).
    fn exec_block(&mut self, stmts: &[LabeledStmt]) -> Result<ControlFlow, RuntimeError> {
        let mut pc = 0;
        while pc < stmts.len() {
            let ls = &stmts[pc];
            let cf = self.exec_stmt(&ls.stmt)?;
            match cf {
                ControlFlow::Normal => pc += 1,
                other => return Ok(other),
            }
        }
        Ok(ControlFlow::Normal)
    }

    /// Execute the top-level statement block with GOTO handling.
    fn exec_top_level(&mut self, stmts: &[LabeledStmt]) -> Result<ControlFlow, RuntimeError> {
        let mut pc = 0;
        while pc < stmts.len() {
            let ls = &stmts[pc];
            let cf = self.exec_stmt(&ls.stmt)?;
            match cf {
                ControlFlow::Normal => {
                    pc += 1;
                }
                ControlFlow::Goto(label) => {
                    let resolved = self.env.borrow().resolve_label(&label);
                    if let Some(idx) = resolved {
                        pc = idx;
                    } else {
                        return Ok(ControlFlow::Goto(label));
                    }
                }
                ControlFlow::Gosub(label) => {
                    let resolved = self.env.borrow().resolve_label(&label);
                    if let Some(idx) = resolved {
                        self.gosub_stack.push(pc + 1);
                        pc = idx;
                    } else {
                        return Err(RuntimeError::General {
                            msg: format!("Label not found for GOSUB: {label}"),
                        });
                    }
                }
                ControlFlow::Return => {
                    if let Some(return_pc) = self.gosub_stack.pop() {
                        pc = return_pc;
                    } else {
                        return Err(RuntimeError::General {
                            msg: "RETURN without GOSUB".into(),
                        });
                    }
                }
                other => return Ok(other),
            }
        }
        Ok(ControlFlow::Normal)
    }

    fn exec_stmt(&mut self, stmt: &Stmt) -> Result<ControlFlow, RuntimeError> {
        match stmt {
            Stmt::Print(ps) => {
                self.exec_print(ps)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::Let { var, expr } => self.exec_let(var, expr),
            Stmt::LetSlice {
                name,
                start,
                end,
                expr,
            } => {
                let start_f = self.eval_expr(start)?.to_f64()?;
                let end_f = self.eval_expr(end)?.to_f64()?;
                if start_f < 1.0 || end_f < 1.0 {
                    return Err(RuntimeError::IllegalFunctionCall {
                        msg: "string slice index must be >= 1".into(),
                    });
                }
                let replacement = self.eval_expr(expr)?.to_string_val()?;
                let mut s = self
                    .env
                    .borrow()
                    .get(name)
                    .unwrap_or(Value::Str(String::new()))
                    .to_string_val()?;
                let (start_0, end_0) = Self::string_slice_byte_range(&s, start_f, end_f);
                if start_0 <= end_0 {
                    s.replace_range(start_0..end_0, &replacement);
                }
                self.env.borrow_mut().set(name, Value::Str(s));
                Ok(ControlFlow::Normal)
            }
            Stmt::Dim(decls) => self.exec_dim(decls),
            Stmt::Const { name, value } => {
                let val = self.eval_expr(value)?;
                self.env.borrow_mut().define_const(name, val)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::Input(input) => {
                self.exec_input(input)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::LineInput { prompt, var } => {
                if let Some(p) = prompt {
                    write!(self.output, "{}", p).ok();
                    self.output.flush().ok();
                }
                let mut line = String::new();
                self.input.read_line(&mut line).ok();
                let line = line
                    .trim_end_matches('\n')
                    .trim_end_matches('\r')
                    .to_string();
                self.env.borrow_mut().set(&var.name, Value::Str(line));
                Ok(ControlFlow::Normal)
            }
            Stmt::If(if_stmt) => self.exec_if(if_stmt),
            Stmt::For(for_stmt) => self.exec_for(for_stmt),
            Stmt::WhileWend { condition, body } => self.exec_while(condition, body),
            Stmt::DoLoop(do_stmt) => self.exec_do(do_stmt),
            Stmt::SelectCase(select) => self.exec_select(select),
            Stmt::Goto(label) => Ok(ControlFlow::Goto(label.clone())),
            Stmt::Gosub(label) => Ok(ControlFlow::Gosub(label.clone())),
            Stmt::Return => Ok(ControlFlow::Return),
            Stmt::OnGoto {
                expr,
                labels,
                is_gosub,
            } => self.exec_on_goto(expr, labels, *is_gosub),
            Stmt::ExitFor => Ok(ControlFlow::ExitFor),
            Stmt::ExitDo => Ok(ControlFlow::ExitDo),
            Stmt::ExitSub => Ok(ControlFlow::ExitSub),
            Stmt::ExitFunction => {
                // ExitFunction doesn't need to carry a value here;
                // the caller (call_user_function) reads the function-name variable.
                Ok(ControlFlow::ExitFunction(Value::Numeric(0.0)))
            }
            Stmt::End | Stmt::System | Stmt::Stop => Ok(ControlFlow::End),
            Stmt::Rem => Ok(ControlFlow::Normal),
            Stmt::ExprStmt(expr) => {
                self.eval_expr(expr)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::SubDef(_) | Stmt::FunctionDef(_) | Stmt::Declare(_) => {
                // Already collected during prescan
                Ok(ControlFlow::Normal)
            }
            Stmt::Call { name, args } => self.exec_sub_call(name, args),
            Stmt::Swap { a, b } => {
                let va = self
                    .env
                    .borrow()
                    .get(&a.name)
                    .unwrap_or(Value::Numeric(0.0));
                let vb = self
                    .env
                    .borrow()
                    .get(&b.name)
                    .unwrap_or(Value::Numeric(0.0));
                self.env.borrow_mut().set(&a.name, vb);
                self.env.borrow_mut().set(&b.name, va);
                Ok(ControlFlow::Normal)
            }
            Stmt::Read(vars) => self.exec_read(vars),
            Stmt::Restore(label) => {
                if let Some(lbl) = label {
                    if let Some(&pos) = self.data_label_pos.get(&lbl.to_string()) {
                        self.data_pos = pos;
                    } else {
                        self.data_pos = 0;
                    }
                } else {
                    self.data_pos = 0;
                }
                Ok(ControlFlow::Normal)
            }
            Stmt::Data(_) => Ok(ControlFlow::Normal), // handled in prescan
            Stmt::OptionBase(n) => {
                self.env.borrow_mut().option_base = *n;
                Ok(ControlFlow::Normal)
            }
            Stmt::Redim { preserve, decls } => self.exec_redim(decls, *preserve),
            Stmt::Erase(names) => self.exec_erase(names),
            Stmt::Open(open) => {
                self.exec_open(open)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::Close(file_nums) => {
                self.exec_close(file_nums)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::PrintFile(pf) => {
                self.exec_file_print(pf)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::WriteFile(wf) => {
                self.exec_file_write(wf)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::InputFile(fi) => {
                self.exec_file_input(fi)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::LineInputFile { file_num, var } => {
                self.exec_line_input_file(file_num, var)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::GetPut(gp) => {
                self.exec_get_put(gp)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::Randomize(expr) => {
                if let Some(e) = expr {
                    let val = self.eval_expr(e)?;
                    self.rng_state = val.to_f64()?.to_bits();
                } else {
                    self.rng_state = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as u64;
                }
                Ok(ControlFlow::Normal)
            }

            Stmt::Write(exprs) => self.exec_write_console(exprs),

            Stmt::Sleep(expr) => {
                if let Some(e) = expr {
                    let secs = self.eval_expr(e)?.to_i64()?;
                    if secs > 0 {
                        std::thread::sleep(std::time::Duration::from_secs(secs as u64));
                    }
                }
                Ok(ControlFlow::Normal)
            }

            Stmt::Clear => {
                self.env.borrow_mut().clear_vars();
                self.data_pos = 0;
                Ok(ControlFlow::Normal)
            }

            Stmt::Name { old, new } => {
                let old_path = self.eval_expr(old)?.to_string_val()?;
                let new_path = self.eval_expr(new)?.to_string_val()?;
                std::fs::rename(&old_path, &new_path)
                    .map_err(|e| RuntimeError::from_io("NAME", e))?;
                Ok(ControlFlow::Normal)
            }

            Stmt::Kill(expr) => {
                let path = self.eval_expr(expr)?.to_string_val()?;
                std::fs::remove_file(&path).map_err(|e| RuntimeError::from_io("KILL", e))?;
                Ok(ControlFlow::Normal)
            }

            Stmt::Mkdir(expr) => {
                let path = self.eval_expr(expr)?.to_string_val()?;
                std::fs::create_dir(&path).map_err(|e| RuntimeError::from_io("MKDIR", e))?;
                Ok(ControlFlow::Normal)
            }

            Stmt::Rmdir(expr) => {
                let path = self.eval_expr(expr)?.to_string_val()?;
                std::fs::remove_dir(&path).map_err(|e| RuntimeError::from_io("RMDIR", e))?;
                Ok(ControlFlow::Normal)
            }

            Stmt::Chdir(expr) => {
                let path = self.eval_expr(expr)?.to_string_val()?;
                std::env::set_current_dir(&path).map_err(|e| RuntimeError::from_io("CHDIR", e))?;
                Ok(ControlFlow::Normal)
            }

            Stmt::Shell(expr) => {
                if let Some(e) = expr {
                    let cmd = self.eval_expr(e)?.to_string_val()?;
                    #[cfg(target_os = "windows")]
                    let result = std::process::Command::new("cmd")
                        .args(["/c", &cmd])
                        .status();
                    #[cfg(not(target_os = "windows"))]
                    let result = std::process::Command::new("sh").args(["-c", &cmd]).status();
                    result.map_err(|e| RuntimeError::General {
                        msg: format!("SHELL error: {}", e),
                    })?;
                }
                Ok(ControlFlow::Normal)
            }

            Stmt::Shared(vars) => {
                for var in vars {
                    self.env.borrow_mut().shared_vars.insert(var.name.clone());
                }
                Ok(ControlFlow::Normal)
            }

            Stmt::Static(decls) => {
                // Mark variables as static and initialize with defaults if not already loaded
                for decl in decls {
                    if self.env.borrow().get(&decl.name).is_none() {
                        let default = Value::default_for(Self::resolve_decl_type(decl));
                        self.env.borrow_mut().set(&decl.name, default);
                    }
                    self.current_static_vars.insert(decl.name.clone());
                }
                Ok(ControlFlow::Normal)
            }

            // User-defined types (collected during prescan)
            Stmt::TypeDef { .. } => Ok(ControlFlow::Normal),

            Stmt::MemberAssign { target, value } => {
                let new_val = self.eval_expr(value)?;
                self.set_member_value(target, new_val)?;
                Ok(ControlFlow::Normal)
            }

            // Console
            Stmt::Cls => {
                write!(self.output, "\x1b[2J\x1b[H").ok();
                self.print_row = 1;
                self.print_col = 0;
                for row in &mut self.screen_buffer {
                    row.fill(b' ');
                }
                Ok(ControlFlow::Normal)
            }
            Stmt::Beep => {
                write!(self.output, "\x07").ok();
                Ok(ControlFlow::Normal)
            }
            Stmt::Locate { row, col } => {
                let r = if let Some(expr) = row {
                    let v = self.eval_expr(expr)?.to_i64()?;
                    if v < 1 || v > self.screen_height as i64 {
                        return Err(RuntimeError::IllegalFunctionCall {
                            msg: format!("LOCATE row {} out of range", v),
                        });
                    }
                    v as usize
                } else {
                    self.print_row
                };
                let c = if let Some(expr) = col {
                    let v = self.eval_expr(expr)?.to_i64()?;
                    if v < 1 || v > self.screen_width as i64 {
                        return Err(RuntimeError::IllegalFunctionCall {
                            msg: format!("LOCATE column {} out of range", v),
                        });
                    }
                    v as usize
                } else {
                    self.print_col + 1
                };
                write!(self.output, "\x1b[{};{}H", r, c).ok();
                self.print_row = r;
                self.print_col = c - 1;
                Ok(ControlFlow::Normal)
            }
            Stmt::Color { fg, bg } => {
                let fg_val = if let Some(expr) = fg {
                    let v = self.eval_expr(expr)?.to_i64()?;
                    if !(0..=15).contains(&v) {
                        return Err(RuntimeError::IllegalFunctionCall {
                            msg: format!("COLOR foreground {} out of range", v),
                        });
                    }
                    Some(v as u8)
                } else {
                    self.current_fg
                };
                let bg_val = if let Some(expr) = bg {
                    let v = self.eval_expr(expr)?.to_i64()?;
                    if !(0..=15).contains(&v) {
                        return Err(RuntimeError::IllegalFunctionCall {
                            msg: format!("COLOR background {} out of range", v),
                        });
                    }
                    Some(v as u8)
                } else {
                    self.current_bg
                };
                self.current_fg = fg_val;
                self.current_bg = bg_val;
                let mut seq = String::from("\x1b[");
                let mut need_sep = false;
                if let Some(f) = fg_val {
                    seq.push_str(&Self::color_fg_to_ansi(f).to_string());
                    need_sep = true;
                }
                if let Some(b) = bg_val {
                    if need_sep {
                        seq.push(';');
                    }
                    seq.push_str(&Self::color_bg_to_ansi(b).to_string());
                }
                seq.push('m');
                write!(self.output, "{}", seq).ok();
                Ok(ControlFlow::Normal)
            }
            Stmt::Width { columns, rows } => {
                if let Some(expr) = columns {
                    self.screen_width = self.eval_expr(expr)?.to_i64()? as usize;
                }
                if let Some(expr) = rows {
                    self.screen_height = self.eval_expr(expr)?.to_i64()? as usize;
                }
                Ok(ControlFlow::Normal)
            }
            Stmt::ViewPrint { top, bottom } => {
                if let (Some(t), Some(b)) = (top, bottom) {
                    let t_val = self.eval_expr(t)?.to_i64()?;
                    let b_val = self.eval_expr(b)?.to_i64()?;
                    write!(self.output, "\x1b[{};{}r", t_val, b_val).ok();
                } else {
                    // Reset scroll region
                    write!(self.output, "\x1b[r").ok();
                }
                Ok(ControlFlow::Normal)
            }

            // WHEN EXCEPTION
            Stmt::WhenException { body, handler } => self.exec_when_exception(body, handler),
            Stmt::Retry => Ok(ControlFlow::Retry),
            Stmt::Continue => Ok(ControlFlow::Continue),

            Stmt::Mat(op) => self.exec_mat(op),

            Stmt::SetPointer { file_num, position } => {
                self.exec_set_pointer(file_num, position)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::AskPointer { file_num, var } => {
                self.exec_ask_pointer(file_num, var)?;
                Ok(ControlFlow::Normal)
            }
        }
    }

    fn exec_let(&mut self, var: &Variable, expr: &Expr) -> Result<ControlFlow, RuntimeError> {
        // Check for array assignment (encoded as BinaryOp::Eq with ArrayIndex left)
        if let Expr::BinaryOp {
            left,
            op: BinOp::Eq,
            right,
        } = expr
            && let Expr::ArrayIndex { name, indices } = left.as_ref()
        {
            let val = self.eval_expr(right)?;
            let idx_vals: Vec<i64> = indices
                .iter()
                .map(|e| self.eval_expr(e).and_then(|v| v.to_i64()))
                .collect::<Result<Vec<_>, _>>()?;
            let key = Self::array_key(name, &idx_vals);
            self.env.borrow_mut().set(&key, val);
            return Ok(ControlFlow::Normal);
        }
        let val = self.eval_expr(expr)?;
        self.env.borrow_mut().set(&var.name, val);
        Ok(ControlFlow::Normal)
    }

    fn exec_dim(&mut self, decls: &[DimDecl]) -> Result<ControlFlow, RuntimeError> {
        for decl in decls {
            let resolved = Self::resolve_decl_type(decl);
            // Store dimension metadata for MAT operations
            if let Some(dims) = &decl.dimensions {
                let base = self.env.borrow().option_base as i64;
                let mut bounds = Vec::new();
                for dim in dims {
                    let (lower, upper) = match dim {
                        (upper_expr, None) => {
                            let upper = self.eval_expr(upper_expr)?.to_i64()?;
                            (base, upper)
                        }
                        (lower_expr, Some(upper_expr)) => {
                            let lower = self.eval_expr(lower_expr)?.to_i64()?;
                            let upper = self.eval_expr(upper_expr)?.to_i64()?;
                            (lower, upper)
                        }
                    };
                    bounds.push((lower, upper));
                }
                self.array_dim_info.insert(decl.name.clone(), bounds);
            }
            if let BasicType::UserDefined(ref type_name) = resolved {
                if decl.dimensions.is_some() {
                    self.array_type_map
                        .insert(decl.name.clone(), type_name.clone());
                } else {
                    let record = self.create_default_record(type_name)?;
                    self.env.borrow_mut().set(&decl.name, record);
                }
            } else {
                let default = Value::default_for(resolved);
                self.env.borrow_mut().set(&decl.name, default);
            }
        }
        Ok(ControlFlow::Normal)
    }

    fn exec_read(&mut self, vars: &[Variable]) -> Result<ControlFlow, RuntimeError> {
        for var in vars {
            if self.data_pos >= self.data_values.len() {
                return Err(RuntimeError::General {
                    msg: "READ past end of DATA".into(),
                });
            }
            let item = &self.data_values[self.data_pos];
            self.data_pos += 1;
            let val = match item {
                DataItem::Number(n) => Value::Numeric(*n),
                DataItem::Str(s) => Value::Str(s.clone()),
            };
            self.env.borrow_mut().set(&var.name, val);
        }
        Ok(ControlFlow::Normal)
    }

    fn exec_redim(
        &mut self,
        decls: &[DimDecl],
        preserve: bool,
    ) -> Result<ControlFlow, RuntimeError> {
        for decl in decls {
            if !preserve {
                let prefix = format!("{}_", decl.name);
                let keys: Vec<String> = self
                    .env
                    .borrow()
                    .var_keys()
                    .into_iter()
                    .filter(|k| k.starts_with(&prefix))
                    .collect();
                for key in keys {
                    self.env.borrow_mut().vars_mut().remove(&key);
                }
            }
            let resolved = Self::resolve_decl_type(decl);
            if let Some(dims) = &decl.dimensions {
                let base = self.env.borrow().option_base as i64;
                let mut bounds = Vec::new();
                for dim in dims {
                    let (lower, upper) = match dim {
                        (upper_expr, None) => {
                            let upper = self.eval_expr(upper_expr)?.to_i64()?;
                            (base, upper)
                        }
                        (lower_expr, Some(upper_expr)) => {
                            let lower = self.eval_expr(lower_expr)?.to_i64()?;
                            let upper = self.eval_expr(upper_expr)?.to_i64()?;
                            (lower, upper)
                        }
                    };
                    bounds.push((lower, upper));
                }
                self.array_dim_info.insert(decl.name.clone(), bounds);
            } else {
                self.array_dim_info.remove(&decl.name);
                self.array_type_map.remove(&decl.name);
            }
            if let BasicType::UserDefined(ref type_name) = resolved {
                if decl.dimensions.is_some() {
                    self.array_type_map
                        .insert(decl.name.clone(), type_name.clone());
                } else {
                    let record = self.create_default_record(type_name)?;
                    self.env.borrow_mut().set(&decl.name, record);
                }
            } else {
                let default = Value::default_for(resolved);
                self.env.borrow_mut().set(&decl.name, default);
            }
        }
        Ok(ControlFlow::Normal)
    }

    fn exec_erase(&mut self, names: &[String]) -> Result<ControlFlow, RuntimeError> {
        for name in names {
            self.env.borrow_mut().set(name, Value::Numeric(0.0));
            let prefix = format!("{name}_");
            let keys: Vec<String> = self
                .env
                .borrow()
                .var_keys()
                .into_iter()
                .filter(|k| k.starts_with(&prefix))
                .collect();
            for key in keys {
                self.env.borrow_mut().vars_mut().remove(&key);
            }
        }
        Ok(ControlFlow::Normal)
    }

    fn exec_write_console(&mut self, exprs: &[Expr]) -> Result<ControlFlow, RuntimeError> {
        for (i, expr) in exprs.iter().enumerate() {
            if i > 0 {
                self.write_text(",");
            }
            let val = self.eval_expr(expr)?;
            match &val {
                Value::Str(s) => self.write_text(&format!("\"{}\"", s)),
                Value::Numeric(n) => {
                    if *n == (*n as i64) as f64 && n.abs() < 1e15 {
                        self.write_text(&format!("{}", *n as i64));
                    } else {
                        self.write_text(&format!("{}", n));
                    }
                }
                Value::Record { type_name, .. } => {
                    self.write_text(&format!("[{}]", type_name));
                }
            };
        }
        self.write_text("\n");
        Ok(ControlFlow::Normal)
    }

    fn exec_print(&mut self, ps: &PrintStmt) -> Result<(), RuntimeError> {
        // Handle PRINT USING
        if let Some(ref fmt_expr) = ps.format {
            let result = self.eval_format_using(fmt_expr, &ps.items)?;
            self.write_text(&result);
            match ps.trailing {
                PrintSep::Newline => {
                    self.write_text("\n");
                }
                PrintSep::Semicolon => {}
                PrintSep::Comma => {
                    let next_zone = ((self.print_col / 16) + 1) * 16;
                    let spaces = next_zone - self.print_col;
                    self.write_text(&" ".repeat(spaces));
                }
            }
            self.output.flush().ok();
            return Ok(());
        }

        for item in &ps.items {
            match item {
                PrintItem::Expr(expr) => {
                    let val = self.eval_expr(expr)?;
                    let s = val.format_for_print();
                    self.write_text(&s);
                }
                PrintItem::Tab(expr) => {
                    let n = self.eval_expr(expr)?.to_i64()?;
                    if n < 0 {
                        return Err(RuntimeError::IllegalFunctionCall {
                            msg: "TAB argument must be non-negative".into(),
                        });
                    }
                    let n = n as usize;
                    if n > self.print_col {
                        let spaces = n - self.print_col;
                        self.write_text(&" ".repeat(spaces));
                    }
                }
                PrintItem::Spc(expr) => {
                    let n = self.eval_expr(expr)?.to_i64()?;
                    if n < 0 {
                        return Err(RuntimeError::IllegalFunctionCall {
                            msg: "SPC argument must be non-negative".into(),
                        });
                    }
                    let n = n as usize;
                    self.write_text(&" ".repeat(n));
                }
                PrintItem::Comma => {
                    // Advance to next 14-column zone
                    let next_zone = ((self.print_col / 16) + 1) * 16;
                    let spaces = next_zone - self.print_col;
                    self.write_text(&" ".repeat(spaces));
                }
            }
        }
        match ps.trailing {
            PrintSep::Newline => {
                self.write_text("\n");
            }
            PrintSep::Semicolon => {}
            PrintSep::Comma => {
                let next_zone = ((self.print_col / 16) + 1) * 16;
                let spaces = next_zone - self.print_col;
                self.write_text(&" ".repeat(spaces));
            }
        }
        self.output.flush().ok();
        Ok(())
    }

    fn exec_input(&mut self, input: &InputStmt) -> Result<(), RuntimeError> {
        loop {
            if let Some(p) = &input.prompt {
                write!(self.output, "{}? ", p).ok();
            } else {
                write!(self.output, "? ").ok();
            }
            self.output.flush().ok();

            let mut line = String::new();
            self.input.read_line(&mut line).ok();
            let line = line.trim_end_matches('\n').trim_end_matches('\r');

            let parts: Vec<&str> = if input.vars.len() == 1 {
                vec![line]
            } else {
                line.split(',').map(|s| s.trim()).collect()
            };

            if parts.len() < input.vars.len() {
                writeln!(self.output, "? Redo from start").ok();
                continue;
            }

            for (var, part) in input.vars.iter().zip(parts.iter()) {
                let val = if var.name.ends_with('$') {
                    Value::Str(part.to_string())
                } else {
                    // Try to parse as number
                    if let Ok(n) = part.parse::<f64>() {
                        Value::Numeric(n)
                    } else {
                        Value::Str(part.to_string())
                    }
                };
                self.env.borrow_mut().set(&var.name, val);
            }
            break;
        }
        self.print_col = 0;
        self.print_row += 1;
        Ok(())
    }

    fn exec_if(&mut self, if_stmt: &IfStmt) -> Result<ControlFlow, RuntimeError> {
        let cond = self.eval_expr(&if_stmt.condition)?.is_truthy()?;
        if cond {
            return self.exec_block(&if_stmt.then_body);
        }

        for (cond_expr, body) in &if_stmt.elseif_clauses {
            let cond = self.eval_expr(cond_expr)?.is_truthy()?;
            if cond {
                return self.exec_block(body);
            }
        }

        if let Some(else_body) = &if_stmt.else_body {
            return self.exec_block(else_body);
        }

        Ok(ControlFlow::Normal)
    }

    fn exec_on_goto(
        &mut self,
        expr: &Expr,
        labels: &[Label],
        is_gosub: bool,
    ) -> Result<ControlFlow, RuntimeError> {
        let val = self.eval_expr(expr)?;
        let n = val.to_i64()?;
        if n >= 1 && n <= labels.len() as i64 {
            let label = &labels[(n - 1) as usize];
            if is_gosub {
                Ok(ControlFlow::Gosub(label.clone()))
            } else {
                Ok(ControlFlow::Goto(label.clone()))
            }
        } else {
            Ok(ControlFlow::Normal)
        }
    }

    fn exec_for(&mut self, for_stmt: &ForStmt) -> Result<ControlFlow, RuntimeError> {
        let start = self.eval_expr(&for_stmt.start)?;
        let end = self.eval_expr(&for_stmt.end)?;
        let step = if let Some(s) = &for_stmt.step {
            self.eval_expr(s)?
        } else {
            Value::Numeric(1.0)
        };

        let step_val = step.to_f64()?;
        let end_val = end.to_f64()?;
        self.env.borrow_mut().set(&for_stmt.var.name, start);

        loop {
            let current = self
                .env
                .borrow()
                .get(&for_stmt.var.name)
                .unwrap_or(Value::Numeric(0.0));
            let cur_val = current.to_f64()?;

            // Check loop condition
            if step_val > 0.0 && cur_val > end_val {
                break;
            }
            if step_val < 0.0 && cur_val < end_val {
                break;
            }
            if step_val == 0.0 {
                break; // Prevent infinite loop
            }

            let cf = self.exec_block(&for_stmt.body)?;
            match cf {
                ControlFlow::ExitFor => break,
                ControlFlow::End => return Ok(ControlFlow::End),
                ControlFlow::Goto(l) => return Ok(ControlFlow::Goto(l)),
                ControlFlow::ExitSub => return Ok(ControlFlow::ExitSub),
                ControlFlow::ExitFunction(v) => return Ok(ControlFlow::ExitFunction(v)),
                ControlFlow::Retry | ControlFlow::Continue => return Ok(cf),
                _ => {}
            }

            // Increment
            let current = self
                .env
                .borrow()
                .get(&for_stmt.var.name)
                .unwrap_or(Value::Numeric(0.0));
            let new_val = current.to_f64()? + step_val;
            self.env
                .borrow_mut()
                .set(&for_stmt.var.name, Value::Numeric(new_val));
        }

        Ok(ControlFlow::Normal)
    }

    fn exec_while(
        &mut self,
        condition: &Expr,
        body: &[LabeledStmt],
    ) -> Result<ControlFlow, RuntimeError> {
        loop {
            let cond = self.eval_expr(condition)?.is_truthy()?;
            if !cond {
                break;
            }
            let cf = self.exec_block(body)?;
            match cf {
                ControlFlow::End => return Ok(ControlFlow::End),
                ControlFlow::Goto(l) => return Ok(ControlFlow::Goto(l)),
                ControlFlow::ExitSub => return Ok(ControlFlow::ExitSub),
                ControlFlow::ExitFunction(v) => return Ok(ControlFlow::ExitFunction(v)),
                ControlFlow::ExitDo => return Ok(ControlFlow::ExitDo),
                ControlFlow::Retry | ControlFlow::Continue => return Ok(cf),
                _ => {}
            }
        }
        Ok(ControlFlow::Normal)
    }

    fn exec_do(&mut self, do_stmt: &DoLoopStmt) -> Result<ControlFlow, RuntimeError> {
        loop {
            if do_stmt.check_at_top
                && let Some(cond) = &do_stmt.condition
            {
                let result = self.eval_expr(cond)?.is_truthy()?;
                let should_continue = if do_stmt.is_while { result } else { !result };
                if !should_continue {
                    break;
                }
            }

            let cf = self.exec_block(&do_stmt.body)?;
            match cf {
                ControlFlow::ExitDo => break,
                ControlFlow::End => return Ok(ControlFlow::End),
                ControlFlow::Goto(l) => return Ok(ControlFlow::Goto(l)),
                ControlFlow::ExitSub => return Ok(ControlFlow::ExitSub),
                ControlFlow::ExitFunction(v) => return Ok(ControlFlow::ExitFunction(v)),
                ControlFlow::Retry | ControlFlow::Continue => return Ok(cf),
                _ => {}
            }

            if !do_stmt.check_at_top
                && let Some(cond) = &do_stmt.condition
            {
                let result = self.eval_expr(cond)?.is_truthy()?;
                let should_continue = if do_stmt.is_while { result } else { !result };
                if !should_continue {
                    break;
                }
            }
        }
        Ok(ControlFlow::Normal)
    }

    fn exec_select(&mut self, select: &SelectCaseStmt) -> Result<ControlFlow, RuntimeError> {
        let test_val = self.eval_expr(&select.expr)?;

        for case in &select.cases {
            let mut matched = false;
            for test in &case.tests {
                match test {
                    CaseTest::Value(expr) => {
                        let val = self.eval_expr(expr)?;
                        if test_val == val {
                            matched = true;
                        }
                    }
                    CaseTest::Range(lo, hi) => {
                        let lo_val = self.eval_expr(lo)?;
                        let hi_val = self.eval_expr(hi)?;
                        if test_val >= lo_val && test_val <= hi_val {
                            matched = true;
                        }
                    }
                    CaseTest::Comparison(op, expr) => {
                        let val = self.eval_expr(expr)?;
                        let result = match op {
                            CompareOp::Eq => test_val == val,
                            CompareOp::Ne => test_val != val,
                            CompareOp::Lt => test_val < val,
                            CompareOp::Gt => test_val > val,
                            CompareOp::Le => test_val <= val,
                            CompareOp::Ge => test_val >= val,
                        };
                        if result {
                            matched = true;
                        }
                    }
                }
                if matched {
                    break;
                }
            }
            if matched {
                return self.exec_block(&case.body);
            }
        }

        if let Some(else_body) = &select.else_body {
            return self.exec_block(else_body);
        }

        Ok(ControlFlow::Normal)
    }

    fn exec_when_exception(
        &mut self,
        body: &[LabeledStmt],
        handler: &[LabeledStmt],
    ) -> Result<ControlFlow, RuntimeError> {
        loop {
            let mut should_retry = false;
            for (i, stmt) in body.iter().enumerate() {
                match self.exec_stmt(&stmt.stmt) {
                    Ok(cf) => match cf {
                        ControlFlow::Normal => {}
                        other => return Ok(other),
                    },
                    Err(err) => {
                        // Enter exception handler
                        self.current_exception = Some(ExceptionInfo {
                            extype: err.ansi_extype(),
                            extext: err.to_string(),
                        });
                        // Execute handler
                        for h in handler {
                            match self.exec_stmt(&h.stmt)? {
                                ControlFlow::Retry => {
                                    should_retry = true;
                                    break;
                                }
                                ControlFlow::Continue => {
                                    // Continue executing body from statement i+1,
                                    // still under exception protection
                                    self.current_exception = None;
                                    let cf = self.exec_when_exception(&body[i + 1..], handler)?;
                                    return Ok(cf);
                                }
                                ControlFlow::Normal => {}
                                other => return Ok(other),
                            }
                        }
                        if should_retry {
                            break; // break inner for loop, outer loop will retry
                        }
                        // Handler completed without RETRY or CONTINUE
                        self.current_exception = None;
                        return Ok(ControlFlow::Normal);
                    }
                }
            }
            if !should_retry {
                break;
            }
        }
        self.current_exception = None;
        Ok(ControlFlow::Normal)
    }

    fn exec_sub_call(
        &mut self,
        name: &str,
        arg_exprs: &[Expr],
    ) -> Result<ControlFlow, RuntimeError> {
        // Evaluate arguments
        let args: Vec<Value> = arg_exprs
            .iter()
            .map(|e| self.eval_expr(e))
            .collect::<Result<Vec<_>, _>>()?;

        // Check for user-defined sub
        let sub = self.subs.get(name).cloned();
        if let Some(sub) = sub {
            if args.len() != sub.params.len() {
                return Err(RuntimeError::ArityMismatch {
                    expected: sub.params.len(),
                    got: args.len(),
                });
            }

            let child_env = Environment::new_child(self.env.clone());
            for (param, val) in sub.params.iter().zip(args.iter()) {
                child_env.borrow_mut().set(&param.name, val.clone());
            }

            // Load static variables
            if let Some(saved) = self.static_vars.get(name) {
                for (key, val) in saved {
                    child_env
                        .borrow_mut()
                        .vars_mut()
                        .insert(key.clone(), val.clone());
                }
            }

            let prev_env = self.env.clone();
            let prev_static = std::mem::take(&mut self.current_static_vars);
            if sub.is_static {
                // Mark all locals as static — we'll capture them after execution
            }
            self.env = child_env.clone();
            let result = self.exec_block(&sub.body);
            self.env = prev_env;

            // Save static variables
            if sub.is_static {
                // Save all non-param local variables
                let param_keys: HashSet<String> =
                    sub.params.iter().map(|p| p.name.clone()).collect();
                let locals: HashMap<String, Value> = child_env
                    .borrow()
                    .var_entries()
                    .filter(|(k, _)| !param_keys.contains(k.as_str()))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                self.static_vars.insert(name.to_string(), locals);
            } else if !self.current_static_vars.is_empty() {
                let saved = self.static_vars.entry(name.to_string()).or_default();
                for key in &self.current_static_vars {
                    if let Some(val) = child_env.borrow().vars_ref().get(key) {
                        saved.insert(key.clone(), val.clone());
                    }
                }
            }
            self.current_static_vars = prev_static;

            self.byref_writeback(&sub.params, arg_exprs, &child_env);

            match result? {
                ControlFlow::End => Ok(ControlFlow::End),
                _ => Ok(ControlFlow::Normal),
            }
        } else {
            Err(RuntimeError::General {
                msg: format!("undefined SUB: {name}"),
            })
        }
    }

    /// BYREF write-back: copy modified parameter values back to caller variables.
    /// Skips BYVAL params, array params, and non-variable argument expressions.
    fn byref_writeback(&self, params: &[Param], arg_exprs: &[Expr], child_env: &EnvRef) {
        for (i, param) in params.iter().enumerate() {
            if param.by_val || param.is_array {
                continue;
            }
            if let Some(Expr::Variable(caller_var)) = arg_exprs.get(i) {
                let val = child_env
                    .borrow()
                    .get(&param.name)
                    .unwrap_or(Value::Numeric(0.0));
                self.env.borrow_mut().set(&caller_var.name, val);
            }
        }
    }

    // ==================== Expression evaluation ====================

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::NumericLit(n) => Ok(Value::Numeric(*n)),
            Expr::StringLit(s) => Ok(Value::Str(s.clone())),
            Expr::Variable(var) => {
                // Auto-initialize undefined variables (classic BASIC behavior)
                if let Some(val) = self.env.borrow().get(&var.name) {
                    Ok(val)
                } else {
                    // Some 0-arg builtins are commonly used like variables in BASIC (e.g. DATE$, TIME$).
                    // Resolve those before default variable auto-initialization.
                    let builtin_name = &var.name;
                    if builtin_name == "CSRLIN" {
                        return Ok(Value::Numeric(self.print_row as f64));
                    }
                    if builtin_name == "INKEY$" {
                        return Ok(Value::Str(self.read_inkey()?));
                    }
                    if builtin_name == "DET" {
                        return Ok(Value::Numeric(self.last_det));
                    }
                    if builtin_name == "EXTYPE" {
                        return Ok(Value::Numeric(
                            self.current_exception
                                .as_ref()
                                .map_or(0.0, |e| e.extype as f64),
                        ));
                    }
                    if builtin_name == "EXTEXT$" {
                        return Ok(Value::Str(
                            self.current_exception
                                .as_ref()
                                .map_or(String::new(), |e| e.extext.clone()),
                        ));
                    }

                    let is_implicit_builtin =
                        matches!(builtin_name.as_str(), "DATE$" | "TIME$" | "TIMER");
                    if is_implicit_builtin
                        && let Some(result) = self.builtins.call(builtin_name, &[])?
                    {
                        return Ok(result);
                    }

                    let default = self.default_for_var(&var.name);
                    self.env.borrow_mut().set(&var.name, default.clone());
                    Ok(default)
                }
            }
            Expr::ArrayIndex { name, indices } => {
                let idx_vals: Vec<i64> = indices
                    .iter()
                    .map(|e| self.eval_expr(e).and_then(|v| v.to_i64()))
                    .collect::<Result<Vec<_>, _>>()?;
                // Simplified array lookup using flattened key
                let key = Self::array_key(name, &idx_vals);
                self.get_or_init_array_element(name, &key)
            }
            Expr::BinaryOp { left, op, right } => {
                let lval = self.eval_expr(left)?;
                let rval = self.eval_expr(right)?;
                self.eval_binary_op(&lval, *op, &rval)
            }
            Expr::UnaryOp { op, operand } => {
                let val = self.eval_expr(operand)?;
                self.eval_unary_op(*op, &val)
            }
            Expr::FunctionCall { name, args } => {
                let arg_vals: Vec<Value> = args
                    .iter()
                    .map(|e| self.eval_expr(e))
                    .collect::<Result<Vec<_>, _>>()?;

                // Stateful functions (need access to interpreter state)
                match name.as_str() {
                    "RND" => {
                        // RND with no args or positive arg → next random number
                        // RND(0) → return last random number
                        // RND(negative) → reseed with that value, return first number
                        if arg_vals.len() > 1 {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 1,
                                got: arg_vals.len(),
                            });
                        }
                        let arg = if arg_vals.is_empty() {
                            1.0
                        } else {
                            arg_vals[0].to_f64()?
                        };
                        if arg == 0.0 {
                            return Ok(Value::Numeric(self.last_rnd));
                        }
                        if arg < 0.0 {
                            self.rng_state = arg.to_bits();
                        }
                        // LCG step
                        self.rng_state = self
                            .rng_state
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        let r = ((self.rng_state >> 33) as f64) / ((1u64 << 31) as f64);
                        self.last_rnd = r;
                        return Ok(Value::Numeric(r));
                    }
                    "CSRLIN" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 0,
                                got: arg_vals.len(),
                            });
                        }
                        return Ok(Value::Numeric(self.print_row as f64));
                    }
                    "DET" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 0,
                                got: arg_vals.len(),
                            });
                        }
                        return Ok(Value::Numeric(self.last_det));
                    }
                    "EXTYPE" => {
                        return Ok(Value::Numeric(
                            self.current_exception
                                .as_ref()
                                .map_or(0.0, |e| e.extype as f64),
                        ));
                    }
                    "EXTEXT$" => {
                        return Ok(Value::Str(
                            self.current_exception
                                .as_ref()
                                .map_or(String::new(), |e| e.extext.clone()),
                        ));
                    }
                    "POS" => {
                        // POS takes 1 arg (ignored) — returns current column (1-indexed)
                        if arg_vals.len() != 1 {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 1,
                                got: arg_vals.len(),
                            });
                        }
                        return Ok(Value::Numeric((self.print_col + 1) as f64));
                    }
                    "INKEY$" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 0,
                                got: arg_vals.len(),
                            });
                        }
                        return Ok(Value::Str(self.read_inkey()?));
                    }
                    "INPUT$" => {
                        return self.eval_input_dollar(&arg_vals);
                    }
                    "SCREEN" => {
                        // SCREEN(row, col) returns ASCII code at position
                        // SCREEN(row, col, 1) returns color attribute (not implemented, returns 7)
                        if arg_vals.len() < 2 || arg_vals.len() > 3 {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 2,
                                got: arg_vals.len(),
                            });
                        }
                        let row = arg_vals[0].to_i64()? as usize;
                        let col = arg_vals[1].to_i64()? as usize;
                        if row < 1 || col < 1 {
                            return Err(RuntimeError::IllegalFunctionCall {
                                msg: format!("SCREEN({}, {}): row and col must be >= 1", row, col),
                            });
                        }
                        let r = row - 1;
                        let c = col - 1;
                        let ch = if r < self.screen_buffer.len() && c < self.screen_buffer[r].len()
                        {
                            self.screen_buffer[r][c]
                        } else {
                            b' '
                        };
                        return Ok(Value::Numeric(ch as f64));
                    }
                    "FREEFILE" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 0,
                                got: arg_vals.len(),
                            });
                        }
                        let n = (1..=255i64)
                            .find(|n| !self.file_handles.contains_key(n))
                            .unwrap_or(0);
                        return Ok(Value::Numeric(n as f64));
                    }
                    "EOF" => {
                        if arg_vals.len() != 1 {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 1,
                                got: arg_vals.len(),
                            });
                        }
                        let fnum = arg_vals[0].to_i64()?;
                        let fh = self.file_handles.get_mut(&fnum).ok_or_else(|| {
                            RuntimeError::General {
                                msg: format!("file #{fnum} is not open"),
                            }
                        })?;
                        // Proactively check EOF by peeking
                        if !fh.eof_flag {
                            if let Some(reader) = &mut fh.reader {
                                let buf = reader.fill_buf().unwrap_or(&[]);
                                if buf.is_empty() {
                                    fh.eof_flag = true;
                                }
                            } else {
                                fh.eof_flag = true;
                            }
                        }
                        return Ok(Value::Numeric(if fh.eof_flag { 1.0 } else { 0.0 }));
                    }
                    "LOF" => {
                        if arg_vals.len() != 1 {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 1,
                                got: arg_vals.len(),
                            });
                        }
                        let fnum = arg_vals[0].to_i64()?;
                        let fh =
                            self.file_handles
                                .get(&fnum)
                                .ok_or_else(|| RuntimeError::General {
                                    msg: format!("file #{fnum} is not open"),
                                })?;
                        let len = if let Some(reader) = &fh.reader {
                            reader
                                .get_ref()
                                .metadata()
                                .map(|m| m.len() as i64)
                                .unwrap_or(0)
                        } else if let Some(writer) = &fh.writer {
                            writer
                                .get_ref()
                                .metadata()
                                .map(|m| m.len() as i64)
                                .unwrap_or(0)
                        } else {
                            0
                        };
                        return Ok(Value::Numeric(len as f64));
                    }
                    "LOC" => {
                        if arg_vals.len() != 1 {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 1,
                                got: arg_vals.len(),
                            });
                        }
                        let fnum = arg_vals[0].to_i64()?;
                        let fh = self.file_handles.get_mut(&fnum).ok_or_else(|| {
                            RuntimeError::General {
                                msg: format!("file #{fnum} is not open"),
                            }
                        })?;
                        let pos = if let Some(reader) = &mut fh.reader {
                            reader.stream_position().unwrap_or(0) as i64
                        } else if let Some(writer) = &mut fh.writer {
                            writer.stream_position().unwrap_or(0) as i64
                        } else {
                            0
                        };
                        return Ok(Value::Numeric(pos as f64));
                    }
                    _ => {}
                }

                // Try builtin first
                if let Some(result) = self.builtins.call(name, &arg_vals)? {
                    return Ok(result);
                }

                // Try user-defined function
                let func = self.functions.get(name).cloned();
                if let Some(func) = func {
                    return self.call_user_function(&func, &arg_vals, args);
                }

                // Fall through to array access
                let idx_vals: Vec<i64> = arg_vals
                    .iter()
                    .map(|v| v.to_i64())
                    .collect::<Result<Vec<_>, _>>()?;
                let key = Self::array_key(name, &idx_vals);
                self.get_or_init_array_element(name, &key)
            }
            Expr::StringSlice { name, start, end } => {
                let s = self
                    .env
                    .borrow()
                    .get(name)
                    .unwrap_or(Value::Str(String::new()))
                    .to_string_val()?;
                let start_f = self.eval_expr(start)?.to_f64()?;
                let end_f = self.eval_expr(end)?.to_f64()?;
                if start_f < 1.0 || end_f < 1.0 {
                    return Err(RuntimeError::IllegalFunctionCall {
                        msg: "string slice index must be >= 1".into(),
                    });
                }
                let (start_0, end_0) = Self::string_slice_byte_range(&s, start_f, end_f);
                if start_0 > end_0 {
                    return Ok(Value::Str(String::new()));
                }
                Ok(Value::Str(s[start_0..end_0].to_string()))
            }
            Expr::Paren(inner) => self.eval_expr(inner),
            Expr::MemberAccess { object, field } => {
                let obj_val = self.eval_expr(object)?;
                match obj_val {
                    Value::Record { fields, .. } => {
                        fields
                            .get(field.as_str())
                            .cloned()
                            .ok_or_else(|| RuntimeError::General {
                                msg: format!("field '{}' not found in type", field),
                            })
                    }
                    _ => Err(RuntimeError::TypeMismatch {
                        msg: "member access on non-record value".into(),
                    }),
                }
            }
        }
    }

    fn call_user_function(
        &mut self,
        func: &UserFunction,
        args: &[Value],
        arg_exprs: &[Expr],
    ) -> Result<Value, RuntimeError> {
        if args.len() != func.params.len() {
            return Err(RuntimeError::ArityMismatch {
                expected: func.params.len(),
                got: args.len(),
            });
        }

        let child_env = Environment::new_child(self.env.clone());

        // Bind parameters
        for (param, val) in func.params.iter().zip(args.iter()) {
            child_env.borrow_mut().set(&param.name, val.clone());
        }

        // Load static variables
        let func_key = func.name.clone();
        if let Some(saved) = self.static_vars.get(&func_key) {
            for (key, val) in saved {
                child_env
                    .borrow_mut()
                    .vars_mut()
                    .insert(key.clone(), val.clone());
            }
        }

        // Initialize function return variable
        let return_default = if func.name.ends_with('$') {
            Value::Str(String::new())
        } else {
            Value::Numeric(0.0)
        };
        child_env.borrow_mut().set(&func.name, return_default);

        let prev_env = self.env.clone();
        let prev_static = std::mem::take(&mut self.current_static_vars);
        self.env = child_env.clone();
        let result = self.exec_block(&func.body);
        self.env = prev_env;

        // Save static variables
        if func.is_static {
            let param_keys: HashSet<String> = func.params.iter().map(|p| p.name.clone()).collect();
            let ret_key = func.name.clone();
            let locals: HashMap<String, Value> = child_env
                .borrow()
                .var_entries()
                .filter(|(k, _)| !param_keys.contains(k.as_str()) && *k != &ret_key)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            self.static_vars.insert(func_key, locals);
        } else if !self.current_static_vars.is_empty() {
            let saved = self.static_vars.entry(func_key).or_default();
            for key in &self.current_static_vars {
                if let Some(val) = child_env.borrow().vars_ref().get(key) {
                    saved.insert(key.clone(), val.clone());
                }
            }
        }
        self.current_static_vars = prev_static;

        self.byref_writeback(&func.params, arg_exprs, &child_env);

        result?;
        // Return value is stored in the function name variable
        Ok(child_env
            .borrow()
            .get(&func.name)
            .unwrap_or(Value::Numeric(0.0)))
    }

    fn eval_binary_op(
        &self,
        left: &Value,
        op: BinOp,
        right: &Value,
    ) -> Result<Value, RuntimeError> {
        let true_val = if self.dialect == crate::Dialect::QuickBasic {
            -1.0
        } else {
            1.0
        };

        // String concatenation with &
        if matches!(op, BinOp::Concat) {
            let a = left.to_string_val()?;
            let b = right.to_string_val()?;
            return Ok(Value::Str(format!("{a}{b}")));
        }

        // String concatenation with + (in QuickBASIC mode)
        if op == BinOp::Add && self.dialect == crate::Dialect::QuickBasic {
            if let Value::Str(sa) = left {
                let sb = right.to_string_val()?;
                return Ok(Value::Str(format!("{}{}", sa, sb)));
            }
            if let Value::Str(_) = right {
                return Err(RuntimeError::TypeMismatch {
                    msg: "cannot concatenate string and number".into(),
                });
            }
        }

        // String comparison
        if matches!(
            op,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge
        ) && let (Value::Str(a), Value::Str(b)) = (left, right)
        {
            let result = match op {
                BinOp::Eq => a == b,
                BinOp::Ne => a != b,
                BinOp::Lt => a < b,
                BinOp::Gt => a > b,
                BinOp::Le => a <= b,
                BinOp::Ge => a >= b,
                _ => unreachable!(),
            };
            return Ok(Value::Numeric(if result { true_val } else { 0.0 }));
        }

        // Numeric operations
        let a = left.to_f64()?;
        let b = right.to_f64()?;

        match op {
            BinOp::Add => Ok(Value::Numeric(a + b)),
            BinOp::Sub => Ok(Value::Numeric(a - b)),
            BinOp::Mul => Ok(Value::Numeric(a * b)),
            BinOp::Div => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero);
                }
                Ok(Value::Numeric(a / b))
            }
            BinOp::Mod => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero);
                }
                // ANSI MOD operates on reals: a MOD b = a - b * INT(a/b)
                Ok(Value::Numeric(a - b * (a / b).floor()))
            }
            BinOp::Pow => Ok(Value::Numeric(a.powf(b))),
            BinOp::Eq => Ok(Value::Numeric(if a == b { true_val } else { 0.0 })),
            BinOp::Ne => Ok(Value::Numeric(if a != b { true_val } else { 0.0 })),
            BinOp::Lt => Ok(Value::Numeric(if a < b { true_val } else { 0.0 })),
            BinOp::Gt => Ok(Value::Numeric(if a > b { true_val } else { 0.0 })),
            BinOp::Le => Ok(Value::Numeric(if a <= b { true_val } else { 0.0 })),
            BinOp::Ge => Ok(Value::Numeric(if a >= b { true_val } else { 0.0 })),
            // Logical/Bitwise operators
            BinOp::And => {
                if self.dialect == crate::Dialect::QuickBasic {
                    let ia = a as i64;
                    let ib = b as i64;
                    Ok(Value::Numeric((ia & ib) as f64))
                } else {
                    Ok(Value::Numeric(if a != 0.0 && b != 0.0 { 1.0 } else { 0.0 }))
                }
            }
            BinOp::Or => {
                if self.dialect == crate::Dialect::QuickBasic {
                    let ia = a as i64;
                    let ib = b as i64;
                    Ok(Value::Numeric((ia | ib) as f64))
                } else {
                    Ok(Value::Numeric(if a != 0.0 || b != 0.0 { 1.0 } else { 0.0 }))
                }
            }
            BinOp::Xor => {
                if self.dialect == crate::Dialect::QuickBasic {
                    let ia = a as i64;
                    let ib = b as i64;
                    Ok(Value::Numeric((ia ^ ib) as f64))
                } else {
                    Ok(Value::Numeric(if (a != 0.0) ^ (b != 0.0) {
                        1.0
                    } else {
                        0.0
                    }))
                }
            }
            BinOp::Concat => unreachable!("Concat handled above"),
        }
    }

    fn eval_unary_op(&self, op: UnaryOp, val: &Value) -> Result<Value, RuntimeError> {
        match op {
            UnaryOp::Neg => {
                let n = val.to_f64()?;
                Ok(Value::Numeric(-n))
            }
            UnaryOp::Not => {
                let n = val.to_f64()?;
                if self.dialect == crate::Dialect::QuickBasic {
                    let inum = n as i64;
                    Ok(Value::Numeric((!inum) as f64))
                } else {
                    Ok(Value::Numeric(if n == 0.0 { 1.0 } else { 0.0 }))
                }
            }
            UnaryOp::Pos => Ok(val.clone()),
        }
    }

    fn resolve_decl_type(decl: &DimDecl) -> BasicType {
        if let Some(ref t) = decl.as_type {
            t.clone()
        } else {
            BasicType::Numeric
        }
    }

    /// Return the default value for a variable
    fn default_for_var(&self, name: &str) -> Value {
        if name.ends_with('$') {
            Value::Str(String::new())
        } else {
            Value::Numeric(0.0)
        }
    }

    /// Build a flattened key for array element access (temporary hack until
    /// proper array storage is implemented).
    fn array_key(name: &str, indices: &[i64]) -> String {
        let idx_part: String = indices
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("_");
        format!("{}_{}", name, idx_part)
    }

    fn get_or_init_array_element(&mut self, name: &str, key: &str) -> Result<Value, RuntimeError> {
        if let Some(v) = self.env.borrow().get(key) {
            return Ok(v);
        }
        if let Some(type_name) = self.array_type_map.get(name).cloned() {
            let record = self.create_default_record(&type_name)?;
            self.env.borrow_mut().set(key, record.clone());
            Ok(record)
        } else {
            Ok(self.default_for_var(name))
        }
    }

    fn create_default_record(&self, type_name: &str) -> Result<Value, RuntimeError> {
        let field_specs: Vec<(String, BasicType)> = self
            .type_defs
            .get(type_name)
            .ok_or_else(|| RuntimeError::General {
                msg: format!("undefined type: {}", type_name),
            })?
            .iter()
            .map(|f| (f.name.clone(), f.field_type.clone()))
            .collect();
        let mut fields = HashMap::new();
        for (name, field_type) in field_specs {
            let val = match &field_type {
                BasicType::UserDefined(nested) => self.create_default_record(nested)?,
                other => Value::default_for(other.clone()),
            };
            fields.insert(name, val);
        }
        Ok(Value::Record {
            type_name: type_name.to_string(),
            fields,
        })
    }

    fn set_member_value(&mut self, target: &Expr, new_val: Value) -> Result<(), RuntimeError> {
        // Collect the member access path: walk MemberAccess chain to find root + field path
        let mut path = Vec::new();
        let mut current = target;
        while let Expr::MemberAccess { object, field } = current {
            path.push(field.clone());
            current = object;
        }
        path.reverse();

        // `current` is now the root expression (Variable or ArrayIndex)
        // `path` is the list of field names to traverse

        // Read the root value
        let root_key = match current {
            Expr::Variable(var) => var.name.clone(),
            Expr::ArrayIndex { name, indices } => {
                let idx_vals: Vec<i64> = indices
                    .iter()
                    .map(|e| self.eval_expr(e).and_then(|v| v.to_i64()))
                    .collect::<Result<Vec<_>, _>>()?;
                Self::array_key(name, &idx_vals)
            }
            _ => {
                return Err(RuntimeError::General {
                    msg: "invalid member assignment target".into(),
                });
            }
        };

        // Get or auto-init the root value
        let mut root_val = if let Some(v) = self.env.borrow().get(&root_key) {
            v
        } else if let Expr::ArrayIndex { name, .. } = current {
            self.get_or_init_array_element(name, &root_key)?
        } else {
            return Err(RuntimeError::General {
                msg: "variable not initialized".into(),
            });
        };

        // Navigate to the innermost record and set the field
        Self::set_nested_field(&mut root_val, &path, new_val, &self.type_defs)?;

        // Write back
        if let Expr::Variable(var) = current {
            self.env.borrow_mut().set(&var.name, root_val);
        } else {
            self.env.borrow_mut().set(&root_key, root_val);
        }
        Ok(())
    }

    fn set_nested_field(
        val: &mut Value,
        path: &[String],
        new_val: Value,
        type_defs: &HashMap<String, Vec<crate::ast::TypeField>>,
    ) -> Result<(), RuntimeError> {
        if path.is_empty() {
            return Ok(());
        }
        if let Value::Record { fields, type_name } = val {
            let field_name = &path[0];
            if path.len() == 1 {
                match fields.get_mut(field_name) {
                    Some(existing) => {
                        let coerced = Self::coerce_for_field_static(
                            type_defs, type_name, field_name, new_val,
                        )?;
                        *existing = coerced;
                    }
                    None => {
                        return Err(RuntimeError::General {
                            msg: format!("field '{}' not found in type {}", field_name, type_name),
                        });
                    }
                }
            } else {
                match fields.get_mut(field_name) {
                    Some(inner) => Self::set_nested_field(inner, &path[1..], new_val, type_defs)?,
                    None => {
                        return Err(RuntimeError::General {
                            msg: format!("field '{}' not found in type {}", field_name, type_name),
                        });
                    }
                }
            }
            Ok(())
        } else {
            Err(RuntimeError::TypeMismatch {
                msg: "member access on non-record value".into(),
            })
        }
    }

    fn coerce_for_field_static(
        type_defs: &HashMap<String, Vec<crate::ast::TypeField>>,
        type_name: &str,
        field_name: &str,
        val: Value,
    ) -> Result<Value, RuntimeError> {
        // In ANSI BASIC, no fixed-string coercion needed — just pass through
        let _ = (type_defs, type_name, field_name);
        Ok(val)
    }

    fn eval_format_using(
        &mut self,
        fmt_expr: &Expr,
        items: &[PrintItem],
    ) -> Result<String, RuntimeError> {
        let fmt_str = self.eval_expr(fmt_expr)?.to_string_val()?;
        let mut vals = Vec::new();
        for item in items {
            if let PrintItem::Expr(expr) = item {
                vals.push(self.eval_expr(expr)?);
            }
        }
        crate::format_using::format_using(&fmt_str, &vals)
    }

    fn extract_matrix(&self, name: &str) -> Result<Vec<Vec<f64>>, RuntimeError> {
        let prefix = format!("{}_", name);
        let env = self.env.borrow();
        let (row_base, col_base, rows, cols, has_dim_info) =
            if let Some(bounds) = self.array_dim_info.get(name) {
                if bounds.len() == 2 {
                    let rows = (bounds[0].1 - bounds[0].0 + 1).max(0) as usize;
                    let cols = (bounds[1].1 - bounds[1].0 + 1).max(0) as usize;
                    (bounds[0].0, bounds[1].0, rows, cols, true)
                } else {
                    let base = env.option_base as i64;
                    (base, base, 0, 0, false)
                }
            } else {
                let base = env.option_base as i64;
                (base, base, 0, 0, false)
            };
        let mut max_row: i64 = row_base - 1;
        let mut max_col: i64 = col_base - 1;
        let mut cells: Vec<(i64, i64, f64)> = Vec::new();
        for key in env.var_keys() {
            if let Some(suffix) = key.strip_prefix(&prefix) {
                let parts: Vec<&str> = suffix.split('_').collect();
                if parts.len() == 2
                    && let (Ok(r), Ok(c)) = (parts[0].parse::<i64>(), parts[1].parse::<i64>())
                {
                    let val = env.get(&key).unwrap_or(Value::Numeric(0.0));
                    let f = val.to_f64().unwrap_or(0.0);
                    cells.push((r, c, f));
                    if r > max_row {
                        max_row = r;
                    }
                    if c > max_col {
                        max_col = c;
                    }
                }
            }
        }
        let rows = if rows == 0 {
            (max_row - row_base + 1) as usize
        } else {
            rows
        };
        let cols = if cols == 0 {
            (max_col - col_base + 1) as usize
        } else {
            cols
        };
        if rows == 0 || cols == 0 || (!has_dim_info && (max_row < row_base || max_col < col_base)) {
            return Err(RuntimeError::General {
                msg: format!("MAT: array '{}' has no 2-D elements", name),
            });
        }
        let mut mat = vec![vec![0.0; cols]; rows];
        for (r, c, v) in cells {
            let ri = (r - row_base) as usize;
            let ci = (c - col_base) as usize;
            if ri < rows && ci < cols {
                mat[ri][ci] = v;
            }
        }
        Ok(mat)
    }

    fn store_matrix(&mut self, name: &str, mat: &[Vec<f64>]) {
        let (row_base, col_base) = self
            .array_dim_info
            .get(name)
            .and_then(|bounds| {
                if bounds.len() == 2 {
                    Some((bounds[0].0, bounds[1].0))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                let base = self.env.borrow().option_base as i64;
                (base, base)
            });
        let prefix = format!("{}_", name);
        let keys: Vec<String> = self
            .env
            .borrow()
            .var_keys()
            .into_iter()
            .filter(|k| k.starts_with(&prefix))
            .collect();
        for key in keys {
            self.env.borrow_mut().vars_mut().remove(&key);
        }
        for (i, row) in mat.iter().enumerate() {
            for (j, &val) in row.iter().enumerate() {
                let key = format!(
                    "{}_{}_{}",
                    name,
                    (i as i64) + row_base,
                    (j as i64) + col_base
                );
                self.env.borrow_mut().set(&key, Value::Numeric(val));
            }
        }
    }

    fn string_slice_byte_range(s: &str, start_f: f64, end_f: f64) -> (usize, usize) {
        let start = (start_f as usize).saturating_sub(1);
        let end = end_f as usize;
        if start >= end {
            return (s.len(), 0);
        }
        let start_byte = s
            .char_indices()
            .nth(start)
            .map(|(idx, _)| idx)
            .unwrap_or(s.len());
        let end_byte = s
            .char_indices()
            .nth(end)
            .map(|(idx, _)| idx)
            .unwrap_or(s.len());
        (start_byte, end_byte)
    }

    fn matrix_dims(&self, name: &str) -> Option<(usize, usize)> {
        // First check DIM metadata
        if let Some(bounds) = self.array_dim_info.get(name)
            && bounds.len() == 2
        {
            let rows = (bounds[0].1 - bounds[0].0 + 1) as usize;
            let cols = (bounds[1].1 - bounds[1].0 + 1) as usize;
            return Some((rows, cols));
        }
        // Fall back to scanning existing keys
        let prefix = format!("{}_", name);
        let env = self.env.borrow();
        let base = env.option_base as i64;
        let mut max_row: i64 = base - 1;
        let mut max_col: i64 = base - 1;
        for key in env.var_keys() {
            if let Some(suffix) = key.strip_prefix(&prefix) {
                let parts: Vec<&str> = suffix.split('_').collect();
                if parts.len() == 2
                    && let (Ok(r), Ok(c)) = (parts[0].parse::<i64>(), parts[1].parse::<i64>())
                {
                    if r > max_row {
                        max_row = r;
                    }
                    if c > max_col {
                        max_col = c;
                    }
                }
            }
        }
        if max_row >= base && max_col >= base {
            Some(((max_row - base + 1) as usize, (max_col - base + 1) as usize))
        } else {
            None
        }
    }

    fn exec_mat(&mut self, op: &MatOp) -> Result<ControlFlow, RuntimeError> {
        use crate::mat;
        match op {
            MatOp::Print { channel: _, name } => {
                let m = self.extract_matrix(name)?;
                for row in &m {
                    let formatted: Vec<String> = row
                        .iter()
                        .map(|v| Value::Numeric(*v).format_for_print())
                        .collect();
                    self.write_text(&formatted.join(" "));
                    self.write_text("\n");
                }
                self.output.flush().ok();
                Ok(ControlFlow::Normal)
            }
            MatOp::Read { name } => {
                let (rows, cols) = self
                    .matrix_dims(name)
                    .ok_or_else(|| RuntimeError::General {
                        msg: format!("MAT READ: array '{}' must be DIMed first", name),
                    })?;
                let mut m = vec![vec![0.0; cols]; rows];
                for row in m.iter_mut().take(rows) {
                    for cell in row.iter_mut().take(cols) {
                        if self.data_pos >= self.data_values.len() {
                            return Err(RuntimeError::General {
                                msg: "MAT READ: READ past end of DATA".into(),
                            });
                        }
                        let item = &self.data_values[self.data_pos];
                        self.data_pos += 1;
                        *cell = match item {
                            DataItem::Number(n) => *n,
                            DataItem::Str(s) => s.parse::<f64>().unwrap_or(0.0),
                        };
                    }
                }
                self.store_matrix(name, &m);
                Ok(ControlFlow::Normal)
            }
            MatOp::Input { channel: _, name } => {
                let (rows, cols) = self
                    .matrix_dims(name)
                    .ok_or_else(|| RuntimeError::General {
                        msg: format!("MAT INPUT: array '{}' must be DIMed first", name),
                    })?;
                let mut m = vec![vec![0.0; cols]; rows];
                for row in m.iter_mut().take(rows) {
                    write!(self.output, "? ").ok();
                    self.output.flush().ok();
                    let mut line = String::new();
                    self.input.read_line(&mut line).ok();
                    let parts: Vec<&str> = line.trim().split(',').map(|s| s.trim()).collect();
                    for (c, cell) in row.iter_mut().enumerate().take(cols) {
                        *cell = parts
                            .get(c)
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0);
                    }
                }
                self.store_matrix(name, &m);
                Ok(ControlFlow::Normal)
            }
            MatOp::Assign { target, source } => {
                match source {
                    MatExpr::Name(src) => {
                        let m = self.extract_matrix(src)?;
                        self.store_matrix(target, &m);
                    }
                    MatExpr::Add(a, b) => {
                        let ma = self.extract_matrix(a)?;
                        let mb = self.extract_matrix(b)?;
                        self.store_matrix(target, &mat::mat_add(&ma, &mb)?);
                    }
                    MatExpr::Sub(a, b) => {
                        let ma = self.extract_matrix(a)?;
                        let mb = self.extract_matrix(b)?;
                        self.store_matrix(target, &mat::mat_sub(&ma, &mb)?);
                    }
                    MatExpr::Mul(a, b) => {
                        let ma = self.extract_matrix(a)?;
                        let mb = self.extract_matrix(b)?;
                        self.store_matrix(target, &mat::mat_mul(&ma, &mb)?);
                    }
                    MatExpr::ScalarMul(expr, a) => {
                        let k = self.eval_expr(expr)?.to_f64()?;
                        let ma = self.extract_matrix(a)?;
                        self.store_matrix(target, &mat::mat_scalar_mul(k, &ma));
                    }
                    MatExpr::Inv(a) => {
                        let ma = self.extract_matrix(a)?;
                        let (inv, det) = mat::mat_inv(&ma)?;
                        self.last_det = det;
                        self.store_matrix(target, &inv);
                    }
                    MatExpr::Trn(a) => {
                        let ma = self.extract_matrix(a)?;
                        self.store_matrix(target, &mat::mat_trn(&ma));
                    }
                    MatExpr::Zer => {
                        let (rows, cols) =
                            self.matrix_dims(target)
                                .ok_or_else(|| RuntimeError::General {
                                    msg: format!("MAT ZER: array '{}' must be DIMed first", target),
                                })?;
                        self.store_matrix(target, &mat::mat_zer(rows, cols));
                    }
                    MatExpr::Con => {
                        let (rows, cols) =
                            self.matrix_dims(target)
                                .ok_or_else(|| RuntimeError::General {
                                    msg: format!("MAT CON: array '{}' must be DIMed first", target),
                                })?;
                        self.store_matrix(target, &mat::mat_con(rows, cols));
                    }
                    MatExpr::Idn => {
                        let (rows, cols) =
                            self.matrix_dims(target)
                                .ok_or_else(|| RuntimeError::General {
                                    msg: format!("MAT IDN: array '{}' must be DIMed first", target),
                                })?;
                        if rows != cols {
                            return Err(RuntimeError::General {
                                msg: "MAT IDN: matrix must be square".into(),
                            });
                        }
                        self.store_matrix(target, &mat::mat_idn(rows));
                    }
                }
                Ok(ControlFlow::Normal)
            }
        }
    }

    /// Write visible text to output and update screen buffer.
    fn write_text(&mut self, text: &str) {
        write!(self.output, "{}", text).ok();
        crate::update_screen_buffer(
            &mut self.screen_buffer,
            &mut self.print_row,
            &mut self.print_col,
            text,
        );
    }

    /// Non-blocking read of a single keypress. Returns "" if no key available.
    /// In non-interactive mode (tests, piped input), always returns "".
    fn read_inkey(&mut self) -> Result<String, RuntimeError> {
        if !self.interactive {
            return Ok(String::new());
        }
        Ok(crate::poll_inkey())
    }

    /// INPUT$(n) — read n characters from keyboard; INPUT$(n, #filenum) — read n bytes from file.
    fn eval_input_dollar(&mut self, args: &[Value]) -> Result<Value, RuntimeError> {
        match args.len() {
            1 => {
                let n = args[0].to_i64()?;
                if n < 1 {
                    return Err(RuntimeError::IllegalFunctionCall {
                        msg: "INPUT$ count must be >= 1".to_string(),
                    });
                }
                let n = n as usize;
                let mut buf = vec![0u8; n];
                let mut total = 0;
                while total < n {
                    match self.input.read(&mut buf[total..]) {
                        Ok(0) => break,
                        Ok(bytes) => total += bytes,
                        Err(_) => break,
                    }
                }
                Ok(Value::Str(
                    String::from_utf8_lossy(&buf[..total]).into_owned(),
                ))
            }
            2 => {
                let n = args[0].to_i64()?;
                if n < 1 {
                    return Err(RuntimeError::IllegalFunctionCall {
                        msg: "INPUT$ count must be >= 1".to_string(),
                    });
                }
                let fnum = args[1].to_i64()?;
                let fh = self
                    .file_handles
                    .get_mut(&fnum)
                    .ok_or_else(|| RuntimeError::General {
                        msg: format!("file #{fnum} is not open"),
                    })?;
                let reader = fh.reader.as_mut().ok_or_else(|| RuntimeError::General {
                    msg: format!("file #{fnum} is not open for reading"),
                })?;
                let n = n as usize;
                let mut buf = vec![0u8; n];
                let mut total = 0;
                while total < n {
                    match reader.read(&mut buf[total..]) {
                        Ok(0) => break,
                        Ok(bytes) => total += bytes,
                        Err(_) => break,
                    }
                }
                Ok(Value::Str(
                    String::from_utf8_lossy(&buf[..total]).into_owned(),
                ))
            }
            _ => Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            }),
        }
    }

    fn color_fg_to_ansi(c: u8) -> u8 {
        match c {
            0 => 30,  // Black
            1 => 34,  // Blue
            2 => 32,  // Green
            3 => 36,  // Cyan
            4 => 31,  // Red
            5 => 35,  // Magenta
            6 => 33,  // Brown/Yellow
            7 => 37,  // White
            8 => 90,  // Gray
            9 => 94,  // Light Blue
            10 => 92, // Light Green
            11 => 96, // Light Cyan
            12 => 91, // Light Red
            13 => 95, // Light Magenta
            14 => 93, // Yellow
            15 => 97, // Bright White
            _ => 37,
        }
    }

    /// Map background color index (0–15) to ANSI SGR code.
    fn color_bg_to_ansi(c: u8) -> u8 {
        match c {
            0 => 40,
            1 => 44,
            2 => 42,
            3 => 46,
            4 => 41,
            5 => 45,
            6 => 43,
            7 => 47,
            8 => 100,
            9 => 104,
            10 => 102,
            11 => 106,
            12 => 101,
            13 => 105,
            14 => 103,
            15 => 107,
            _ => 40,
        }
    }

    // ==================== File I/O ====================

    fn exec_open(&mut self, open: &OpenStmt) -> Result<(), RuntimeError> {
        let filename = self.eval_expr(&open.name)?.to_string_val()?;
        let file_num = self.eval_expr(&open.channel)?.to_i64()?;

        if !(1..=255).contains(&file_num) {
            return Err(RuntimeError::General {
                msg: format!("invalid file number: {file_num}"),
            });
        }
        if self.file_handles.contains_key(&file_num) {
            return Err(RuntimeError::General {
                msg: format!("file #{file_num} is already open"),
            });
        }

        let (reader, writer) = match open.access {
            FileAccess::Input => {
                let f = File::open(&filename).map_err(|e| RuntimeError::General {
                    msg: format!("cannot open '{filename}': {e}"),
                })?;
                (Some(BufReader::new(f)), None)
            }
            FileAccess::Output => {
                let f = File::create(&filename).map_err(|e| RuntimeError::General {
                    msg: format!("cannot create '{filename}': {e}"),
                })?;
                (None, Some(BufWriter::new(f)))
            }
            FileAccess::OutIn => {
                let f = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(&filename)
                    .map_err(|e| RuntimeError::General {
                        msg: format!("cannot open '{filename}': {e}"),
                    })?;
                let f2 = f.try_clone().map_err(|e| RuntimeError::General {
                    msg: format!("cannot clone file handle: {e}"),
                })?;
                (Some(BufReader::new(f)), Some(BufWriter::new(f2)))
            }
            FileAccess::Append => {
                let f = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(&filename)
                    .map_err(|e| RuntimeError::General {
                        msg: format!("cannot open '{filename}': {e}"),
                    })?;
                (None, Some(BufWriter::new(f)))
            }
        };

        self.file_handles.insert(
            file_num,
            FileHandle {
                _access: open.access,
                reader,
                writer,
                eof_flag: false,
            },
        );

        Ok(())
    }

    fn exec_close(&mut self, file_nums: &[Expr]) -> Result<(), RuntimeError> {
        if file_nums.is_empty() {
            // Close all
            for (_, fh) in self.file_handles.drain() {
                if let Some(mut w) = fh.writer {
                    let _ = w.flush();
                }
            }
        } else {
            let nums: Vec<i64> = file_nums
                .iter()
                .map(|e| self.eval_expr(e).and_then(|v| v.to_i64()))
                .collect::<Result<Vec<_>, _>>()?;
            for n in nums {
                if let Some(fh) = self.file_handles.remove(&n)
                    && let Some(mut w) = fh.writer
                {
                    let _ = w.flush();
                }
            }
        }
        Ok(())
    }

    fn exec_set_pointer(
        &mut self,
        file_num_expr: &Expr,
        position_expr: &Expr,
    ) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(file_num_expr)?.to_i64()?;
        let position = self.eval_expr(position_expr)?.to_i64()?;
        let fh = self
            .file_handles
            .get_mut(&file_num)
            .ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open"),
            })?;
        // SET POINTER uses 1-based byte position
        let byte_pos = (position - 1).max(0) as u64;
        if let Some(writer) = &mut fh.writer {
            writer.flush().map_err(|e| RuntimeError::General {
                msg: format!("flush error: {e}"),
            })?;
            writer
                .seek(SeekFrom::Start(byte_pos))
                .map_err(|e| RuntimeError::General {
                    msg: format!("seek error: {e}"),
                })?;
        }
        if let Some(reader) = &mut fh.reader {
            reader
                .seek(SeekFrom::Start(byte_pos))
                .map_err(|e| RuntimeError::General {
                    msg: format!("seek error: {e}"),
                })?;
        }
        Ok(())
    }

    fn exec_ask_pointer(
        &mut self,
        file_num_expr: &Expr,
        var: &Variable,
    ) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(file_num_expr)?.to_i64()?;
        let fh = self
            .file_handles
            .get_mut(&file_num)
            .ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open"),
            })?;
        // Flush writer to ensure position reflects writes
        if let Some(writer) = &mut fh.writer {
            writer.flush().map_err(|e| RuntimeError::General {
                msg: format!("flush error: {e}"),
            })?;
        }
        let byte_pos = if let Some(writer) = &mut fh.writer {
            writer.stream_position().unwrap_or(0)
        } else if let Some(reader) = &mut fh.reader {
            reader.stream_position().unwrap_or(0)
        } else {
            0
        };
        // ASK POINTER returns 1-based byte position
        let result = byte_pos as i64 + 1;
        self.env
            .borrow_mut()
            .set(&var.name, Value::Numeric(result as f64));
        Ok(())
    }

    fn exec_file_print(&mut self, pf: &FilePrintStmt) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(&pf.file_num)?.to_i64()?;

        // Handle PRINT #n, USING
        if let Some(ref fmt_expr) = pf.format {
            let result = self.eval_format_using(fmt_expr, &pf.items)?;
            let trailing = pf.trailing;

            let fh = self
                .file_handles
                .get_mut(&file_num)
                .ok_or_else(|| RuntimeError::General {
                    msg: format!("file #{file_num} is not open"),
                })?;
            let writer = fh.writer.as_mut().ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open for writing"),
            })?;

            let _ = write!(writer, "{}", result);
            match trailing {
                PrintSep::Newline => {
                    let _ = writeln!(writer);
                }
                PrintSep::Semicolon => {}
                PrintSep::Comma => {
                    let _ = write!(writer, "\t");
                }
            }
            return Ok(());
        }

        // Evaluate all items first to avoid borrow conflicts
        let mut parts: Vec<String> = Vec::new();
        let trailing = pf.trailing;
        for item in &pf.items {
            match item {
                PrintItem::Expr(expr) => {
                    let val = self.eval_expr(expr)?;
                    parts.push(val.format_for_print());
                }
                PrintItem::Tab(_) | PrintItem::Spc(_) => {
                    // Simplified: just add a space
                    parts.push(" ".to_string());
                }
                PrintItem::Comma => {
                    parts.push("\t".to_string());
                }
            }
        }

        let fh = self
            .file_handles
            .get_mut(&file_num)
            .ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open"),
            })?;
        let writer = fh.writer.as_mut().ok_or_else(|| RuntimeError::General {
            msg: format!("file #{file_num} is not open for writing"),
        })?;

        for part in &parts {
            let _ = write!(writer, "{}", part);
        }
        match trailing {
            PrintSep::Newline => {
                let _ = writeln!(writer);
            }
            PrintSep::Semicolon => {}
            PrintSep::Comma => {
                let _ = write!(writer, "\t");
            }
        }

        Ok(())
    }

    fn exec_file_write(&mut self, wf: &FileWriteStmt) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(&wf.file_num)?.to_i64()?;

        // Evaluate all expressions first
        let vals: Vec<Value> = wf
            .exprs
            .iter()
            .map(|e| self.eval_expr(e))
            .collect::<Result<Vec<_>, _>>()?;

        let fh = self
            .file_handles
            .get_mut(&file_num)
            .ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open"),
            })?;
        let writer = fh.writer.as_mut().ok_or_else(|| RuntimeError::General {
            msg: format!("file #{file_num} is not open for writing"),
        })?;

        for (i, val) in vals.iter().enumerate() {
            if i > 0 {
                let _ = write!(writer, ",");
            }
            match val {
                Value::Str(s) => {
                    let _ = write!(writer, "\"{}\"", s);
                }
                _ => {
                    let _ = write!(writer, "{}", val.format_for_write());
                }
            }
        }
        let _ = writeln!(writer);

        Ok(())
    }

    fn exec_file_input(&mut self, fi: &FileInputStmt) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(&fi.file_num)?.to_i64()?;

        // Read fields from file for each variable
        let mut fields: Vec<String> = Vec::new();
        {
            let fh = self
                .file_handles
                .get_mut(&file_num)
                .ok_or_else(|| RuntimeError::General {
                    msg: format!("file #{file_num} is not open"),
                })?;
            let reader = fh.reader.as_mut().ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open for reading"),
            })?;

            for _ in 0..fi.vars.len() {
                let field = Self::read_next_field(reader)?;
                if field.is_none() {
                    fh.eof_flag = true;
                    break;
                }
                fields.push(field.unwrap());
            }

            // Check if we've reached EOF
            let buf = reader.fill_buf().unwrap_or(&[]);
            if buf.is_empty() {
                fh.eof_flag = true;
            }
        }

        for (i, var) in fi.vars.iter().enumerate() {
            let field = fields.get(i).cloned().unwrap_or_default();
            let val = if var.name.ends_with('$') {
                Value::Str(field)
            } else if let Ok(n) = field.parse::<f64>() {
                Value::Numeric(n)
            } else {
                Value::Str(field)
            };
            self.env.borrow_mut().set(&var.name, val);
        }

        Ok(())
    }

    fn read_next_field(reader: &mut BufReader<File>) -> Result<Option<String>, RuntimeError> {
        // Skip leading whitespace (spaces, tabs) but not newlines
        loop {
            let buf = reader.fill_buf().map_err(|e| RuntimeError::General {
                msg: format!("file read error: {e}"),
            })?;
            if buf.is_empty() {
                return Ok(None);
            }
            let ch = buf[0];
            match ch {
                b' ' | b'\t' => {
                    reader.consume(1);
                }
                b'\r' | b'\n' => {
                    reader.consume(1);
                    if ch == b'\r' {
                        let buf2 = reader.fill_buf().unwrap_or(&[]);
                        if !buf2.is_empty() && buf2[0] == b'\n' {
                            reader.consume(1);
                        }
                    }
                }
                _ => break,
            }
        }

        let buf = reader.fill_buf().map_err(|e| RuntimeError::General {
            msg: format!("file read error: {e}"),
        })?;
        if buf.is_empty() {
            return Ok(None);
        }

        // Check for quoted string
        if buf[0] == b'"' {
            reader.consume(1); // consume opening quote
            let mut field = String::new();
            let mut byte = [0u8; 1];
            loop {
                let n = reader.read(&mut byte).unwrap_or(0);
                if n == 0 {
                    break;
                }
                if byte[0] == b'"' {
                    break;
                }
                field.push(byte[0] as char);
            }
            // Consume trailing comma; newlines are consumed at the start of the next field read.
            let buf = reader.fill_buf().unwrap_or(&[]);
            if !buf.is_empty() && buf[0] == b',' {
                reader.consume(1);
            }
            return Ok(Some(field));
        }

        // Unquoted field: read until comma or newline
        let mut field = String::new();
        let mut byte = [0u8; 1];
        loop {
            let n = reader.read(&mut byte).unwrap_or(0);
            if n == 0 {
                break;
            }
            if byte[0] == b',' || byte[0] == b'\r' || byte[0] == b'\n' {
                // Handle \r\n
                if byte[0] == b'\r' {
                    let buf = reader.fill_buf().unwrap_or(&[]);
                    if !buf.is_empty() && buf[0] == b'\n' {
                        reader.consume(1);
                    }
                }
                break;
            }
            field.push(byte[0] as char);
        }

        Ok(Some(field.trim().to_string()))
    }

    fn exec_line_input_file(
        &mut self,
        file_num_expr: &Expr,
        var: &Variable,
    ) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(file_num_expr)?.to_i64()?;

        let line = {
            let fh = self
                .file_handles
                .get_mut(&file_num)
                .ok_or_else(|| RuntimeError::General {
                    msg: format!("file #{file_num} is not open"),
                })?;
            let reader = fh.reader.as_mut().ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open for reading"),
            })?;

            let mut line = String::new();
            let bytes_read = reader
                .read_line(&mut line)
                .map_err(|e| RuntimeError::General {
                    msg: format!("file read error: {e}"),
                })?;

            if bytes_read == 0 {
                fh.eof_flag = true;
            }

            // Check if more data available
            let buf = reader.fill_buf().unwrap_or(&[]);
            if buf.is_empty() {
                fh.eof_flag = true;
            }

            line.trim_end_matches('\n')
                .trim_end_matches('\r')
                .to_string()
        };

        self.env.borrow_mut().set(&var.name, Value::Str(line));
        Ok(())
    }

    fn get_var_basic_type(&self, name: &str) -> BasicType {
        if name.ends_with('%') {
            BasicType::Integer
        } else if name.ends_with('&') {
            BasicType::Long
        } else if name.ends_with('!') {
            BasicType::Single
        } else if name.ends_with('#') {
            BasicType::Double
        } else if name.ends_with('$') {
            BasicType::String
        } else {
            if let Some(t_name) = self.array_type_map.get(name) {
                if t_name == "NUMERIC" {
                    BasicType::Numeric
                } else if t_name == "STRING" {
                    BasicType::String
                } else {
                    BasicType::UserDefined(t_name.clone())
                }
            } else {
                BasicType::Numeric
            }
        }
    }

    fn serialize_value(
        &self,
        writer: &mut BufWriter<File>,
        val: &Value,
        ty: &BasicType,
    ) -> Result<(), RuntimeError> {
        match ty {
            BasicType::Integer => {
                let n = val.to_f64()? as i16;
                writer
                    .write_all(&n.to_le_bytes())
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary write error: {e}"),
                    })?;
            }
            BasicType::Long => {
                let n = val.to_f64()? as i32;
                writer
                    .write_all(&n.to_le_bytes())
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary write error: {e}"),
                    })?;
            }
            BasicType::Single => {
                let n = val.to_f64()? as f32;
                writer
                    .write_all(&n.to_le_bytes())
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary write error: {e}"),
                    })?;
            }
            BasicType::Double | BasicType::Numeric => {
                let n = val.to_f64()?;
                writer
                    .write_all(&n.to_le_bytes())
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary write error: {e}"),
                    })?;
            }
            BasicType::FixedLengthString(len) => {
                let s = val.to_string_val()?;
                let mut bytes = s.into_bytes();
                bytes.resize(*len, 0);
                writer
                    .write_all(&bytes)
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary write error: {e}"),
                    })?;
            }
            BasicType::String => {
                let s = val.to_string_val()?;
                let len = s.len() as u16;
                writer
                    .write_all(&len.to_le_bytes())
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary write error: {e}"),
                    })?;
                writer
                    .write_all(s.as_bytes())
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary write error: {e}"),
                    })?;
            }
            BasicType::UserDefined(nested_name) => {
                if let Value::Record { fields, .. } = val {
                    let fields_def =
                        self.type_defs
                            .get(nested_name)
                            .ok_or_else(|| RuntimeError::General {
                                msg: format!("undefined TYPE: {nested_name}"),
                            })?;
                    for field in fields_def {
                        let field_val =
                            fields
                                .get(&field.name)
                                .ok_or_else(|| RuntimeError::General {
                                    msg: format!("missing field in record: {}", field.name),
                                })?;
                        self.serialize_value(writer, field_val, &field.field_type)?;
                    }
                } else {
                    return Err(RuntimeError::TypeMismatch {
                        msg: format!("expected record of type {nested_name}"),
                    });
                }
            }
        }
        Ok(())
    }

    fn deserialize_value(
        &self,
        reader: &mut BufReader<File>,
        ty: &BasicType,
    ) -> Result<Value, RuntimeError> {
        match ty {
            BasicType::Integer => {
                let mut buf = [0u8; 2];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary read error: {e}"),
                    })?;
                let val = i16::from_le_bytes(buf) as f64;
                Ok(Value::Numeric(val))
            }
            BasicType::Long => {
                let mut buf = [0u8; 4];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary read error: {e}"),
                    })?;
                let val = i32::from_le_bytes(buf) as f64;
                Ok(Value::Numeric(val))
            }
            BasicType::Single => {
                let mut buf = [0u8; 4];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary read error: {e}"),
                    })?;
                let val = f32::from_le_bytes(buf) as f64;
                Ok(Value::Numeric(val))
            }
            BasicType::Double | BasicType::Numeric => {
                let mut buf = [0u8; 8];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary read error: {e}"),
                    })?;
                let val = f64::from_le_bytes(buf);
                Ok(Value::Numeric(val))
            }
            BasicType::FixedLengthString(len) => {
                let mut buf = vec![0u8; *len];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary read error: {e}"),
                    })?;
                let s = String::from_utf8_lossy(&buf)
                    .trim_end_matches('\0')
                    .to_string();
                Ok(Value::Str(s))
            }
            BasicType::String => {
                let mut len_buf = [0u8; 2];
                reader
                    .read_exact(&mut len_buf)
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary read error: {e}"),
                    })?;
                let len = u16::from_le_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| RuntimeError::General {
                        msg: format!("binary read error: {e}"),
                    })?;
                let s = String::from_utf8_lossy(&buf).to_string();
                Ok(Value::Str(s))
            }
            BasicType::UserDefined(nested_name) => {
                let fields_def =
                    self.type_defs
                        .get(nested_name)
                        .ok_or_else(|| RuntimeError::General {
                            msg: format!("undefined TYPE: {nested_name}"),
                        })?;
                let mut fields = HashMap::new();
                for field in fields_def {
                    let field_val = self.deserialize_value(reader, &field.field_type)?;
                    fields.insert(field.name.clone(), field_val);
                }
                Ok(Value::Record {
                    type_name: nested_name.clone(),
                    fields,
                })
            }
        }
    }

    fn exec_get_put_inner(
        &mut self,
        fh: &mut FileHandle,
        gp: &GetPutStmt,
        record: Option<i64>,
    ) -> Result<(), RuntimeError> {
        if gp.is_get {
            if let Some(writer) = &mut fh.writer {
                writer.flush().ok();
            }

            if let Some(pos) = record {
                let byte_pos = (pos - 1).max(0) as u64;
                if let Some(reader) = &mut fh.reader {
                    reader
                        .seek(SeekFrom::Start(byte_pos))
                        .map_err(|e| RuntimeError::General {
                            msg: format!("seek error: {e}"),
                        })?;
                }
            }

            let reader = fh.reader.as_mut().ok_or_else(|| RuntimeError::General {
                msg: "file is not open for reading".into(),
            })?;

            if let Some(var) = &gp.var {
                if self.dialect == crate::Dialect::QuickBasic {
                    let var_type = self.get_var_basic_type(&var.name);
                    let val = self.deserialize_value(reader, &var_type)?;
                    self.env.borrow_mut().set(&var.name, val);
                } else {
                    let val = self
                        .env
                        .borrow()
                        .get(&var.name)
                        .unwrap_or(Value::Str(String::new()));
                    let read_len = match &val {
                        Value::Str(s) if !s.is_empty() => s.len(),
                        _ => 128,
                    };
                    let mut buf = vec![0u8; read_len];
                    let bytes_read = reader.read(&mut buf).unwrap_or(0);
                    if bytes_read == 0 {
                        fh.eof_flag = true;
                    }
                    buf.truncate(bytes_read);
                    let s = String::from_utf8_lossy(&buf)
                        .trim_end_matches('\0')
                        .to_string();
                    self.env.borrow_mut().set(&var.name, Value::Str(s));
                }
            }
        } else {
            if let Some(pos) = record {
                let byte_pos = (pos - 1).max(0) as u64;
                if let Some(writer) = &mut fh.writer {
                    writer
                        .seek(SeekFrom::Start(byte_pos))
                        .map_err(|e| RuntimeError::General {
                            msg: format!("seek error: {e}"),
                        })?;
                }
            }

            let writer = fh.writer.as_mut().ok_or_else(|| RuntimeError::General {
                msg: "file is not open for writing".into(),
            })?;

            if let Some(var) = &gp.var {
                if self.dialect == crate::Dialect::QuickBasic {
                    let val = self
                        .env
                        .borrow()
                        .get(&var.name)
                        .unwrap_or(Value::Numeric(0.0));
                    let var_type = self.get_var_basic_type(&var.name);
                    self.serialize_value(writer, &val, &var_type)?;
                } else {
                    let val = self
                        .env
                        .borrow()
                        .get(&var.name)
                        .unwrap_or(Value::Str(String::new()));
                    let s = match val {
                        Value::Str(s) => s,
                        other => other.format_for_write(),
                    };
                    let _ = writer.write_all(s.as_bytes());
                }
            }
        }

        Ok(())
    }

    fn exec_get_put(&mut self, gp: &GetPutStmt) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(&gp.file_num)?.to_i64()?;
        let record = if let Some(expr) = &gp.record {
            Some(self.eval_expr(expr)?.to_i64()?)
        } else {
            None
        };

        let mut fh = self
            .file_handles
            .remove(&file_num)
            .ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open"),
            })?;

        let res = self.exec_get_put_inner(&mut fh, gp, record);
        self.file_handles.insert(file_num, fh);
        res
    }
}
