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
use crate::token::TypeSuffix;
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
    Resume,
    ResumeNext,
    Chain {
        filespec: String,
        common_values: Vec<(CommonVarSpec, CommonTransferValue)>,
    },
}

#[derive(Clone, Debug)]
struct CommonVarSpec {
    as_type: Option<BasicType>,
    is_array: bool,
    is_shared: bool,
}

#[derive(Clone, Debug)]
enum CommonTransferValue {
    Scalar(Value),
    Array(Vec<(String, Value)>),
}

#[derive(Clone, Debug)]
struct ErrorInfo {
    err_code: i32,
    err_line: usize,
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
    suffix: Option<TypeSuffix>,
    params: Vec<Param>,
    body: Vec<LabeledStmt>,
    is_static: bool,
}

#[derive(Clone)]
struct DefFnDef {
    name: String,
    params: Vec<Param>,
    body: DefFnBody,
}

struct FileHandle {
    mode: FileMode,
    reader: Option<BufReader<File>>,
    writer: Option<BufWriter<File>>,
    rec_len: i64,
    eof_flag: bool,
}

pub struct Interpreter {
    env: EnvRef,
    builtins: BuiltinRegistry,
    subs: HashMap<String, UserSub>,
    functions: HashMap<String, UserFunction>,
    def_fns: HashMap<String, DefFnDef>,
    print_col: usize,
    print_row: usize,
    screen_width: usize,
    screen_height: usize,
    current_fg: Option<u8>,
    current_bg: Option<u8>,
    data_values: Vec<DataItem>,
    data_pos: usize,
    output: Box<dyn Write>,
    input: Box<dyn BufRead>,
    file_handles: HashMap<i64, FileHandle>,
    // Error handling state
    error_handler: Option<Label>,
    current_error: Option<ErrorInfo>,
    error_resume_pc: Option<usize>,
    in_error_handler: bool,
    // Random number generator state
    rng_state: u64,
    last_rnd: f64,
    // Phase 3: STATIC variable persistence
    static_vars: HashMap<String, HashMap<String, Value>>,
    current_static_vars: HashSet<String>,
    // Phase 4: DEFtype map (A=0 .. Z=25)
    deftype_map: [Option<BasicType>; 26],
    type_defs: HashMap<String, Vec<crate::ast::TypeField>>,
    array_type_map: HashMap<String, String>,
    // CHAIN/COMMON support
    common_declarations: Vec<(CommonVarSpec, String)>,
    source_dir: Option<std::path::PathBuf>,
    interactive: bool,
    screen_buffer: Vec<Vec<u8>>,
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

/// Extract the array name prefix from a flattened array key.
/// E.g., "MYARR_0_1" → "MYARR_", "MYARR%_0" → "MYARR%_"
fn find_array_prefix(key: &str) -> &str {
    // Array keys are NAME_idx or NAME%_idx — find first '_' followed by a digit
    for (i, c) in key.char_indices() {
        if c == '_' {
            if let Some(next) = key[i + 1..].chars().next() {
                if next.is_ascii_digit() {
                    return &key[..=i];
                }
            }
        }
    }
    // Fallback: entire key (shouldn't happen with well-formed array keys)
    key
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
            env: Environment::new_global(),
            builtins: BuiltinRegistry::new(),
            subs: HashMap::new(),
            functions: HashMap::new(),
            def_fns: HashMap::new(),
            print_col: 0,
            print_row: 1,
            screen_width: 80,
            screen_height: 25,
            current_fg: None,
            current_bg: None,
            data_values: Vec::new(),
            data_pos: 0,
            output,
            input,
            file_handles: HashMap::new(),
            error_handler: None,
            current_error: None,
            error_resume_pc: None,
            in_error_handler: false,
            rng_state: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64,
            last_rnd: 0.0,
            static_vars: HashMap::new(),
            current_static_vars: HashSet::new(),
            deftype_map: std::array::from_fn(|_| None),
            type_defs: HashMap::new(),
            array_type_map: HashMap::new(),
            common_declarations: Vec::new(),
            source_dir: None,
            interactive: false,
            screen_buffer: vec![vec![b' '; 80]; 25],
        }
    }

    pub fn run_source(&mut self, source: &str) -> Result<(), Box<dyn std::error::Error>> {
        let tokens = crate::lexer::Lexer::new(source).tokenize()?;
        let program = crate::parser::Parser::new(tokens).parse_program()?;
        self.run_program(&program)?;
        Ok(())
    }

    pub fn run_file(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = std::path::Path::new(path);
        let canonical = std::fs::canonicalize(path).map_err(|e| {
            RuntimeError::General {
                msg: format!("Cannot open '{}': {}", path.display(), e),
            }
        })?;
        self.source_dir = canonical.parent().map(|p| p.to_path_buf());
        let source = std::fs::read_to_string(&canonical).map_err(|e| {
            RuntimeError::General {
                msg: format!("Cannot read '{}': {}", canonical.display(), e),
            }
        })?;
        self.run_source(&source)
    }

    pub fn run_program(&mut self, program: &Program) -> Result<(), RuntimeError> {
        // Pre-scan: collect labels, DATA statements, SUB/FUNCTION definitions
        self.prescan(&program.statements);

        // Execute top-level statements
        let cf = self.exec_block(&program.statements)?;
        match cf {
            ControlFlow::Chain { filespec, common_values } => {
                self.chain_loop(filespec, common_values)
            }
            _ => Ok(()),
        }
    }

    fn prescan(&mut self, stmts: &[LabeledStmt]) {
        for (i, ls) in stmts.iter().enumerate() {
            if let Some(label) = &ls.label {
                self.env.borrow_mut().register_label(label, i);
            }
            match &ls.stmt {
                Stmt::Data(items) => {
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
                    let func_name = match func.suffix {
                        Some(s) => format!("{}{}", func.name, s.to_char()),
                        None => func.name.clone(),
                    };
                    self.functions.insert(
                        func_name,
                        UserFunction {
                            name: func.name.clone(),
                            suffix: func.suffix,
                            params: func.params.clone(),
                            body: func.body.clone(),
                            is_static: func.is_static,
                        },
                    );
                }
                Stmt::DefFn { name, params, body } => {
                    self.def_fns.insert(
                        name.clone(),
                        DefFnDef {
                            name: name.clone(),
                            params: params.clone(),
                            body: body.clone(),
                        },
                    );
                }
                Stmt::TypeDef { name, fields } => {
                    self.type_defs.insert(name.clone(), fields.clone());
                }
                Stmt::Common(common_stmt) => {
                    // Only unnamed blocks participate in CHAIN variable transfer
                    if common_stmt.block_name.is_none() {
                        for var in &common_stmt.vars {
                            let key = Environment::var_key(&var.name, var.suffix);
                            let spec = CommonVarSpec {
                                as_type: var.as_type.clone(),
                                is_array: var.is_array,
                                is_shared: common_stmt.shared,
                            };
                            self.common_declarations.push((spec, key));
                        }
                    }
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
                _ => {}
            }
        }
    }

    fn exec_block(&mut self, stmts: &[LabeledStmt]) -> Result<ControlFlow, RuntimeError> {
        let mut pc = 0;
        while pc < stmts.len() {
            let ls = &stmts[pc];
            let result = self.exec_stmt(&ls.stmt);
            let cf = match result {
                Ok(cf) => cf,
                Err(err) => {
                    if let (Some(handler), false) =
                        (&self.error_handler, self.in_error_handler)
                    {
                        // Trap the error
                        let handler = handler.clone();
                        self.current_error = Some(ErrorInfo {
                            err_code: err.qbasic_error_code(),
                            err_line: ls.line,
                        });
                        self.error_resume_pc = Some(pc);
                        self.in_error_handler = true;
                        // Resolve handler label and jump
                        let resolved = self.env.borrow().resolve_label(&handler);
                        if let Some(idx) = resolved {
                            pc = idx;
                            continue;
                        } else {
                            return Err(RuntimeError::UndefinedLabel {
                                label: handler.to_string(),
                            });
                        }
                    } else {
                        return Err(err);
                    }
                }
            };
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
                        self.env.borrow_mut().gosub_stack.push(pc + 1);
                        pc = idx;
                    } else {
                        return Ok(ControlFlow::Gosub(label));
                    }
                }
                ControlFlow::Return => {
                    let return_pc = self.env.borrow_mut().gosub_stack.pop();
                    pc = return_pc.ok_or(RuntimeError::ReturnWithoutGosub)?;
                }
                ControlFlow::Resume => {
                    if let Some(resume_pc) = self.error_resume_pc {
                        self.in_error_handler = false;
                        pc = resume_pc;
                    } else {
                        return Err(RuntimeError::ResumeWithoutError);
                    }
                }
                ControlFlow::ResumeNext => {
                    if let Some(resume_pc) = self.error_resume_pc {
                        self.in_error_handler = false;
                        pc = resume_pc + 1;
                    } else {
                        return Err(RuntimeError::ResumeWithoutError);
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
            Stmt::Let { var, expr } => {
                // Check for array assignment (encoded as BinaryOp::Eq with ArrayIndex left)
                if let Expr::BinaryOp {
                    left,
                    op: BinOp::Eq,
                    right,
                } = expr
                    && let Expr::ArrayIndex {
                        name,
                        suffix,
                        indices,
                    } = left.as_ref()
                {
                    let val = self.eval_expr(right)?;
                    let idx_vals: Vec<i64> = indices
                        .iter()
                        .map(|e| self.eval_expr(e).and_then(|v| v.to_i64()))
                        .collect::<Result<Vec<_>, _>>()?;
                    let key = Self::array_key(name, *suffix, &idx_vals);
                    self.env.borrow_mut().set(&key, None, val);
                    return Ok(ControlFlow::Normal);
                }
                let val = self.eval_expr(expr)?;
                self.env.borrow_mut().set(&var.name, var.suffix, val);
                Ok(ControlFlow::Normal)
            }
            Stmt::Dim(decls) => {
                for decl in decls {
                    let resolved = Self::resolve_decl_type(decl);
                    if let BasicType::UserDefined(ref type_name) = resolved {
                        if decl.dimensions.is_some() {
                            // Array of TYPE: register for lazy auto-init
                            self.array_type_map.insert(decl.name.clone(), type_name.clone());
                        } else {
                            let record = self.create_default_record(type_name)?;
                            self.env.borrow_mut().set(&decl.name, decl.suffix, record);
                        }
                    } else {
                        let default = Value::default_for(resolved);
                        self.env.borrow_mut().set(&decl.name, decl.suffix, default);
                    }
                }
                Ok(ControlFlow::Normal)
            }
            Stmt::Const { name, value } => {
                let val = self.eval_expr(value)?;
                self.env.borrow_mut().define_const(name, None, val)?;
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
                let line = line.trim_end_matches('\n').trim_end_matches('\r').to_string();
                self.env
                    .borrow_mut()
                    .set(&var.name, var.suffix, Value::Str(line));
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
            Stmt::ExitFor => Ok(ControlFlow::ExitFor),
            Stmt::ExitDo => Ok(ControlFlow::ExitDo),
            Stmt::ExitSub => Ok(ControlFlow::ExitSub),
            Stmt::ExitFunction => {
                // ExitFunction doesn't need to carry a value here;
                // the caller (call_user_function) reads the function-name variable.
                Ok(ControlFlow::ExitFunction(Value::Integer(0)))
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
            Stmt::Call { name, args } => {
                self.exec_sub_call(name, args)
            }
            Stmt::Swap { a, b } => {
                let va = self.env.borrow().get(&a.name, a.suffix).unwrap_or(Value::Integer(0));
                let vb = self.env.borrow().get(&b.name, b.suffix).unwrap_or(Value::Integer(0));
                self.env.borrow_mut().set(&a.name, a.suffix, vb);
                self.env.borrow_mut().set(&b.name, b.suffix, va);
                Ok(ControlFlow::Normal)
            }
            Stmt::Read(vars) => {
                for var in vars {
                    if self.data_pos >= self.data_values.len() {
                        return Err(RuntimeError::General {
                            msg: "READ past end of DATA".into(),
                        });
                    }
                    let item = &self.data_values[self.data_pos];
                    self.data_pos += 1;
                    let val = match item {
                        DataItem::Number(n) => Value::Double(*n),
                        DataItem::Str(s) => Value::Str(s.clone()),
                    };
                    self.env.borrow_mut().set(&var.name, var.suffix, val);
                }
                Ok(ControlFlow::Normal)
            }
            Stmt::Restore(label) => {
                if label.is_some() {
                    // TODO: restore to specific label
                }
                self.data_pos = 0;
                Ok(ControlFlow::Normal)
            }
            Stmt::Data(_) => Ok(ControlFlow::Normal), // handled in prescan
            Stmt::OptionBase(n) => {
                self.env.borrow_mut().option_base = *n;
                Ok(ControlFlow::Normal)
            }
            Stmt::Redim { decls, .. } => {
                for decl in decls {
                    let default = Value::default_for(Self::resolve_decl_type(decl));
                    self.env.borrow_mut().set(&decl.name, decl.suffix, default);
                    // Clear flattened array elements (keys like "NAME(0,1)")
                    let prefix = format!("{}(", decl.name);
                    let keys: Vec<String> = self.env.borrow().var_keys()
                        .into_iter()
                        .filter(|k| k.starts_with(&prefix))
                        .collect();
                    for key in keys {
                        self.env.borrow_mut().vars_mut().remove(&key);
                    }
                }
                Ok(ControlFlow::Normal)
            }
            Stmt::Erase(names) => {
                for name in names {
                    self.env.borrow_mut().set(name, None, Value::Integer(0));
                    // Clear flattened array elements
                    let prefix = format!("{name}(");
                    let keys: Vec<String> = self.env.borrow().var_keys()
                        .into_iter()
                        .filter(|k| k.starts_with(&prefix))
                        .collect();
                    for key in keys {
                        self.env.borrow_mut().vars_mut().remove(&key);
                    }
                }
                Ok(ControlFlow::Normal)
            }
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
            Stmt::OnErrorGoto(target) => {
                match target {
                    Some(label) => {
                        self.error_handler = Some(label.clone());
                    }
                    None => {
                        // ON ERROR GOTO 0 — disable error handling
                        self.error_handler = None;
                        self.current_error = None;
                        self.error_resume_pc = None;
                        self.in_error_handler = false;
                    }
                }
                Ok(ControlFlow::Normal)
            }
            Stmt::Resume(target) => {
                match target {
                    ResumeTarget::Default => {
                        Ok(ControlFlow::Resume)
                    }
                    ResumeTarget::Next => {
                        Ok(ControlFlow::ResumeNext)
                    }
                    ResumeTarget::Label(label) => {
                        // Clear error state and jump to the label
                        self.in_error_handler = false;
                        self.current_error = None;
                        self.error_resume_pc = None;
                        Ok(ControlFlow::Goto(label.clone()))
                    }
                }
            }
            Stmt::OnGoto { expr, labels } => {
                let n = self.eval_expr(expr)?.to_i64()? as usize;
                if n >= 1 && n <= labels.len() {
                    Ok(ControlFlow::Goto(labels[n - 1].clone()))
                } else {
                    Ok(ControlFlow::Normal)
                }
            }
            Stmt::OnGosub { expr, labels } => {
                let n = self.eval_expr(expr)?.to_i64()? as usize;
                if n >= 1 && n <= labels.len() {
                    Ok(ControlFlow::Gosub(labels[n - 1].clone()))
                } else {
                    Ok(ControlFlow::Normal)
                }
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

            // Phase 1: WRITE (console)
            Stmt::Write(exprs) => {
                for (i, expr) in exprs.iter().enumerate() {
                    if i > 0 {
                        self.write_text(",");
                    }
                    let val = self.eval_expr(expr)?;
                    match &val {
                        Value::Str(s) => self.write_text(&format!("\"{}\"", s)),
                        Value::Integer(n) => self.write_text(&format!("{}", n)),
                        Value::Long(n) => self.write_text(&format!("{}", n)),
                        Value::Single(n) => {
                            if *n == (*n as i64) as f64 && n.abs() < 1e15 {
                                self.write_text(&format!("{}", *n as i64));
                            } else {
                                self.write_text(&format!("{}", n));
                            }
                        }
                        Value::Double(n) => {
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

            // Phase 1: SLEEP
            Stmt::Sleep(expr) => {
                if let Some(e) = expr {
                    let secs = self.eval_expr(e)?.to_i64()?;
                    if secs > 0 {
                        std::thread::sleep(std::time::Duration::from_secs(secs as u64));
                    }
                }
                Ok(ControlFlow::Normal)
            }

            // Phase 1: CLEAR
            Stmt::Clear => {
                self.env.borrow_mut().clear_vars();
                self.data_pos = 0;
                Ok(ControlFlow::Normal)
            }

            // Phase 1: NAME old AS new
            Stmt::Name { old, new } => {
                let old_path = self.eval_expr(old)?.to_string_val()?;
                let new_path = self.eval_expr(new)?.to_string_val()?;
                std::fs::rename(&old_path, &new_path).map_err(|e| RuntimeError::General {
                    msg: format!("NAME error: {}", e),
                })?;
                Ok(ControlFlow::Normal)
            }

            // Phase 1: KILL
            Stmt::Kill(expr) => {
                let path = self.eval_expr(expr)?.to_string_val()?;
                std::fs::remove_file(&path).map_err(|e| RuntimeError::General {
                    msg: format!("KILL error: {}", e),
                })?;
                Ok(ControlFlow::Normal)
            }

            // Phase 1: MKDIR
            Stmt::Mkdir(expr) => {
                let path = self.eval_expr(expr)?.to_string_val()?;
                std::fs::create_dir(&path).map_err(|e| RuntimeError::General {
                    msg: format!("MKDIR error: {}", e),
                })?;
                Ok(ControlFlow::Normal)
            }

            // Phase 1: RMDIR
            Stmt::Rmdir(expr) => {
                let path = self.eval_expr(expr)?.to_string_val()?;
                std::fs::remove_dir(&path).map_err(|e| RuntimeError::General {
                    msg: format!("RMDIR error: {}", e),
                })?;
                Ok(ControlFlow::Normal)
            }

            // Phase 1: CHDIR
            Stmt::Chdir(expr) => {
                let path = self.eval_expr(expr)?.to_string_val()?;
                std::env::set_current_dir(&path).map_err(|e| RuntimeError::General {
                    msg: format!("CHDIR error: {}", e),
                })?;
                Ok(ControlFlow::Normal)
            }

            // Phase 1: SHELL
            Stmt::Shell(expr) => {
                if let Some(e) = expr {
                    let cmd = self.eval_expr(e)?.to_string_val()?;
                    #[cfg(target_os = "windows")]
                    let result = std::process::Command::new("cmd").args(["/c", &cmd]).status();
                    #[cfg(not(target_os = "windows"))]
                    let result = std::process::Command::new("sh").args(["-c", &cmd]).status();
                    result.map_err(|e| RuntimeError::General {
                        msg: format!("SHELL error: {}", e),
                    })?;
                }
                Ok(ControlFlow::Normal)
            }

            // Phase 2: MID$ assignment
            Stmt::MidAssign { var, start, length, replacement } => {
                let current = self.env.borrow().get(&var.name, var.suffix)
                    .unwrap_or(Value::Str(String::new()))
                    .to_string_val()?;
                let start_pos = (self.eval_expr(start)?.to_i64()? - 1).max(0) as usize;
                let repl = self.eval_expr(replacement)?.to_string_val()?;
                let mut chars: Vec<char> = current.chars().collect();
                let repl_chars: Vec<char> = repl.chars().collect();
                let char_count = chars.len();
                let max_len = if let Some(len_expr) = length {
                    self.eval_expr(len_expr)?.to_i64()? as usize
                } else {
                    char_count.saturating_sub(start_pos)
                };
                // Cannot extend string; replacement is truncated
                let avail = char_count.saturating_sub(start_pos);
                let replace_len = max_len.min(avail).min(repl_chars.len());
                for i in 0..replace_len {
                    if start_pos + i < chars.len() {
                        chars[start_pos + i] = repl_chars[i];
                    }
                }
                let result: String = chars.into_iter().collect();
                self.env.borrow_mut().set(&var.name, var.suffix, Value::Str(result));
                Ok(ControlFlow::Normal)
            }

            // Phase 2: LSET
            Stmt::Lset { var, expr } => {
                let current = self.env.borrow().get(&var.name, var.suffix)
                    .unwrap_or(Value::Str(String::new()))
                    .to_string_val()?;
                let target_len = current.chars().count();
                let new_val = self.eval_expr(expr)?.to_string_val()?;
                let new_chars: Vec<char> = new_val.chars().collect();
                let result: String = if new_chars.len() >= target_len {
                    new_chars[..target_len].iter().collect()
                } else {
                    let mut s: String = new_chars.into_iter().collect();
                    for _ in 0..(target_len - s.chars().count()) {
                        s.push(' ');
                    }
                    s
                };
                self.env.borrow_mut().set(&var.name, var.suffix, Value::Str(result));
                Ok(ControlFlow::Normal)
            }

            // Phase 2: RSET
            Stmt::Rset { var, expr } => {
                let current = self.env.borrow().get(&var.name, var.suffix)
                    .unwrap_or(Value::Str(String::new()))
                    .to_string_val()?;
                let target_len = current.chars().count();
                let new_val = self.eval_expr(expr)?.to_string_val()?;
                let new_chars: Vec<char> = new_val.chars().collect();
                let result: String = if new_chars.len() >= target_len {
                    new_chars[..target_len].iter().collect()
                } else {
                    let pad = target_len - new_chars.len();
                    let mut s = String::new();
                    for _ in 0..pad {
                        s.push(' ');
                    }
                    s.extend(new_chars);
                    s
                };
                self.env.borrow_mut().set(&var.name, var.suffix, Value::Str(result));
                Ok(ControlFlow::Normal)
            }

            // Phase 3: SHARED
            Stmt::Shared(vars) => {
                for var in vars {
                    let key = Environment::var_key(&var.name, var.suffix);
                    self.env.borrow_mut().shared_vars.insert(key);
                }
                Ok(ControlFlow::Normal)
            }

            // Phase 3: STATIC (variable declarations handled in exec_sub_call)
            Stmt::Static(decls) => {
                // Mark variables as static and initialize with defaults if not already loaded
                for decl in decls {
                    let key = Environment::var_key(&decl.name, decl.suffix);
                    if self.env.borrow().get(&decl.name, decl.suffix).is_none() {
                        let default = Value::default_for(Self::resolve_decl_type(decl));
                        self.env.borrow_mut().set(&decl.name, decl.suffix, default);
                    }
                    self.current_static_vars.insert(key);
                }
                Ok(ControlFlow::Normal)
            }

            // Phase 4: DEFtype
            Stmt::DefType { typ, ranges } => {
                for &(start, end) in ranges {
                    let s = (start as u8 - b'A') as usize;
                    let e = (end as u8 - b'A') as usize;
                    for i in s..=e.min(25) {
                        self.deftype_map[i] = Some(typ.clone());
                    }
                }
                Ok(ControlFlow::Normal)
            }

            // Phase 4: DEF FN (collected during prescan)
            Stmt::DefFn { .. } => Ok(ControlFlow::Normal),

            // User-defined types (collected during prescan)
            Stmt::TypeDef { .. } => Ok(ControlFlow::Normal),

            Stmt::MemberAssign { target, value } => {
                let new_val = self.eval_expr(value)?;
                self.set_member_value(target, new_val)?;
                Ok(ControlFlow::Normal)
            }

            // CHAIN/COMMON
            Stmt::Common(common_stmt) => {
                // COMMON SHARED: register vars as shared in the current environment
                if common_stmt.shared && common_stmt.block_name.is_none() {
                    for var in &common_stmt.vars {
                        let key = Environment::var_key(&var.name, var.suffix);
                        self.env.borrow_mut().shared_vars.insert(key);
                    }
                }
                Ok(ControlFlow::Normal)
            }

            Stmt::Chain { filespec } => {
                let path_str = self.eval_expr(filespec)?.to_string_val()?;
                let resolved_path = self.resolve_chain_path(&path_str);

                // Snapshot current COMMON variable values
                let env = &self.env;
                let common_values: Vec<(CommonVarSpec, CommonTransferValue)> = self
                    .common_declarations
                    .iter()
                    .map(|(spec, key)| {
                        let transfer = if spec.is_array {
                            // Collect all flattened array elements matching this var's key prefix
                            let prefix = format!("{}_", key);
                            let env_borrow = env.borrow();
                            let elements: Vec<(String, Value)> = env_borrow
                                .vars_ref()
                                .iter()
                                .filter(|(k, _)| k.starts_with(&prefix))
                                .map(|(k, v)| (k.clone(), v.clone()))
                                .collect();
                            CommonTransferValue::Array(elements)
                        } else {
                            let value = env.borrow().get_by_key(key).unwrap_or_else(|| {
                                Value::default_for_type(spec.as_type.as_ref())
                            });
                            CommonTransferValue::Scalar(value)
                        };
                        (spec.clone(), transfer)
                    })
                    .collect();

                Ok(ControlFlow::Chain {
                    filespec: resolved_path,
                    common_values,
                })
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
                    if v < 0 || v > 15 {
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
                    if v < 0 || v > 15 {
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
                    seq.push_str(&Self::qb_fg_to_ansi(f).to_string());
                    need_sep = true;
                }
                if let Some(b) = bg_val {
                    if need_sep { seq.push(';'); }
                    seq.push_str(&Self::qb_bg_to_ansi(b).to_string());
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
        }
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
                    let next_zone = ((self.print_col / 14) + 1) * 14;
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
                    let n = self.eval_expr(expr)?.to_i64()? as usize;
                    if n > self.print_col {
                        let spaces = n - self.print_col;
                        self.write_text(&" ".repeat(spaces));
                    }
                }
                PrintItem::Spc(expr) => {
                    let n = self.eval_expr(expr)?.to_i64()? as usize;
                    self.write_text(&" ".repeat(n));
                }
                PrintItem::Comma => {
                    // Advance to next 14-column zone
                    let next_zone = ((self.print_col / 14) + 1) * 14;
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
                let next_zone = ((self.print_col / 14) + 1) * 14;
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
                let val = if matches!(var.suffix, Some(TypeSuffix::String)) {
                    Value::Str(part.to_string())
                } else {
                    // Try to parse as number
                    if let Ok(n) = part.parse::<i64>() {
                        Value::Integer(n)
                    } else if let Ok(n) = part.parse::<f64>() {
                        Value::Double(n)
                    } else {
                        Value::Str(part.to_string())
                    }
                };
                self.env.borrow_mut().set(&var.name, var.suffix, val);
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

    fn exec_for(&mut self, for_stmt: &ForStmt) -> Result<ControlFlow, RuntimeError> {
        let start = self.eval_expr(&for_stmt.start)?;
        let end = self.eval_expr(&for_stmt.end)?;
        let step = if let Some(s) = &for_stmt.step {
            self.eval_expr(s)?
        } else {
            Value::Integer(1)
        };

        let step_val = step.to_f64()?;
        let end_val = end.to_f64()?;
        self.env
            .borrow_mut()
            .set(&for_stmt.var.name, for_stmt.var.suffix, start);

        loop {
            let current = self
                .env
                .borrow()
                .get(&for_stmt.var.name, for_stmt.var.suffix)
                .unwrap_or(Value::Integer(0));
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
                ControlFlow::Gosub(l) => return Ok(ControlFlow::Gosub(l)),
                ControlFlow::ExitSub => return Ok(ControlFlow::ExitSub),
                ControlFlow::ExitFunction(v) => return Ok(ControlFlow::ExitFunction(v)),
                _ => {}
            }

            // Increment
            let current = self
                .env
                .borrow()
                .get(&for_stmt.var.name, for_stmt.var.suffix)
                .unwrap_or(Value::Integer(0));
            let new_val = current.to_f64()? + step_val;
            let new_value = if matches!(current, Value::Integer(_)) && step_val == step_val.trunc()
            {
                Value::Integer(new_val as i64)
            } else {
                Value::Double(new_val)
            };
            self.env
                .borrow_mut()
                .set(&for_stmt.var.name, for_stmt.var.suffix, new_value);
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
                ControlFlow::Gosub(l) => return Ok(ControlFlow::Gosub(l)),
                ControlFlow::ExitSub => return Ok(ControlFlow::ExitSub),
                ControlFlow::ExitFunction(v) => return Ok(ControlFlow::ExitFunction(v)),
                ControlFlow::ExitDo => return Ok(ControlFlow::ExitDo),
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
                ControlFlow::Gosub(l) => return Ok(ControlFlow::Gosub(l)),
                ControlFlow::ExitSub => return Ok(ControlFlow::ExitSub),
                ControlFlow::ExitFunction(v) => return Ok(ControlFlow::ExitFunction(v)),
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
                child_env
                    .borrow_mut()
                    .set(&param.name, param.suffix, val.clone());
            }

            // Load static variables
            if let Some(saved) = self.static_vars.get(name) {
                for (key, val) in saved {
                    child_env.borrow_mut().vars_mut().insert(key.clone(), val.clone());
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
                let param_keys: HashSet<String> = sub.params.iter()
                    .map(|p| Environment::var_key(&p.name, p.suffix))
                    .collect();
                let locals: HashMap<String, Value> = child_env.borrow().var_entries()
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

    // ==================== Expression evaluation ====================

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::IntegerLit(n) => Ok(Value::Integer(*n)),
            Expr::DoubleLit(n) => Ok(Value::Double(*n)),
            Expr::StringLit(s) => Ok(Value::Str(s.clone())),
            Expr::Variable(var) => {
                // Auto-initialize undefined variables (classic BASIC behavior)
                if let Some(val) = self.env.borrow().get(&var.name, var.suffix) {
                    Ok(val)
                } else {
                    // Some 0-arg builtins are commonly used like variables in BASIC (e.g. DATE$, TIME$).
                    // Resolve those before default variable auto-initialization.
                    let builtin_name = match var.suffix {
                        Some(s) => format!("{}{}", var.name, s.to_char()),
                        None => var.name.clone(),
                    };
                    // ERR and ERL are special interpreter-state functions used without parens
                    if builtin_name == "ERR" || builtin_name == "ERL" {
                        return Ok(self.get_error_value(&builtin_name));
                    }
                    if builtin_name == "CSRLIN" {
                        return Ok(Value::Integer(self.print_row as i64));
                    }
                    if builtin_name == "INKEY$" {
                        return Ok(Value::Str(self.read_inkey()?));
                    }

                    let is_implicit_builtin = matches!(builtin_name.as_str(), "DATE$" | "TIME$" | "TIMER");
                    if is_implicit_builtin
                        && let Some(result) = self.builtins.call(&builtin_name, &[])?
                    {
                        return Ok(result);
                    }

                    let default = self.default_for_var(&var.name, var.suffix);
                    self.env
                        .borrow_mut()
                        .set(&var.name, var.suffix, default.clone());
                    Ok(default)
                }
            }
            Expr::ArrayIndex {
                name,
                suffix,
                indices,
            } => {
                let idx_vals: Vec<i64> = indices
                    .iter()
                    .map(|e| self.eval_expr(e).and_then(|v| v.to_i64()))
                    .collect::<Result<Vec<_>, _>>()?;
                // Simplified array lookup using flattened key
                let key = Self::array_key(name, *suffix, &idx_vals);
                self.get_or_init_array_element(name, *suffix, &key)
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
            Expr::FunctionCall { name, suffix, args } => {
                let arg_vals: Vec<Value> = args
                    .iter()
                    .map(|e| self.eval_expr(e))
                    .collect::<Result<Vec<_>, _>>()?;

                // Build canonical name
                let func_name = match suffix {
                    Some(s) => format!("{}{}", name, s.to_char()),
                    None => name.clone(),
                };

                // Stateful functions (need access to interpreter state)
                match name.as_str() {
                    "RND" => {
                        // RND with no args or positive arg → next random number
                        // RND(0) → return last random number
                        // RND(negative) → reseed with that value, return first number
                        if arg_vals.len() > 1 {
                            return Err(RuntimeError::ArityMismatch { expected: 1, got: arg_vals.len() });
                        }
                        let arg = if arg_vals.is_empty() { 1.0 } else { arg_vals[0].to_f64()? };
                        if arg == 0.0 {
                            return Ok(Value::Single(self.last_rnd as f32 as f64));
                        }
                        if arg < 0.0 {
                            self.rng_state = arg.to_bits();
                        }
                        // LCG step
                        self.rng_state = self.rng_state
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        let r = ((self.rng_state >> 33) as f64) / ((1u64 << 31) as f64);
                        self.last_rnd = r;
                        return Ok(Value::Single(r as f32 as f64));
                    }
                    "ERR" | "ERL" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch { expected: 0, got: arg_vals.len() });
                        }
                        return Ok(self.get_error_value(name));
                    }
                    "CSRLIN" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch { expected: 0, got: arg_vals.len() });
                        }
                        return Ok(Value::Integer(self.print_row as i64));
                    }
                    "POS" => {
                        // POS takes 1 arg (ignored) — returns current column (1-indexed)
                        if arg_vals.len() != 1 {
                            return Err(RuntimeError::ArityMismatch { expected: 1, got: arg_vals.len() });
                        }
                        return Ok(Value::Integer((self.print_col + 1) as i64));
                    }
                    "INKEY$" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch { expected: 0, got: arg_vals.len() });
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
                            return Err(RuntimeError::ArityMismatch { expected: 2, got: arg_vals.len() });
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
                        let ch = if r < self.screen_buffer.len()
                            && c < self.screen_buffer[r].len()
                        {
                            self.screen_buffer[r][c]
                        } else {
                            b' '
                        };
                        return Ok(Value::Integer(ch as i64));
                    }
                    "FREEFILE" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch { expected: 0, got: arg_vals.len() });
                        }
                        let n = (1..=255i64)
                            .find(|n| !self.file_handles.contains_key(n))
                            .unwrap_or(0);
                        return Ok(Value::Integer(n));
                    }
                    "EOF" => {
                        if arg_vals.len() != 1 {
                            return Err(RuntimeError::ArityMismatch { expected: 1, got: arg_vals.len() });
                        }
                        let fnum = arg_vals[0].to_i64()?;
                        let fh = self.file_handles.get_mut(&fnum).ok_or_else(|| RuntimeError::General {
                            msg: format!("file #{fnum} is not open"),
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
                        return Ok(Value::Integer(if fh.eof_flag { -1 } else { 0 }));
                    }
                    "LOF" => {
                        if arg_vals.len() != 1 {
                            return Err(RuntimeError::ArityMismatch { expected: 1, got: arg_vals.len() });
                        }
                        let fnum = arg_vals[0].to_i64()?;
                        let fh = self.file_handles.get(&fnum).ok_or_else(|| RuntimeError::General {
                            msg: format!("file #{fnum} is not open"),
                        })?;
                        let len = if let Some(reader) = &fh.reader {
                            reader.get_ref().metadata().map(|m| m.len() as i64).unwrap_or(0)
                        } else if let Some(writer) = &fh.writer {
                            writer.get_ref().metadata().map(|m| m.len() as i64).unwrap_or(0)
                        } else {
                            0
                        };
                        return Ok(Value::Integer(len));
                    }
                    "LOC" => {
                        if arg_vals.len() != 1 {
                            return Err(RuntimeError::ArityMismatch { expected: 1, got: arg_vals.len() });
                        }
                        let fnum = arg_vals[0].to_i64()?;
                        let fh = self.file_handles.get_mut(&fnum).ok_or_else(|| RuntimeError::General {
                            msg: format!("file #{fnum} is not open"),
                        })?;
                        let pos = if let Some(reader) = &mut fh.reader {
                            reader.stream_position().unwrap_or(0) as i64
                        } else if let Some(writer) = &mut fh.writer {
                            writer.stream_position().unwrap_or(0) as i64
                        } else {
                            0
                        };
                        return Ok(Value::Integer(pos));
                    }
                    _ => {}
                }

                // Try builtin first
                if let Some(result) = self.builtins.call(&func_name, &arg_vals)? {
                    return Ok(result);
                }
                // Try without suffix
                if let Some(result) = self.builtins.call(name, &arg_vals)? {
                    return Ok(result);
                }

                // Try DEF FN function
                let def_fn = self.def_fns.get(&func_name).or_else(|| self.def_fns.get(name)).cloned();
                if let Some(def_fn) = def_fn {
                    return self.call_def_fn(&def_fn, &arg_vals);
                }

                // Try user-defined function
                let func = self.functions.get(&func_name).or_else(|| self.functions.get(name)).cloned();
                if let Some(func) = func {
                    return self.call_user_function(&func, &arg_vals);
                }

                // Fall through to array access
                let idx_vals: Vec<i64> = arg_vals
                    .iter()
                    .map(|v| v.to_i64())
                    .collect::<Result<Vec<_>, _>>()?;
                let key = Self::array_key(name, *suffix, &idx_vals);
                self.get_or_init_array_element(name, *suffix, &key)
            }
            Expr::Paren(inner) => self.eval_expr(inner),
            Expr::MemberAccess { object, field } => {
                let obj_val = self.eval_expr(object)?;
                match obj_val {
                    Value::Record { fields, .. } => {
                        fields.get(field.as_str()).cloned().ok_or_else(|| {
                            RuntimeError::General {
                                msg: format!("field '{}' not found in type", field),
                            }
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
            child_env
                .borrow_mut()
                .set(&param.name, param.suffix, val.clone());
        }

        // Load static variables
        let func_key = match func.suffix {
            Some(s) => format!("{}{}", func.name, s.to_char()),
            None => func.name.clone(),
        };
        if let Some(saved) = self.static_vars.get(&func_key) {
            for (key, val) in saved {
                child_env.borrow_mut().vars_mut().insert(key.clone(), val.clone());
            }
        }

        // Initialize function return variable
        let return_default = Value::default_for_suffix(func.suffix);
        child_env
            .borrow_mut()
            .set(&func.name, func.suffix, return_default);

        let prev_env = self.env.clone();
        let prev_static = std::mem::take(&mut self.current_static_vars);
        self.env = child_env.clone();
        let result = self.exec_block(&func.body);
        self.env = prev_env;

        // Save static variables
        if func.is_static {
            let param_keys: HashSet<String> = func.params.iter()
                .map(|p| Environment::var_key(&p.name, p.suffix))
                .collect();
            let ret_key = Environment::var_key(&func.name, func.suffix);
            let locals: HashMap<String, Value> = child_env.borrow().var_entries()
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

        match result? {
            ControlFlow::ExitFunction(v) => Ok(v),
            _ => {
                // Return value is stored in the function name variable
                Ok(child_env
                    .borrow()
                    .get(&func.name, func.suffix)
                    .unwrap_or(Value::Integer(0)))
            }
        }
    }

    fn call_def_fn(
        &mut self,
        def_fn: &DefFnDef,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        if args.len() != def_fn.params.len() {
            return Err(RuntimeError::ArityMismatch {
                expected: def_fn.params.len(),
                got: args.len(),
            });
        }

        match &def_fn.body {
            DefFnBody::SingleLine(expr) => {
                // DEF FN shares the current scope — bind params temporarily
                let mut old_vals: Vec<(String, Option<TypeSuffix>, Option<Value>)> = Vec::new();
                for (param, val) in def_fn.params.iter().zip(args.iter()) {
                    let old = self.env.borrow().get(&param.name, param.suffix);
                    old_vals.push((param.name.clone(), param.suffix, old));
                    self.env.borrow_mut().set(&param.name, param.suffix, val.clone());
                }
                let result = self.eval_expr(expr);
                // Restore old values
                for (name, suffix, old) in old_vals {
                    match old {
                        Some(v) => self.env.borrow_mut().set(&name, suffix, v),
                        None => {
                            let key = Environment::var_key(&name, suffix);
                            self.env.borrow_mut().vars_mut().remove(&key);
                        }
                    }
                }
                result
            }
            DefFnBody::MultiLine(body) => {
                // Multi-line DEF FN: execute body, return value from function name variable
                let child_env = Environment::new_child(self.env.clone());
                for (param, val) in def_fn.params.iter().zip(args.iter()) {
                    child_env.borrow_mut().set(&param.name, param.suffix, val.clone());
                }
                // Initialize return variable
                child_env.borrow_mut().set(&def_fn.name, None, Value::Double(0.0));

                let prev_env = self.env.clone();
                self.env = child_env.clone();
                let result = self.exec_block(body);
                self.env = prev_env;

                match result? {
                    ControlFlow::ExitFunction(v) => Ok(v),
                    _ => Ok(child_env.borrow().get(&def_fn.name, None).unwrap_or(Value::Double(0.0))),
                }
            }
        }
    }

    fn eval_binary_op(
        &self,
        left: &Value,
        op: BinOp,
        right: &Value,
    ) -> Result<Value, RuntimeError> {
        // String concatenation
        if matches!(op, BinOp::Add)
            && let (Value::Str(a), Value::Str(b)) = (left, right)
        {
            return Ok(Value::Str(format!("{a}{b}")));
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
            return Ok(Value::Integer(if result { -1 } else { 0 }));
        }

        // Numeric operations
        let a = left.to_f64()?;
        let b = right.to_f64()?;

        match op {
            BinOp::Add => {
                let common = Value::common_numeric_type(left, right)?;
                let result = a + b;
                Self::make_numeric(result, common)
            }
            BinOp::Sub => {
                let common = Value::common_numeric_type(left, right)?;
                let result = a - b;
                Self::make_numeric(result, common)
            }
            BinOp::Mul => {
                let common = Value::common_numeric_type(left, right)?;
                let result = a * b;
                Self::make_numeric(result, common)
            }
            BinOp::Div => {
                if b == 0.0 {
                    return Err(RuntimeError::DivisionByZero);
                }
                Ok(Value::Double(a / b))
            }
            BinOp::IntDiv => {
                let bi = b as i64;
                if bi == 0 {
                    return Err(RuntimeError::DivisionByZero);
                }
                Ok(Value::Integer(a as i64 / bi))
            }
            BinOp::Mod => {
                let bi = b as i64;
                if bi == 0 {
                    return Err(RuntimeError::DivisionByZero);
                }
                Ok(Value::Integer(a as i64 % bi))
            }
            BinOp::Pow => {
                Ok(Value::Double(a.powf(b)))
            }
            BinOp::Eq => Ok(Value::Integer(if a == b { -1 } else { 0 })),
            BinOp::Ne => Ok(Value::Integer(if a != b { -1 } else { 0 })),
            BinOp::Lt => Ok(Value::Integer(if a < b { -1 } else { 0 })),
            BinOp::Gt => Ok(Value::Integer(if a > b { -1 } else { 0 })),
            BinOp::Le => Ok(Value::Integer(if a <= b { -1 } else { 0 })),
            BinOp::Ge => Ok(Value::Integer(if a >= b { -1 } else { 0 })),
            BinOp::And => Ok(Value::Integer(a as i64 & b as i64)),
            BinOp::Or => Ok(Value::Integer(a as i64 | b as i64)),
            BinOp::Xor => Ok(Value::Integer(a as i64 ^ b as i64)),
            BinOp::Eqv => Ok(Value::Integer(!(a as i64 ^ b as i64))),
            BinOp::Imp => Ok(Value::Integer(!(a as i64) | b as i64)),
        }
    }

    fn eval_unary_op(&self, op: UnaryOp, val: &Value) -> Result<Value, RuntimeError> {
        match op {
            UnaryOp::Neg => {
                let n = val.to_f64()?;
                match val {
                    Value::Integer(_) => Ok(Value::Integer(-n as i64)),
                    Value::Long(_) => Ok(Value::Long(-n as i64)),
                    Value::Single(_) => Ok(Value::Single(-n)),
                    _ => Ok(Value::Double(-n)),
                }
            }
            UnaryOp::Not => {
                let n = val.to_i64()?;
                Ok(Value::Integer(!n))
            }
            UnaryOp::Pos => Ok(val.clone()),
        }
    }

    fn make_numeric(n: f64, ty: BasicType) -> Result<Value, RuntimeError> {
        Ok(match ty {
            BasicType::Integer => Value::Integer(n as i64),
            BasicType::Long => Value::Long(n as i64),
            BasicType::Single => Value::Single(n),
            BasicType::Double => Value::Double(n),
            _ => unreachable!("make_numeric called with non-numeric type"),
        })
    }

    fn resolve_decl_type(decl: &DimDecl) -> BasicType {
        if let Some(ref t) = decl.as_type {
            t.clone()
        } else if let Some(s) = decl.suffix {
            s.to_basic_type()
        } else {
            BasicType::Single
        }
    }

    /// Return the default value for a variable considering DEFtype map
    fn default_for_var(&self, name: &str, suffix: Option<TypeSuffix>) -> Value {
        if suffix.is_some() {
            return Value::default_for_suffix(suffix);
        }
        // Check DEFtype map based on first letter
        if let Some(first_char) = name.chars().next() {
            if first_char.is_ascii_alphabetic() {
                let idx = (first_char.to_ascii_uppercase() as u8 - b'A') as usize;
                if let Some(ref basic_type) = self.deftype_map[idx] {
                    return Value::default_for(basic_type.clone());
                }
            }
        }
        Value::default_for_suffix(None)
    }

    fn resolve_chain_path(&self, path_str: &str) -> String {
        let path = std::path::Path::new(path_str);
        if path.is_absolute() {
            path_str.to_string()
        } else if let Some(ref dir) = self.source_dir {
            dir.join(path).to_string_lossy().into_owned()
        } else {
            path_str.to_string()
        }
    }

    fn chain_loop(
        &mut self,
        mut filespec: String,
        mut common_values: Vec<(CommonVarSpec, CommonTransferValue)>,
    ) -> Result<(), RuntimeError> {
        loop {
            // Canonicalize first to resolve the path once, then read using the resolved path
            let read_path = if let Ok(canonical) = std::fs::canonicalize(&filespec) {
                self.source_dir = canonical.parent().map(|p| p.to_path_buf());
                canonical
            } else {
                std::path::PathBuf::from(&filespec)
            };

            let source = std::fs::read_to_string(&read_path).map_err(|e| {
                RuntimeError::General {
                    msg: format!("CHAIN error: cannot open '{}': {}", filespec, e),
                }
            })?;

            // Lex and parse
            let tokens = crate::lexer::Lexer::new(&source).tokenize().map_err(|e| {
                RuntimeError::General {
                    msg: format!("CHAIN error in '{}': {}", filespec, e),
                }
            })?;
            let program =
                crate::parser::Parser::new(tokens)
                    .parse_program()
                    .map_err(|e| RuntimeError::General {
                        msg: format!("CHAIN error in '{}': {}", filespec, e),
                    })?;

            self.reset_program_state();

            // Prescan the new program
            self.prescan(&program.statements);

            // Map incoming common values to the new program's COMMON declarations by position
            for i in 0..self.common_declarations.len() {
                let (spec, key) = &self.common_declarations[i];
                let as_type = &spec.as_type;
                let is_shared = spec.is_shared;
                let key = key.clone();

                let mut env = self.env.borrow_mut();
                if let Some((_, transfer)) = common_values.get(i) {
                    match transfer {
                        CommonTransferValue::Scalar(value) => {
                            let final_value = if let Some(target_type) = as_type {
                                value.coerce_to_type(target_type)
                            } else {
                                value.clone()
                            };
                            env.set_by_key(&key, final_value);
                        }
                        CommonTransferValue::Array(elements) => {
                            // Remap array element keys from old name to new name
                            if let Some(first) = elements.first() {
                                let old_prefix = find_array_prefix(&first.0);
                                let new_prefix = format!("{}_", key);
                                for (old_key, val) in elements {
                                    if old_key.starts_with(old_prefix) {
                                        let index_part = &old_key[old_prefix.len()..];
                                        let new_key =
                                            format!("{}{}", new_prefix, index_part);
                                        env.set_by_key(&new_key, val.clone());
                                    }
                                }
                            }
                        }
                    }
                } else if let Some(target_type) = as_type {
                    // No incoming value — initialize to the declared type's default
                    env.set_by_key(&key, Value::default_for_type(Some(target_type)));
                }

                if is_shared {
                    env.shared_vars.insert(key);
                }
            }

            // Execute the new program
            let cf = self.exec_block(&program.statements)?;

            match cf {
                ControlFlow::Chain {
                    filespec: next_file,
                    common_values: next_vals,
                } => {
                    filespec = next_file;
                    common_values = next_vals;
                    // Loop continues
                }
                _ => return Ok(()),
            }
        }
    }

    /// Reset interpreter state for CHAIN. Preserves file handles, I/O, and RNG.
    fn reset_program_state(&mut self) {
        self.env = Environment::new_global();
        self.subs.clear();
        self.functions.clear();
        self.def_fns.clear();
        self.data_values.clear();
        self.data_pos = 0;
        self.error_handler = None;
        self.current_error = None;
        self.error_resume_pc = None;
        self.in_error_handler = false;
        self.static_vars.clear();
        self.current_static_vars.clear();
        self.deftype_map = std::array::from_fn(|_| None);
        self.type_defs.clear();
        self.array_type_map.clear();
        self.common_declarations.clear();
    }

    /// Build a flattened key for array element access (temporary hack until
    /// proper array storage is implemented).
    fn array_key(name: &str, suffix: Option<TypeSuffix>, indices: &[i64]) -> String {
        let idx_part: String = indices
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("_");
        match suffix {
            Some(s) => format!("{}{}_{}", name, s.to_char(), idx_part),
            None => format!("{}_{}", name, idx_part),
        }
    }

    fn get_or_init_array_element(
        &mut self,
        name: &str,
        suffix: Option<TypeSuffix>,
        key: &str,
    ) -> Result<Value, RuntimeError> {
        if let Some(v) = self.env.borrow().get(key, None) {
            return Ok(v);
        }
        if let Some(type_name) = self.array_type_map.get(name).cloned() {
            let record = self.create_default_record(&type_name)?;
            self.env.borrow_mut().set(key, None, record.clone());
            Ok(record)
        } else {
            Ok(Value::default_for_suffix(suffix))
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
            Expr::Variable(var) => Environment::var_key(&var.name, var.suffix),
            Expr::ArrayIndex { name, suffix, indices } => {
                let idx_vals: Vec<i64> = indices
                    .iter()
                    .map(|e| self.eval_expr(e).and_then(|v| v.to_i64()))
                    .collect::<Result<Vec<_>, _>>()?;
                Self::array_key(name, *suffix, &idx_vals)
            }
            _ => {
                return Err(RuntimeError::General {
                    msg: "invalid member assignment target".into(),
                });
            }
        };

        // Get or auto-init the root value
        let mut root_val = if let Some(v) = self.env.borrow().get(&root_key, None) {
            v
        } else if let Expr::ArrayIndex { name, suffix, .. } = current {
            self.get_or_init_array_element(name, *suffix, &root_key)?
        } else {
            return Err(RuntimeError::General {
                msg: "variable not initialized".into(),
            });
        };

        // Navigate to the innermost record and set the field
        Self::set_nested_field(&mut root_val, &path, new_val, &self.type_defs)?;

        // Write back
        if let Expr::Variable(var) = current {
            self.env.borrow_mut().set(&var.name, var.suffix, root_val);
        } else {
            self.env.borrow_mut().set(&root_key, None, root_val);
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
                        let coerced = Self::coerce_for_field_static(type_defs, type_name, field_name, new_val)?;
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
        if let Some(fields_def) = type_defs.get(type_name) {
            for f in fields_def {
                if f.name == field_name {
                    if let BasicType::FixedString(n) = f.field_type {
                        let s = val.to_string_val()?;
                        let char_count = s.chars().count();
                        if char_count > n {
                            let byte_end = s.char_indices().nth(n).map(|(i, _)| i).unwrap_or(s.len());
                            return Ok(Value::Str(s[..byte_end].to_string()));
                        } else if char_count < n {
                            let mut padded = s;
                            padded.push_str(&" ".repeat(n - char_count));
                            return Ok(Value::Str(padded));
                        }
                        return Ok(Value::Str(s));
                    }
                    break;
                }
            }
        }
        Ok(val)
    }

    fn eval_format_using(&mut self, fmt_expr: &Expr, items: &[PrintItem]) -> Result<String, RuntimeError> {
        let fmt_str = self.eval_expr(fmt_expr)?.to_string_val()?;
        let mut vals = Vec::new();
        for item in items {
            if let PrintItem::Expr(expr) = item {
                vals.push(self.eval_expr(expr)?);
            }
        }
        crate::format_using::format_using(&fmt_str, &vals)
    }

    fn get_error_value(&self, name: &str) -> Value {
        match name {
            "ERR" => Value::Integer(self.current_error.as_ref().map_or(0, |e| e.err_code) as i64),
            "ERL" => Value::Integer(self.current_error.as_ref().map_or(0, |e| e.err_line) as i64),
            _ => Value::Integer(0),
        }
    }

    /// Write visible text to output and update screen buffer.
    fn write_text(&mut self, text: &str) {
        write!(self.output, "{}", text).ok();
        for ch in text.bytes() {
            if ch == b'\n' {
                self.print_col = 0;
                self.print_row += 1;
            } else if ch == b'\r' {
                self.print_col = 0;
            } else {
                let row = self.print_row.saturating_sub(1);
                let col = self.print_col;
                if row < self.screen_buffer.len() && col < self.screen_buffer[row].len() {
                    self.screen_buffer[row][col] = ch;
                }
                self.print_col += 1;
            }
        }
    }

    /// Map QBasic foreground color index (0–15) to ANSI SGR code.

    /// Non-blocking read of a single keypress. Returns "" if no key available.
    /// In non-interactive mode (tests, piped input), always returns "".
    fn read_inkey(&mut self) -> Result<String, RuntimeError> {
        if !self.interactive {
            return Ok(String::new());
        }
        use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
        use std::time::Duration;
        crossterm::terminal::enable_raw_mode().map_err(|e| RuntimeError::General {
            msg: format!("INKEY$: failed to enable raw mode: {e}"),
        })?;
        let result = if event::poll(Duration::ZERO).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(KeyEvent { code, modifiers, .. })) => {
                    match code {
                        KeyCode::Char(c) => {
                            if modifiers.contains(KeyModifiers::CONTROL) {
                                // Ctrl+C → CHR$(3), etc.
                                let ctrl = (c as u8).wrapping_sub(b'a').wrapping_add(1);
                                String::from(ctrl as char)
                            } else {
                                String::from(c)
                            }
                        }
                        KeyCode::Enter => String::from('\r'),
                        KeyCode::Esc => String::from(27 as char),
                        KeyCode::Backspace => String::from(8 as char),
                        KeyCode::Tab => String::from(9 as char),
                        // Extended keys: CHR$(0) + scan code
                        KeyCode::Up => format!("\0H"),
                        KeyCode::Down => format!("\0P"),
                        KeyCode::Left => format!("\0K"),
                        KeyCode::Right => format!("\0M"),
                        KeyCode::Home => format!("\0G"),
                        KeyCode::End => format!("\0O"),
                        KeyCode::PageUp => format!("\0I"),
                        KeyCode::PageDown => format!("\0Q"),
                        KeyCode::Insert => format!("\0R"),
                        KeyCode::Delete => format!("\0S"),
                        KeyCode::F(n) if n >= 1 && n <= 10 => {
                            // F1=59, F2=60, ..., F10=68
                            format!("\0{}", (58 + n) as char)
                        }
                        _ => String::new(),
                    }
                }
                _ => String::new(),
            }
        } else {
            String::new()
        };
        crossterm::terminal::disable_raw_mode().ok();
        Ok(result)
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
                Ok(Value::Str(String::from_utf8_lossy(&buf[..total]).into_owned()))
            }
            2 => {
                let n = args[0].to_i64()?;
                if n < 1 {
                    return Err(RuntimeError::IllegalFunctionCall {
                        msg: "INPUT$ count must be >= 1".to_string(),
                    });
                }
                let fnum = args[1].to_i64()?;
                let fh = self.file_handles.get_mut(&fnum).ok_or_else(|| RuntimeError::General {
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
                Ok(Value::Str(String::from_utf8_lossy(&buf[..total]).into_owned()))
            }
            _ => Err(RuntimeError::ArityMismatch { expected: 1, got: args.len() }),
        }
    }

    fn qb_fg_to_ansi(c: u8) -> u8 {
        match c {
            0 => 30,   // Black
            1 => 34,   // Blue
            2 => 32,   // Green
            3 => 36,   // Cyan
            4 => 31,   // Red
            5 => 35,   // Magenta
            6 => 33,   // Brown/Yellow
            7 => 37,   // White
            8 => 90,   // Gray
            9 => 94,   // Light Blue
            10 => 92,  // Light Green
            11 => 96,  // Light Cyan
            12 => 91,  // Light Red
            13 => 95,  // Light Magenta
            14 => 93,  // Yellow
            15 => 97,  // Bright White
            _ => 37,
        }
    }

    /// Map QBasic background color index (0–15) to ANSI SGR code.
    fn qb_bg_to_ansi(c: u8) -> u8 {
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
        let filename = self.eval_expr(&open.filename)?.to_string_val()?;
        let file_num = self.eval_expr(&open.file_num)?.to_i64()?;
        let rec_len = if let Some(expr) = &open.rec_len {
            self.eval_expr(expr)?.to_i64()?
        } else {
            128
        };

        if file_num < 1 || file_num > 255 {
            return Err(RuntimeError::General {
                msg: format!("invalid file number: {file_num}"),
            });
        }
        if self.file_handles.contains_key(&file_num) {
            return Err(RuntimeError::General {
                msg: format!("file #{file_num} is already open"),
            });
        }

        let (reader, writer) = match open.mode {
            FileMode::Input => {
                let f = File::open(&filename).map_err(|e| RuntimeError::General {
                    msg: format!("cannot open '{filename}': {e}"),
                })?;
                (Some(BufReader::new(f)), None)
            }
            FileMode::Output => {
                let f = File::create(&filename).map_err(|e| RuntimeError::General {
                    msg: format!("cannot create '{filename}': {e}"),
                })?;
                (None, Some(BufWriter::new(f)))
            }
            FileMode::Append => {
                let f = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&filename)
                    .map_err(|e| RuntimeError::General {
                        msg: format!("cannot open '{filename}' for append: {e}"),
                    })?;
                (None, Some(BufWriter::new(f)))
            }
            FileMode::Random | FileMode::Binary => {
                let f = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .open(&filename)
                    .map_err(|e| RuntimeError::General {
                        msg: format!("cannot open '{filename}': {e}"),
                    })?;
                let f2 = f.try_clone().map_err(|e| RuntimeError::General {
                    msg: format!("cannot clone file handle: {e}"),
                })?;
                (Some(BufReader::new(f)), Some(BufWriter::new(f2)))
            }
        };

        self.file_handles.insert(file_num, FileHandle {
            mode: open.mode,
            reader,
            writer,
            rec_len,
            eof_flag: false,
        });

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
                if let Some(fh) = self.file_handles.remove(&n) {
                    if let Some(mut w) = fh.writer {
                        let _ = w.flush();
                    }
                }
            }
        }
        Ok(())
    }

    fn exec_file_print(&mut self, pf: &FilePrintStmt) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(&pf.file_num)?.to_i64()?;

        // Handle PRINT #n, USING
        if let Some(ref fmt_expr) = pf.format {
            let result = self.eval_format_using(fmt_expr, &pf.items)?;
            let trailing = pf.trailing;

            let fh = self.file_handles.get_mut(&file_num).ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open"),
            })?;
            let writer = fh.writer.as_mut().ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open for writing"),
            })?;

            let _ = write!(writer, "{}", result);
            match trailing {
                PrintSep::Newline => { let _ = writeln!(writer); }
                PrintSep::Semicolon => {}
                PrintSep::Comma => { let _ = write!(writer, "\t"); }
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

        let fh = self.file_handles.get_mut(&file_num).ok_or_else(|| RuntimeError::General {
            msg: format!("file #{file_num} is not open"),
        })?;
        let writer = fh.writer.as_mut().ok_or_else(|| RuntimeError::General {
            msg: format!("file #{file_num} is not open for writing"),
        })?;

        for part in &parts {
            let _ = write!(writer, "{}", part);
        }
        match trailing {
            PrintSep::Newline => { let _ = writeln!(writer); }
            PrintSep::Semicolon => {}
            PrintSep::Comma => { let _ = write!(writer, "\t"); }
        }

        Ok(())
    }

    fn exec_file_write(&mut self, wf: &FileWriteStmt) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(&wf.file_num)?.to_i64()?;

        // Evaluate all expressions first
        let vals: Vec<Value> = wf.exprs
            .iter()
            .map(|e| self.eval_expr(e))
            .collect::<Result<Vec<_>, _>>()?;

        let fh = self.file_handles.get_mut(&file_num).ok_or_else(|| RuntimeError::General {
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
                Value::Str(s) => { let _ = write!(writer, "\"{}\"", s); }
                _ => { let _ = write!(writer, "{}", val.format_for_write()); }
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
            let fh = self.file_handles.get_mut(&file_num).ok_or_else(|| RuntimeError::General {
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
            let val = if matches!(var.suffix, Some(TypeSuffix::String)) {
                Value::Str(field)
            } else if let Ok(n) = field.parse::<i64>() {
                Value::Integer(n)
            } else if let Ok(n) = field.parse::<f64>() {
                Value::Double(n)
            } else {
                Value::Str(field)
            };
            self.env.borrow_mut().set(&var.name, var.suffix, val);
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
                b' ' | b'\t' => { reader.consume(1); }
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
            // Consume trailing comma or newline
            let buf = reader.fill_buf().unwrap_or(&[]);
            if !buf.is_empty() && (buf[0] == b',' || buf[0] == b'\r' || buf[0] == b'\n') {
                if buf[0] == b',' {
                    reader.consume(1);
                }
                // newlines consumed at start of next field read
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
            let fh = self.file_handles.get_mut(&file_num).ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open"),
            })?;
            let reader = fh.reader.as_mut().ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open for reading"),
            })?;

            let mut line = String::new();
            let bytes_read = reader.read_line(&mut line).map_err(|e| RuntimeError::General {
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

        self.env
            .borrow_mut()
            .set(&var.name, var.suffix, Value::Str(line));
        Ok(())
    }

    fn exec_get_put(&mut self, gp: &GetPutStmt) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(&gp.file_num)?.to_i64()?;
        let record = if let Some(expr) = &gp.record {
            Some(self.eval_expr(expr)?.to_i64()?)
        } else {
            None
        };

        let fh = self.file_handles.get_mut(&file_num).ok_or_else(|| RuntimeError::General {
            msg: format!("file #{file_num} is not open"),
        })?;

        let rec_len = fh.rec_len;

        if gp.is_get {
            // Seek if record specified
            if let Some(rec) = record {
                let pos = (rec - 1) * rec_len;
                if let Some(reader) = &mut fh.reader {
                    reader.seek(SeekFrom::Start(pos as u64)).map_err(|e| RuntimeError::General {
                        msg: format!("seek error: {e}"),
                    })?;
                }
            }

            // Read rec_len bytes
            let reader = fh.reader.as_mut().ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open for reading"),
            })?;
            let mut buf = vec![0u8; rec_len as usize];
            let bytes_read = reader.read(&mut buf).unwrap_or(0);
            if bytes_read == 0 {
                fh.eof_flag = true;
            }
            buf.truncate(bytes_read);

            // Trim trailing nulls/spaces for string representation
            let s = String::from_utf8_lossy(&buf).trim_end_matches('\0').to_string();

            if let Some(var) = &gp.var {
                self.env.borrow_mut().set(&var.name, var.suffix, Value::Str(s));
            }
        } else {
            // PUT
            // Seek if record specified
            if let Some(rec) = record {
                let pos = (rec - 1) * rec_len;
                if let Some(writer) = &mut fh.writer {
                    writer.seek(SeekFrom::Start(pos as u64)).map_err(|e| RuntimeError::General {
                        msg: format!("seek error: {e}"),
                    })?;
                }
            }

            let writer = fh.writer.as_mut().ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open for writing"),
            })?;

            if let Some(var) = &gp.var {
                let val = self.env.borrow().get(&var.name, var.suffix)
                    .unwrap_or(Value::Str(String::new()));
                let s = match val {
                    Value::Str(s) => s,
                    other => other.format_for_write(),
                };
                let bytes = s.as_bytes();
                // Pad to rec_len for RANDOM mode
                if fh.mode == FileMode::Random {
                    let mut padded = vec![0u8; rec_len as usize];
                    let copy_len = bytes.len().min(rec_len as usize);
                    padded[..copy_len].copy_from_slice(&bytes[..copy_len]);
                    let _ = writer.write_all(&padded);
                } else {
                    let _ = writer.write_all(bytes);
                }
            }
        }

        Ok(())
    }
}
