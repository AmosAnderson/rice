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
use crate::value::{Value, basic_string_to_bytes, bytes_to_basic_string};

enum ControlFlow {
    Normal,
    ExitFor,
    ExitDo,
    ExitSub,
    ExitFunction,
    Goto(Label),
    Return,
    End,
    Retry,
    Continue,
    Resume(crate::ast::ResumeKind),
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
    print_col: usize,
    reader: Option<BufReader<File>>,
    writer: Option<BufWriter<File>>,
    eof_flag: bool,
    record_len: Option<usize>,
    field_layout: Vec<FieldBinding>,
    field_buffer: Vec<u8>,
}

#[derive(Clone)]
struct FieldBinding {
    name: String,
    offset: usize,
    width: usize,
}

#[derive(Clone)]
struct LabelTarget {
    statements: Rc<Vec<LabeledStmt>>,
    index: usize,
    trap_errors: bool,
}

pub struct Interpreter {
    pub dialect: crate::Dialect,
    gosub_targets: HashMap<String, LabelTarget>,
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
    /// DEFSTR letters: untyped variables starting with these default to STRING.
    def_str_letters: [bool; 26],
    /// Letters covered by any DEFtype statement (numeric or string). Used for OPTION EXPLICIT.
    def_typed_letters: [bool; 26],
    /// True when OPTION EXPLICIT has been executed; undeclared variable references become errors.
    option_explicit: bool,
    /// Overrides for DATE$ and TIME$ pseudo-variables (set by assignment).
    date_override: Option<String>,
    time_override: Option<String>,
    /// ENVIRON changes are local to this interpreter and inherited by SHELL.
    environment_overrides: HashMap<String, String>,
    /// Classic ON ERROR handler target (None = no handler / disabled).
    on_error_label: Option<Label>,
    /// Most recent BASIC error code (ERR) and line (ERL).
    err_code: i32,
    err_line: usize,
    /// Top-level statement that can be retried/resumed after an ON ERROR transfer.
    err_resume_pc: Option<usize>,
    /// Prevents recursively trapping errors raised while an ON ERROR handler is active.
    handling_error: bool,
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
            dialect: crate::DEFAULT_DIALECT,
            gosub_targets: HashMap::new(),
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
            def_str_letters: [false; 26],
            def_typed_letters: [false; 26],
            option_explicit: false,
            date_override: None,
            time_override: None,
            environment_overrides: HashMap::new(),
            on_error_label: None,
            err_code: 0,
            err_line: 0,
            err_resume_pc: None,
            handling_error: false,
        }
    }

    pub fn run_source(&mut self, source: &str) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(detected) = crate::detect_dialect(source) {
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

        let previous_targets = std::mem::replace(
            &mut self.gosub_targets,
            Self::collect_label_targets(&program.statements, true),
        );
        let result = self.exec_top_level(&program.statements);
        self.gosub_targets = previous_targets;
        match result? {
            ControlFlow::Normal | ControlFlow::End => Ok(()),
            ControlFlow::Goto(label) => Err(RuntimeError::UndefinedLabel {
                label: label.to_string(),
            }),
            _ => Err(RuntimeError::General {
                msg: "control-flow statement outside its enclosing construct".into(),
            }),
        }
    }

    fn prescan(&mut self, stmts: &[LabeledStmt]) {
        for ls in stmts {
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
        self.exec_block_from(stmts, 0)
    }

    fn exec_block_from(
        &mut self,
        stmts: &[LabeledStmt],
        mut pc: usize,
    ) -> Result<ControlFlow, RuntimeError> {
        while pc < stmts.len() {
            let ls = &stmts[pc];
            let cf = self.exec_stmt(&ls.stmt)?;
            match cf {
                ControlFlow::Normal => pc += 1,
                ControlFlow::Goto(ref label) => {
                    if let Some(idx) = Self::label_index(stmts, label) {
                        pc = idx;
                    } else {
                        return Ok(cf);
                    }
                }
                other => return Ok(other),
            }
        }
        Ok(ControlFlow::Normal)
    }

    fn label_index(stmts: &[LabeledStmt], label: &Label) -> Option<usize> {
        stmts
            .iter()
            .position(|stmt| stmt.label.as_ref() == Some(label))
    }

    /// Retain statement blocks for labels in this procedure, excluding other procedures.
    fn collect_label_targets(
        stmts: &[LabeledStmt],
        trap_errors: bool,
    ) -> HashMap<String, LabelTarget> {
        fn collect(
            stmts: &[LabeledStmt],
            targets: &mut HashMap<String, LabelTarget>,
            trap_errors: bool,
        ) {
            if stmts.iter().any(|stmt| stmt.label.is_some()) {
                let block = Rc::new(stmts.to_vec());
                for (index, stmt) in stmts.iter().enumerate() {
                    if let Some(label) = &stmt.label {
                        targets
                            .entry(label.to_string())
                            .or_insert_with(|| LabelTarget {
                                statements: block.clone(),
                                index,
                                trap_errors,
                            });
                    }
                }
            }
            for stmt in stmts {
                match &stmt.stmt {
                    Stmt::If(if_stmt) => {
                        collect(&if_stmt.then_body, targets, false);
                        for (_, body) in &if_stmt.elseif_clauses {
                            collect(body, targets, false);
                        }
                        if let Some(body) = &if_stmt.else_body {
                            collect(body, targets, false);
                        }
                    }
                    Stmt::For(for_stmt) => collect(&for_stmt.body, targets, false),
                    Stmt::WhileWend { body, .. } => collect(body, targets, false),
                    Stmt::DoLoop(do_stmt) => collect(&do_stmt.body, targets, false),
                    Stmt::SelectCase(select) => {
                        for case in &select.cases {
                            collect(&case.body, targets, false);
                        }
                        if let Some(body) = &select.else_body {
                            collect(body, targets, false);
                        }
                    }
                    Stmt::WhenException { body, handler } => {
                        collect(body, targets, false);
                        collect(handler, targets, false);
                    }
                    _ => {}
                }
            }
        }

        let mut targets = HashMap::new();
        collect(stmts, &mut targets, trap_errors);
        targets
    }

    fn exec_gosub(&mut self, label: &Label) -> Result<ControlFlow, RuntimeError> {
        let mut target = label.clone();
        loop {
            let destination = self
                .gosub_targets
                .get(&target.to_string())
                .cloned()
                .ok_or_else(|| RuntimeError::UndefinedLabel {
                    label: target.to_string(),
                })?;
            // Keep the caller's nested blocks alive until this invocation returns.
            let result = if destination.trap_errors {
                self.exec_top_level_from(&destination.statements, destination.index, true)?
            } else {
                self.exec_block_from(&destination.statements, destination.index)?
            };
            match result {
                ControlFlow::Return => return Ok(ControlFlow::Normal),
                ControlFlow::Goto(label) => target = label,
                // Falling off the program ends execution; it is not an implicit RETURN.
                ControlFlow::Normal => return Ok(ControlFlow::End),
                other => return Ok(other),
            }
        }
    }

    /// Execute the top-level statement block with GOTO handling.
    fn exec_top_level(&mut self, stmts: &[LabeledStmt]) -> Result<ControlFlow, RuntimeError> {
        self.exec_top_level_from(stmts, 0, false)
    }

    fn exec_top_level_from(
        &mut self,
        stmts: &[LabeledStmt],
        mut pc: usize,
        is_gosub: bool,
    ) -> Result<ControlFlow, RuntimeError> {
        while pc < stmts.len() {
            let ls = &stmts[pc];
            let cf = match self.exec_stmt(&ls.stmt) {
                Ok(cf) => cf,
                Err(err) => {
                    if self.handling_error {
                        return Err(err);
                    }
                    if let Some(handler) = self.on_error_label.clone() {
                        self.err_code = err.basic_err_code();
                        self.err_line = Self::erl_for_stmt(ls);
                        self.err_resume_pc = Some(pc);
                        self.handling_error = true;
                        let resolved = Self::label_index(stmts, &handler);
                        if let Some(idx) = resolved {
                            pc = idx;
                            continue;
                        }
                        return Err(RuntimeError::UndefinedLabel {
                            label: handler.to_string(),
                        });
                    }
                    return Err(err);
                }
            };
            match cf {
                ControlFlow::Normal => {
                    pc += 1;
                }
                ControlFlow::Goto(label) => {
                    let resolved = Self::label_index(stmts, &label);
                    if let Some(idx) = resolved {
                        pc = idx;
                    } else {
                        return Ok(ControlFlow::Goto(label));
                    }
                }
                ControlFlow::Return => {
                    if is_gosub {
                        return Ok(ControlFlow::Return);
                    }
                    return Err(RuntimeError::General {
                        msg: "RETURN without GOSUB".into(),
                    });
                }
                ControlFlow::Resume(kind) => {
                    pc = self.resolve_resume_pc(&kind, stmts)?;
                    self.handling_error = false;
                    self.err_resume_pc = None;
                }
                other => return Ok(other),
            }
        }
        Ok(ControlFlow::Normal)
    }

    fn erl_for_stmt(ls: &LabeledStmt) -> usize {
        match &ls.label {
            Some(Label::Number(n)) => *n as usize,
            _ => 0,
        }
    }

    fn resolve_resume_pc(
        &self,
        kind: &ResumeKind,
        stmts: &[LabeledStmt],
    ) -> Result<usize, RuntimeError> {
        let error_pc = self.err_resume_pc.ok_or_else(|| RuntimeError::General {
            msg: "RESUME without error".into(),
        })?;
        match kind {
            ResumeKind::Same => Ok(error_pc),
            ResumeKind::Next => Ok(error_pc + 1),
            ResumeKind::Label(label) => {
                Self::label_index(stmts, label).ok_or_else(|| RuntimeError::UndefinedLabel {
                    label: label.to_string(),
                })
            }
        }
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
                self.require_writable(name)?;
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
            Stmt::MidAssign {
                name,
                start,
                len,
                expr,
            } => {
                self.require_writable(name)?;
                let start_i = self.eval_expr(start)?.to_i64()?;
                if start_i < 1 {
                    return Err(RuntimeError::IllegalFunctionCall {
                        msg: "MID$ start must be >= 1".into(),
                    });
                }
                let repl = self.eval_expr(expr)?.to_string_val()?;
                let s = self
                    .env
                    .borrow()
                    .get(name)
                    .unwrap_or(Value::Str(String::new()))
                    .to_string_val()?;
                let mut chars: Vec<char> = s.chars().collect();
                let start_idx = (start_i - 1) as usize;
                // Number of characters to overwrite: min(len, repl.len, remaining)
                let max_len = chars.len().saturating_sub(start_idx);
                let n = match len {
                    Some(e) => {
                        let l = self.eval_expr(e)?.to_i64()?;
                        if l < 0 {
                            return Err(RuntimeError::IllegalFunctionCall {
                                msg: "MID$ length must be non-negative".into(),
                            });
                        }
                        (l as usize).min(max_len)
                    }
                    None => max_len,
                };
                let repl_chars: Vec<char> = repl.chars().collect();
                for (i, rc) in repl_chars.iter().take(n).enumerate() {
                    chars[start_idx + i] = *rc;
                }
                let new_s: String = chars.into_iter().collect();
                self.env.borrow_mut().set(name, Value::Str(new_s));
                Ok(ControlFlow::Normal)
            }
            Stmt::Dim { decls, shared } => self.exec_dim(decls, *shared),
            Stmt::Const { name, value } => {
                self.env.borrow_mut().declare_var(name);
                let val = self.eval_expr(value)?;
                self.env.borrow_mut().define_const(name, val)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::OptionExplicit => {
                self.option_explicit = true;
                Ok(ControlFlow::Normal)
            }
            Stmt::Input(input) => {
                self.exec_input(input)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::LineInput { prompt, var } => {
                self.require_writable(&var.name)?;
                if let Some(p) = prompt {
                    write!(self.output, "{}", p)
                        .map_err(|e| RuntimeError::from_io("LINE INPUT", e))?;
                    self.output
                        .flush()
                        .map_err(|e| RuntimeError::from_io("LINE INPUT", e))?;
                }
                let mut line = String::new();
                if self
                    .input
                    .read_line(&mut line)
                    .map_err(|e| RuntimeError::from_io("LINE INPUT", e))?
                    == 0
                {
                    return Err(RuntimeError::BasicError { code: 62 });
                }
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
            Stmt::Gosub(label) => self.exec_gosub(label),
            Stmt::Return => Ok(ControlFlow::Return),
            Stmt::OnGoto {
                expr,
                labels,
                is_gosub,
            } => self.exec_on_goto(expr, labels, *is_gosub),
            Stmt::OnError { label } => {
                self.on_error_label = label.clone();
                if label.is_none() {
                    self.err_code = 0;
                    self.err_line = 0;
                    self.err_resume_pc = None;
                    self.handling_error = false;
                }
                Ok(ControlFlow::Normal)
            }
            Stmt::Resume(kind) => Ok(ControlFlow::Resume(kind.clone())),
            Stmt::ErrorStmt(expr) => {
                let code = self.eval_expr(expr)?.to_i64()?;
                if !(1..=255).contains(&code) {
                    return Err(RuntimeError::IllegalFunctionCall {
                        msg: format!("ERROR code {code} out of range"),
                    });
                }
                Err(RuntimeError::BasicError { code: code as i32 })
            }
            Stmt::ExitFor => Ok(ControlFlow::ExitFor),
            Stmt::ExitDo => Ok(ControlFlow::ExitDo),
            Stmt::ExitSub => Ok(ControlFlow::ExitSub),
            Stmt::ExitFunction => Ok(ControlFlow::ExitFunction),
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
                self.require_writable(&a.name)?;
                self.require_writable(&b.name)?;
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
            Stmt::Field { file_num, fields } => {
                self.exec_field(file_num, fields)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::LSetRSet {
                var,
                expr,
                right_align,
            } => {
                self.exec_lset_rset(var, expr, *right_align)?;
                Ok(ControlFlow::Normal)
            }
            Stmt::Seek { file_num, position } => {
                let fnum = self.eval_expr(file_num)?.to_i64()?;
                let pos = self.eval_expr(position)?.to_i64()?;
                if pos < 1 {
                    return Err(RuntimeError::IllegalFunctionCall {
                        msg: "SEEK position must be >= 1".into(),
                    });
                }
                let byte_pos = (pos - 1) as u64;
                let fh = self
                    .file_handles
                    .get_mut(&fnum)
                    .ok_or_else(|| RuntimeError::General {
                        msg: format!("file #{fnum} is not open"),
                    })?;
                if let Some(reader) = &mut fh.reader {
                    reader
                        .seek(SeekFrom::Start(byte_pos))
                        .map_err(|e| RuntimeError::General {
                            msg: format!("SEEK error: {e}"),
                        })?;
                }
                if let Some(writer) = &mut fh.writer {
                    writer
                        .seek(SeekFrom::Start(byte_pos))
                        .map_err(|e| RuntimeError::General {
                            msg: format!("SEEK error: {e}"),
                        })?;
                }
                fh.eof_flag = false;
                Ok(ControlFlow::Normal)
            }
            Stmt::Reset => {
                for (_, mut fh) in self.file_handles.drain() {
                    if let Some(mut w) = fh.writer.take() {
                        let _ = w.flush();
                    }
                }
                Ok(ControlFlow::Normal)
            }
            Stmt::DefType { is_string, ranges } => {
                for (start, end) in ranges {
                    let (s, e) = (*start as u8, *end as u8);
                    for c in s..=e {
                        if c.is_ascii_uppercase() {
                            self.def_str_letters[(c - b'A') as usize] = *is_string;
                            self.def_typed_letters[(c - b'A') as usize] = true;
                        }
                    }
                }
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
            Stmt::Environ(expr) => {
                let s = self.eval_expr(expr)?.to_string_val()?;
                if let Some(pos) = s.find('=') {
                    let name = &s[..pos];
                    let value = &s[pos + 1..];
                    if name.is_empty() {
                        return Err(RuntimeError::IllegalFunctionCall {
                            msg: "ENVIRON: empty variable name".into(),
                        });
                    }
                    if name.contains('\0') || value.contains('\0') {
                        return Err(RuntimeError::IllegalFunctionCall {
                            msg: "ENVIRON: name and value must not contain NUL".into(),
                        });
                    }
                    self.environment_overrides
                        .insert(Self::environment_key(name), value.to_string());
                } else {
                    return Err(RuntimeError::IllegalFunctionCall {
                        msg: "ENVIRON: expected \"name=value\"".into(),
                    });
                }
                Ok(ControlFlow::Normal)
            }
            Stmt::DateAssign(expr) => {
                let s = self.eval_expr(expr)?.to_string_val()?;
                Self::validate_date_format(&s)?;
                self.date_override = Some(s);
                Ok(ControlFlow::Normal)
            }
            Stmt::TimeAssign(expr) => {
                let s = self.eval_expr(expr)?.to_string_val()?;
                Self::validate_time_format(&s)?;
                self.time_override = Some(s);
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

            Stmt::Chdrive(expr) => {
                let drive = self.eval_expr(expr)?.to_string_val()?;
                if let Some(letter) = drive.chars().next() {
                    let root = format!("{}:\\", letter);
                    std::env::set_current_dir(&root)
                        .map_err(|e| RuntimeError::from_io("CHDRIVE", e))?;
                }
                Ok(ControlFlow::Normal)
            }

            Stmt::Files(expr) => {
                let pattern = match expr {
                    Some(e) => self.eval_expr(e)?.to_string_val()?,
                    None => ".".to_string(),
                };
                let dir = if pattern.is_empty() {
                    ".".to_string()
                } else {
                    pattern
                };
                let path = std::path::Path::new(&dir);
                let read_dir = if path.is_dir() {
                    std::fs::read_dir(path)
                } else {
                    std::fs::read_dir(
                        path.parent()
                            .filter(|p| !p.as_os_str().is_empty())
                            .unwrap_or(std::path::Path::new(".")),
                    )
                };
                let entries = read_dir.map_err(|e| RuntimeError::from_io("FILES", e))?;
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    self.write_text(&name)?;
                    self.write_text("\n")?;
                }
                Ok(ControlFlow::Normal)
            }

            Stmt::Shell(expr) => {
                if let Some(e) = expr {
                    let cmd = self.eval_expr(e)?.to_string_val()?;
                    #[cfg(target_os = "windows")]
                    let result = std::process::Command::new("cmd")
                        .args(["/c", &cmd])
                        .envs(&self.environment_overrides)
                        .status();
                    #[cfg(not(target_os = "windows"))]
                    let result = std::process::Command::new("sh")
                        .args(["-c", &cmd])
                        .envs(&self.environment_overrides)
                        .status();
                    result.map_err(|e| RuntimeError::General {
                        msg: format!("SHELL error: {}", e),
                    })?;
                }
                Ok(ControlFlow::Normal)
            }

            Stmt::Shared(vars) => {
                for var in vars {
                    self.env.borrow_mut().declare_var(&var.name);
                    self.env.borrow_mut().shared_vars.insert(var.name.clone());
                }
                Ok(ControlFlow::Normal)
            }
            Stmt::Common { names, shared: _ } => {
                // CHAIN is not supported, so plain COMMON and COMMON SHARED are
                // treated identically: both declare module-level variables that are
                // visible inside procedures. The parsed `shared` flag is retained for
                // syntax fidelity but does not change runtime behavior.
                for name in names {
                    self.env.borrow_mut().declare_var(name);
                    self.env.borrow_mut().shared_vars.insert(name.clone());
                }
                Ok(ControlFlow::Normal)
            }

            Stmt::Static(decls) => {
                // Mark variables as static and initialize with defaults if not already loaded
                for decl in decls {
                    self.env.borrow_mut().declare_var(&decl.name);
                    if self.env.borrow().get(&decl.name).is_none() {
                        let default = match Self::resolve_decl_type(decl) {
                            BasicType::UserDefined(name) => self.create_default_record(&name)?,
                            ty => Value::default_for(ty),
                        };
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
                let width = match columns {
                    Some(expr) => self.eval_expr(expr)?.to_i64()?,
                    None => self.screen_width as i64,
                };
                let height = match rows {
                    Some(expr) => self.eval_expr(expr)?.to_i64()?,
                    None => self.screen_height as i64,
                };
                if width < 1 || height < 1 {
                    return Err(RuntimeError::IllegalFunctionCall {
                        msg: "WIDTH columns and rows must be positive".into(),
                    });
                }
                self.screen_width = width as usize;
                self.screen_height = height as usize;
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
            self.require_writable(name)?;
            self.env.borrow_mut().set(&key, val);
            return Ok(ControlFlow::Normal);
        }
        self.require_writable(&var.name)?;
        let val = self.eval_expr(expr)?;
        self.env.borrow_mut().set(&var.name, val);
        Ok(ControlFlow::Normal)
    }

    fn exec_dim(&mut self, decls: &[DimDecl], shared: bool) -> Result<ControlFlow, RuntimeError> {
        for decl in decls {
            self.env.borrow_mut().declare_var(&decl.name);
            if shared {
                self.env.borrow_mut().shared_vars.insert(decl.name.clone());
            }
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
                    if lower > upper
                        || upper
                            .checked_sub(lower)
                            .and_then(|span| span.checked_add(1))
                            .is_none()
                    {
                        return Err(RuntimeError::SubscriptOutOfRange);
                    }
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
            self.require_writable(&var.name)?;
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
            self.env.borrow_mut().declare_var(&decl.name);
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
                    if lower > upper
                        || upper
                            .checked_sub(lower)
                            .and_then(|span| span.checked_add(1))
                            .is_none()
                    {
                        return Err(RuntimeError::SubscriptOutOfRange);
                    }
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
            self.require_declared(name)?;
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
                self.write_text(",")?;
            }
            let val = self.eval_expr(expr)?;
            match &val {
                Value::Str(s) => self.write_text(&format!("\"{}\"", s))?,
                Value::Numeric(n) => {
                    if *n == (*n as i64) as f64 && n.abs() < 1e15 {
                        self.write_text(&format!("{}", *n as i64))?;
                    } else {
                        self.write_text(&format!("{}", n))?;
                    }
                }
                Value::Record { type_name, .. } => {
                    self.write_text(&format!("[{}]", type_name))?;
                }
            };
        }
        self.write_text("\n")?;
        Ok(ControlFlow::Normal)
    }

    fn exec_print(&mut self, ps: &PrintStmt) -> Result<(), RuntimeError> {
        // Handle PRINT USING
        if let Some(ref fmt_expr) = ps.format {
            let result = self.eval_format_using(fmt_expr, &ps.items)?;
            self.write_text(&result)?;
            match ps.trailing {
                PrintSep::Newline => {
                    self.write_text("\n")?;
                }
                PrintSep::Semicolon => {}
                PrintSep::Comma => {
                    let next_zone = ((self.print_col / 16) + 1) * 16;
                    let spaces = next_zone - self.print_col;
                    self.write_text(&" ".repeat(spaces))?;
                }
            }
            self.output
                .flush()
                .map_err(|e| RuntimeError::from_io("output", e))?;
            return Ok(());
        }

        for item in &ps.items {
            match item {
                PrintItem::Expr(expr) => {
                    let val = self.eval_expr(expr)?;
                    let s = val.format_for_print();
                    self.write_text(&s)?;
                }
                PrintItem::Tab(expr) => {
                    let n = self.eval_expr(expr)?.to_i64()?;
                    if n < 0 {
                        return Err(RuntimeError::IllegalFunctionCall {
                            msg: "TAB argument must be non-negative".into(),
                        });
                    }
                    let n = (n as usize).saturating_sub(1);
                    if n > self.print_col {
                        let spaces = n - self.print_col;
                        self.write_text(&" ".repeat(spaces))?;
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
                    self.write_text(&" ".repeat(n))?;
                }
                PrintItem::Comma => {
                    // Advance to the next 16-column zone
                    let next_zone = ((self.print_col / 16) + 1) * 16;
                    let spaces = next_zone - self.print_col;
                    self.write_text(&" ".repeat(spaces))?;
                }
            }
        }
        match ps.trailing {
            PrintSep::Newline => {
                self.write_text("\n")?;
            }
            PrintSep::Semicolon => {}
            PrintSep::Comma => {}
        }
        self.output
            .flush()
            .map_err(|e| RuntimeError::from_io("output", e))?;
        Ok(())
    }

    fn exec_input(&mut self, input: &InputStmt) -> Result<(), RuntimeError> {
        for var in &input.vars {
            self.require_writable(&var.name)?;
        }
        loop {
            if let Some(p) = &input.prompt {
                write!(self.output, "{}? ", p).map_err(|e| RuntimeError::from_io("INPUT", e))?;
            } else {
                write!(self.output, "? ").map_err(|e| RuntimeError::from_io("INPUT", e))?;
            }
            self.output
                .flush()
                .map_err(|e| RuntimeError::from_io("INPUT", e))?;

            let mut line = String::new();
            if self
                .input
                .read_line(&mut line)
                .map_err(|e| RuntimeError::from_io("INPUT", e))?
                == 0
            {
                return Err(RuntimeError::BasicError { code: 62 });
            }
            let line = line.trim_end_matches('\n').trim_end_matches('\r');

            let parts: Vec<&str> = if input.vars.len() == 1 {
                vec![line]
            } else {
                line.split(',').map(|s| s.trim()).collect()
            };

            if parts.len() != input.vars.len() {
                writeln!(self.output, "? Redo from start")
                    .map_err(|e| RuntimeError::from_io("INPUT", e))?;
                continue;
            }

            let mut values = Vec::new();
            for (var, part) in input.vars.iter().zip(parts.iter()) {
                let current = self
                    .env
                    .borrow()
                    .get(&var.name)
                    .unwrap_or_else(|| self.default_for_var(&var.name));
                let val = if matches!(current, Value::Str(_)) {
                    Value::Str(part.to_string())
                } else {
                    if let Ok(n) = part.trim().parse::<f64>()
                        && n.is_finite()
                    {
                        Value::Numeric(n)
                    } else {
                        break;
                    }
                };
                values.push(val);
            }
            if values.len() != input.vars.len() {
                writeln!(self.output, "? Redo from start")
                    .map_err(|e| RuntimeError::from_io("INPUT", e))?;
                continue;
            }
            for (var, val) in input.vars.iter().zip(values) {
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
                self.exec_gosub(label)
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
        self.env.borrow_mut().declare_var(&for_stmt.var.name);
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
                ControlFlow::Normal => {}
                other => return Ok(other),
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
                ControlFlow::Normal => {}
                other => return Ok(other),
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
                ControlFlow::Normal => {}
                other => return Ok(other),
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
                child_env.borrow_mut().declare_var(&param.name);
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
            let previous_targets = std::mem::replace(
                &mut self.gosub_targets,
                Self::collect_label_targets(&sub.body, false),
            );
            self.env = child_env.clone();
            let result = self.exec_block(&sub.body);
            self.env = prev_env;
            self.gosub_targets = previous_targets;

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
                ControlFlow::Normal | ControlFlow::ExitSub => Ok(ControlFlow::Normal),
                ControlFlow::End => Ok(ControlFlow::End),
                ControlFlow::Goto(label) => Err(RuntimeError::UndefinedLabel {
                    label: label.to_string(),
                }),
                ControlFlow::Return => Err(RuntimeError::General {
                    msg: "RETURN without GOSUB in current procedure".into(),
                }),
                _ => Err(RuntimeError::General {
                    msg: "control-flow statement outside its enclosing construct".into(),
                }),
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

    fn require_declared(&self, name: &str) -> Result<(), RuntimeError> {
        if !self.option_explicit {
            return Ok(());
        }
        if self.is_declared(name) {
            return Ok(());
        }
        Err(RuntimeError::General {
            msg: format!("Variable '{}' is not declared (OPTION EXPLICIT)", name),
        })
    }

    fn require_writable(&self, name: &str) -> Result<(), RuntimeError> {
        self.require_declared(name)?;
        if self.env.borrow().is_const(name) {
            return Err(RuntimeError::General {
                msg: format!("cannot assign to constant: {name}"),
            });
        }
        Ok(())
    }

    fn is_declared(&self, name: &str) -> bool {
        {
            let env = self.env.borrow();
            // Explicit declarations in the current scope, or a module-level
            // variable made visible here via SHARED/COMMON, or a constant.
            if env.is_declared(name) || env.is_shared(name) || env.is_const(name) {
                return true;
            }
        }
        // Type suffix declares the variable by use.
        if Self::has_type_suffix(name) {
            return true;
        }
        // DEFtype letter range declares variables starting with that letter.
        if let Some(first) = name.chars().next() {
            let up = first.to_ascii_uppercase();
            if up.is_ascii_alphabetic() && self.def_typed_letters[(up as u8 - b'A') as usize] {
                return true;
            }
        }
        false
    }

    fn has_type_suffix(name: &str) -> bool {
        name.ends_with(['$', '%', '!', '#', '&'])
    }

    fn current_date_value(&self) -> Result<Value, RuntimeError> {
        match &self.date_override {
            Some(date) => Ok(Value::Str(date.clone())),
            None => crate::builtins::builtin_date(&[]),
        }
    }

    fn current_time_value(&self) -> Result<Value, RuntimeError> {
        match &self.time_override {
            Some(time) => Ok(Value::Str(time.clone())),
            None => crate::builtins::builtin_time(&[]),
        }
    }

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
                    if builtin_name == "ERR" {
                        return Ok(Value::Numeric(self.err_code as f64));
                    }
                    if builtin_name == "ERL" {
                        return Ok(Value::Numeric(self.err_line as f64));
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

                    if builtin_name == "DATE$" {
                        return self.current_date_value();
                    }
                    if builtin_name == "TIME$" {
                        return self.current_time_value();
                    }

                    let is_implicit_builtin = matches!(builtin_name.as_str(), "TIMER");
                    if is_implicit_builtin
                        && let Some(result) = self.builtins.call(builtin_name, &[])?
                    {
                        return Ok(result);
                    }

                    // A 0-argument user-defined function may be referenced without parentheses.
                    if let Some(func) = self.functions.get(builtin_name).cloned()
                        && func.params.is_empty()
                    {
                        return self.call_user_function(&func, &[], &[]);
                    }

                    self.require_declared(&var.name)?;
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
                // LBOUND/UBOUND take an array name (not its value) plus an optional dimension.
                if name == "LBOUND" || name == "UBOUND" {
                    return self.eval_array_bound(name, args);
                }
                let arg_vals: Vec<Value> = args
                    .iter()
                    .map(|e| self.eval_expr(e))
                    .collect::<Result<Vec<_>, _>>()?;

                // Stateful functions (need access to interpreter state)
                match name.as_str() {
                    "ENVIRON$" => {
                        if arg_vals.len() != 1 {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 1,
                                got: arg_vals.len(),
                            });
                        }
                        let key = Self::environment_key(&arg_vals[0].to_string_val()?);
                        if let Some(value) = self.environment_overrides.get(&key) {
                            return Ok(Value::Str(value.clone()));
                        }
                    }
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
                    "ERR" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 0,
                                got: arg_vals.len(),
                            });
                        }
                        return Ok(Value::Numeric(self.err_code as f64));
                    }
                    "ERL" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 0,
                                got: arg_vals.len(),
                            });
                        }
                        return Ok(Value::Numeric(self.err_line as f64));
                    }
                    "DATE$" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 0,
                                got: arg_vals.len(),
                            });
                        }
                        return self.current_date_value();
                    }
                    "TIME$" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch {
                                expected: 0,
                                got: arg_vals.len(),
                            });
                        }
                        return self.current_time_value();
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
                        let row = arg_vals[0].to_i64()?;
                        let col = arg_vals[1].to_i64()?;
                        if row < 1 || col < 1 {
                            return Err(RuntimeError::IllegalFunctionCall {
                                msg: format!("SCREEN({}, {}): row and col must be >= 1", row, col),
                            });
                        }
                        let r = (row - 1) as usize;
                        let c = (col - 1) as usize;
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
                        let true_val = if self.dialect == crate::Dialect::QuickBasic {
                            -1.0
                        } else {
                            1.0
                        };
                        return Ok(Value::Numeric(if fh.eof_flag { true_val } else { 0.0 }));
                    }
                    "LOF" => {
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
                        if let Some(writer) = &mut fh.writer {
                            writer
                                .flush()
                                .map_err(|e| RuntimeError::from_io("LOF", e))?;
                        }
                        let len = if let Some(reader) = &fh.reader {
                            reader
                                .get_ref()
                                .metadata()
                                .map_err(|e| RuntimeError::from_io("LOF", e))?
                                .len()
                        } else if let Some(writer) = &fh.writer {
                            writer
                                .get_ref()
                                .metadata()
                                .map_err(|e| RuntimeError::from_io("LOF", e))?
                                .len()
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
                    "SEEK" => {
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
                        let pos = if let Some(writer) = &mut fh.writer {
                            writer.flush().map_err(|e| RuntimeError::General {
                                msg: format!("flush error: {e}"),
                            })?;
                            writer.stream_position().unwrap_or(0)
                        } else if let Some(reader) = &mut fh.reader {
                            reader.stream_position().unwrap_or(0)
                        } else {
                            0
                        };
                        // SEEK returns the 1-based byte position of the next read/write.
                        return Ok(Value::Numeric((pos + 1) as f64));
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
                self.require_declared(name)?;
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

    fn eval_array_bound(&mut self, name: &str, args: &[Expr]) -> Result<Value, RuntimeError> {
        if args.is_empty() || args.len() > 2 {
            return Err(RuntimeError::ArityMismatch {
                expected: 1,
                got: args.len(),
            });
        }
        let array_name = match &args[0] {
            Expr::Variable(v) => v.name.clone(),
            Expr::FunctionCall { name, .. } => name.clone(),
            Expr::ArrayIndex { name, .. } => name.clone(),
            _ => {
                return Err(RuntimeError::IllegalFunctionCall {
                    msg: format!("{name} requires an array name argument"),
                });
            }
        };
        let dim = if args.len() == 2 {
            self.eval_expr(&args[1])?.to_i64()?
        } else {
            1
        };
        let bounds = self.array_dim_info.get(&array_name).ok_or_else(|| {
            RuntimeError::IllegalFunctionCall {
                msg: format!("{name}: array '{array_name}' is not dimensioned"),
            }
        })?;
        if dim < 1 || dim as usize > bounds.len() {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: format!("{name}: dimension {dim} out of range"),
            });
        }
        let (lower, upper) = bounds[(dim - 1) as usize];
        let v = if name == "LBOUND" { lower } else { upper };
        Ok(Value::Numeric(v as f64))
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
            child_env.borrow_mut().declare_var(&param.name);
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
        child_env.borrow_mut().declare_var(&func.name);
        let return_default = if func.name.ends_with('$') {
            Value::Str(String::new())
        } else {
            Value::Numeric(0.0)
        };
        child_env.borrow_mut().set(&func.name, return_default);

        let prev_env = self.env.clone();
        let prev_static = std::mem::take(&mut self.current_static_vars);
        let previous_targets = std::mem::replace(
            &mut self.gosub_targets,
            Self::collect_label_targets(&func.body, false),
        );
        self.env = child_env.clone();
        let result = self.exec_block(&func.body);
        self.env = prev_env;
        self.gosub_targets = previous_targets;

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
            UnaryOp::Pos => Ok(Value::Numeric(val.to_f64()?)),
        }
    }

    fn resolve_decl_type(decl: &DimDecl) -> BasicType {
        if let Some(ref t) = decl.as_type {
            t.clone()
        } else if decl.name.ends_with('$') {
            BasicType::String
        } else {
            BasicType::Numeric
        }
    }

    /// Return the default value for a variable
    fn default_for_var(&self, name: &str) -> Value {
        if name.ends_with('$') {
            Value::Str(String::new())
        } else if Self::is_untyped_numeric(name) {
            // Honor DEFSTR for untyped variables based on their first letter.
            let first = name.chars().next().unwrap_or('Z').to_ascii_uppercase();
            if first.is_ascii_alphabetic() && self.def_str_letters[(first as u8 - b'A') as usize] {
                Value::Str(String::new())
            } else {
                Value::Numeric(0.0)
            }
        } else {
            Value::Numeric(0.0)
        }
    }

    fn is_untyped_numeric(name: &str) -> bool {
        !name.ends_with(['$', '%', '!', '#', '&'])
    }

    fn environment_key(name: &str) -> String {
        if cfg!(windows) {
            name.to_uppercase()
        } else {
            name.to_string()
        }
    }

    fn validate_date_format(s: &str) -> Result<(), RuntimeError> {
        // Accept MM-DD-YYYY or MM/DD/YYYY (QBasic allows both separators).
        let s = s.trim();
        if s.len() != 10 {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: format!("DATE$: invalid date format '{}'", s),
            });
        }
        let sep1 = s.chars().nth(2);
        let sep2 = s.chars().nth(5);
        if sep1 != sep2 || !matches!(sep1, Some('-') | Some('/')) {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: format!("DATE$: invalid date format '{}'", s),
            });
        }
        let parts: Vec<&str> = s.split(['-', '/']).collect();
        if parts.len() != 3 {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: format!("DATE$: invalid date format '{}'", s),
            });
        }
        let month: u32 = parts[0]
            .parse()
            .map_err(|_| RuntimeError::IllegalFunctionCall {
                msg: format!("DATE$: invalid month '{}'", parts[0]),
            })?;
        let day: u32 = parts[1]
            .parse()
            .map_err(|_| RuntimeError::IllegalFunctionCall {
                msg: format!("DATE$: invalid day '{}'", parts[1]),
            })?;
        let year: u32 = parts[2]
            .parse()
            .map_err(|_| RuntimeError::IllegalFunctionCall {
                msg: format!("DATE$: invalid year '{}'", parts[2]),
            })?;
        if !(1..=12).contains(&month) || !(1..=31).contains(&day) || !(0..=9999).contains(&year) {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: format!("DATE$: invalid date '{}'", s),
            });
        }
        Ok(())
    }

    fn validate_time_format(s: &str) -> Result<(), RuntimeError> {
        let s = s.trim();
        if s.len() != 8 {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: format!("TIME$: invalid time format '{}'", s),
            });
        }
        if s.chars().nth(2) != Some(':') || s.chars().nth(5) != Some(':') {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: format!("TIME$: invalid time format '{}'", s),
            });
        }
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: format!("TIME$: invalid time format '{}'", s),
            });
        }
        let hour: u32 = parts[0]
            .parse()
            .map_err(|_| RuntimeError::IllegalFunctionCall {
                msg: format!("TIME$: invalid hour '{}'", parts[0]),
            })?;
        let minute: u32 = parts[1]
            .parse()
            .map_err(|_| RuntimeError::IllegalFunctionCall {
                msg: format!("TIME$: invalid minute '{}'", parts[1]),
            })?;
        let second: u32 = parts[2]
            .parse()
            .map_err(|_| RuntimeError::IllegalFunctionCall {
                msg: format!("TIME$: invalid second '{}'", parts[2]),
            })?;
        if hour >= 24 || minute >= 60 || second >= 60 {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: format!("TIME$: invalid time '{}'", s),
            });
        }
        Ok(())
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
        // Under OPTION EXPLICIT, arrays must be dimensioned before use.
        if self.option_explicit && !self.is_declared(name) {
            return Err(RuntimeError::General {
                msg: format!("Array '{}' is not declared (OPTION EXPLICIT)", name),
            });
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
        self.create_record_fields(type_name, &mut HashSet::new())
    }

    fn create_record_fields(
        &self,
        type_name: &str,
        active_types: &mut HashSet<String>,
    ) -> Result<Value, RuntimeError> {
        if !active_types.insert(type_name.to_string()) {
            return Err(RuntimeError::General {
                msg: format!("recursive TYPE definition: {type_name}"),
            });
        }
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
                BasicType::UserDefined(nested) => {
                    self.create_record_fields(nested, active_types)?
                }
                other => Value::default_for(other.clone()),
            };
            fields.insert(name, val);
        }
        active_types.remove(type_name);
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
                    self.write_text(&formatted.join(" "))?;
                    self.write_text("\n")?;
                }
                self.output
                    .flush()
                    .map_err(|e| RuntimeError::from_io("output", e))?;
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
                    self.output
                        .flush()
                        .map_err(|e| RuntimeError::from_io("output", e))?;
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
    fn write_text(&mut self, text: &str) -> Result<(), RuntimeError> {
        write!(self.output, "{}", text).map_err(|e| RuntimeError::from_io("output", e))?;
        crate::update_screen_buffer(
            &mut self.screen_buffer,
            &mut self.print_row,
            &mut self.print_col,
            text,
        );
        Ok(())
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
        let mut record_len = if let Some(expr) = &open.record_len {
            let len = self.eval_expr(expr)?.to_i64()?;
            if len < 1 {
                return Err(RuntimeError::IllegalFunctionCall {
                    msg: format!("record length must be >= 1, got {len}"),
                });
            }
            Some(len as usize)
        } else {
            None
        };

        if record_len.is_none()
            && self.dialect == crate::Dialect::QuickBasic
            && open.access == FileAccess::OutIn
            && open.organization == Some(FileOrg::Sequential)
        {
            record_len = Some(128);
        }

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

        let mut field_buffer = Vec::new();
        if let Some(len) = record_len {
            field_buffer
                .try_reserve_exact(len)
                .map_err(|_| RuntimeError::BasicError { code: 7 })?;
            field_buffer.resize(len, b' ');
        }

        let (reader, writer) = match open.access {
            FileAccess::Input => {
                let f = File::open(&filename).map_err(|e| RuntimeError::from_io("OPEN", e))?;
                (Some(BufReader::new(f)), None)
            }
            FileAccess::Output => {
                let f = File::create(&filename).map_err(|e| RuntimeError::from_io("OPEN", e))?;
                (None, Some(BufWriter::new(f)))
            }
            FileAccess::OutIn => {
                let f = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .open(&filename)
                    .map_err(|e| RuntimeError::from_io("OPEN", e))?;
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
                    .map_err(|e| RuntimeError::from_io("OPEN", e))?;
                (None, Some(BufWriter::new(f)))
            }
        };

        self.file_handles.insert(
            file_num,
            FileHandle {
                _access: open.access,
                reader,
                writer,
                print_col: 0,
                eof_flag: false,
                record_len,
                field_layout: Vec::new(),
                field_buffer,
            },
        );

        Ok(())
    }

    fn exec_close(&mut self, file_nums: &[Expr]) -> Result<(), RuntimeError> {
        let nums: Vec<i64> = if file_nums.is_empty() {
            self.file_handles.keys().copied().collect()
        } else {
            file_nums
                .iter()
                .map(|e| self.eval_expr(e).and_then(|v| v.to_i64()))
                .collect::<Result<Vec<_>, _>>()?
        };
        // Keep handles available if flushing fails, so an error handler can retry.
        for n in &nums {
            if let Some(fh) = self.file_handles.get_mut(n)
                && let Some(writer) = &mut fh.writer
            {
                writer
                    .flush()
                    .map_err(|e| RuntimeError::from_io("CLOSE", e))?;
            }
        }
        for n in nums {
            self.file_handles.remove(&n);
        }
        Ok(())
    }

    fn exec_field(
        &mut self,
        file_num_expr: &Expr,
        fields: &[FieldSpec],
    ) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(file_num_expr)?.to_i64()?;
        let mut specs = Vec::new();
        for field in fields {
            let width = self.eval_expr(&field.width)?.to_i64()?;
            if width < 0 {
                return Err(RuntimeError::IllegalFunctionCall {
                    msg: format!("FIELD width must be non-negative, got {width}"),
                });
            }
            if !field.var.name.ends_with('$') {
                return Err(RuntimeError::TypeMismatch {
                    msg: "FIELD variables must be strings".into(),
                });
            }
            specs.push((width as usize, field.var.name.clone()));
        }

        let updates = {
            let fh = self
                .file_handles
                .get_mut(&file_num)
                .ok_or_else(|| RuntimeError::General {
                    msg: format!("file #{file_num} is not open"),
                })?;

            let total_width = specs.iter().try_fold(0usize, |total, (width, _)| {
                total
                    .checked_add(*width)
                    .ok_or_else(|| RuntimeError::Overflow {
                        msg: "FIELD widths exceed the supported record size".into(),
                    })
            })?;
            if fh.record_len.is_none() {
                fh.field_buffer
                    .try_reserve(total_width.max(1).saturating_sub(fh.field_buffer.len()))
                    .map_err(|_| RuntimeError::BasicError { code: 7 })?;
                fh.record_len = Some(total_width.max(1));
                fh.field_buffer.resize(total_width.max(1), b' ');
            }
            let record_len = fh.record_len.unwrap_or(total_width.max(1));
            if total_width > record_len {
                return Err(RuntimeError::IllegalFunctionCall {
                    msg: format!(
                        "FIELD widths total {total_width}, exceeding record length {record_len}"
                    ),
                });
            }
            if fh.field_buffer.len() != record_len {
                fh.field_buffer.resize(record_len, b' ');
            }

            let mut offset = 0;
            let mut layout = Vec::new();
            for (width, name) in specs {
                layout.push(FieldBinding {
                    name,
                    offset,
                    width,
                });
                offset += width;
            }
            fh.field_layout = layout;
            Self::field_variable_values(fh)
        };

        for (name, value) in updates {
            self.env.borrow_mut().set(&name, Value::Str(value));
        }
        Ok(())
    }

    fn exec_lset_rset(
        &mut self,
        var: &Variable,
        expr: &Expr,
        right_align: bool,
    ) -> Result<(), RuntimeError> {
        if !var.name.ends_with('$') {
            return Err(RuntimeError::TypeMismatch {
                msg: "LSET/RSET target must be a string variable".into(),
            });
        }
        let value = self.eval_expr(expr)?.to_string_val()?;
        let bytes = basic_string_to_bytes(&value);
        let mut field_update = None;

        for fh in self.file_handles.values_mut() {
            if let Some(binding) = fh
                .field_layout
                .iter()
                .find(|binding| binding.name == var.name)
                .cloned()
            {
                let new_value = Self::write_field_binding(fh, &binding, &bytes, right_align);
                field_update = Some(new_value);
                break;
            }
        }

        let value = if let Some(value) = field_update {
            value
        } else {
            let width = self
                .env
                .borrow()
                .get(&var.name)
                .and_then(|v| v.to_string_val().ok())
                .map(|s| basic_string_to_bytes(&s).len())
                .unwrap_or(bytes.len());
            bytes_to_basic_string(&Self::aligned_bytes(&bytes, width, right_align))
        };
        self.env.borrow_mut().set(&var.name, Value::Str(value));
        Ok(())
    }

    fn aligned_bytes(bytes: &[u8], width: usize, right_align: bool) -> Vec<u8> {
        let mut result = vec![b' '; width];
        let copy_len = bytes.len().min(width);
        let dest_start = if right_align { width - copy_len } else { 0 };
        result[dest_start..dest_start + copy_len].copy_from_slice(&bytes[..copy_len]);
        result
    }

    fn write_field_binding(
        fh: &mut FileHandle,
        binding: &FieldBinding,
        bytes: &[u8],
        right_align: bool,
    ) -> String {
        let start = binding.offset;
        let end = start + binding.width;
        if fh.field_buffer.len() < end {
            fh.field_buffer.resize(end, b' ');
        }
        let aligned = Self::aligned_bytes(bytes, binding.width, right_align);
        fh.field_buffer[start..end].copy_from_slice(&aligned);
        bytes_to_basic_string(&fh.field_buffer[start..end])
    }

    fn field_variable_values(fh: &FileHandle) -> Vec<(String, String)> {
        fh.field_layout
            .iter()
            .map(|binding| {
                let start = binding.offset;
                let end = (start + binding.width).min(fh.field_buffer.len());
                (
                    binding.name.clone(),
                    bytes_to_basic_string(&fh.field_buffer[start..end]),
                )
            })
            .collect()
    }

    fn record_start(fh: &FileHandle, record: i64) -> Result<u64, RuntimeError> {
        if record < 1 {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: "record position must be >= 1".into(),
            });
        }
        let idx = (record - 1) as u64;
        idx.checked_mul(fh.record_len.unwrap_or(1) as u64)
            .ok_or_else(|| RuntimeError::Overflow {
                msg: "record position exceeds the supported file size".into(),
            })
    }

    fn exec_set_pointer(
        &mut self,
        file_num_expr: &Expr,
        position_expr: &Expr,
    ) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(file_num_expr)?.to_i64()?;
        let position = self.eval_expr(position_expr)?.to_i64()?;
        if position < 1 {
            return Err(RuntimeError::IllegalFunctionCall {
                msg: "SET POINTER position must be >= 1".into(),
            });
        }
        let fh = self
            .file_handles
            .get_mut(&file_num)
            .ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open"),
            })?;
        // SET POINTER uses 1-based byte position
        let byte_pos = (position - 1) as u64;
        if let Some(writer) = &mut fh.writer {
            writer
                .flush()
                .map_err(|e| RuntimeError::from_io("flush", e))?;
            writer
                .seek(SeekFrom::Start(byte_pos))
                .map_err(|e| RuntimeError::from_io("seek", e))?;
        }
        if let Some(reader) = &mut fh.reader {
            reader
                .seek(SeekFrom::Start(byte_pos))
                .map_err(|e| RuntimeError::from_io("seek", e))?;
        }
        fh.eof_flag = false;
        Ok(())
    }

    fn exec_ask_pointer(
        &mut self,
        file_num_expr: &Expr,
        var: &Variable,
    ) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(file_num_expr)?.to_i64()?;
        self.require_writable(&var.name)?;
        let fh = self
            .file_handles
            .get_mut(&file_num)
            .ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open"),
            })?;
        // Flush writer to ensure position reflects writes
        if let Some(writer) = &mut fh.writer {
            writer
                .flush()
                .map_err(|e| RuntimeError::from_io("flush", e))?;
        }
        let byte_pos = if let Some(writer) = &mut fh.writer {
            writer
                .stream_position()
                .map_err(|e| RuntimeError::from_io("ASK POINTER", e))?
        } else if let Some(reader) = &mut fh.reader {
            reader
                .stream_position()
                .map_err(|e| RuntimeError::from_io("ASK POINTER", e))?
        } else {
            0
        };
        // ASK POINTER returns 1-based byte position
        let result = byte_pos as f64 + 1.0;
        self.env.borrow_mut().set(&var.name, Value::Numeric(result));
        Ok(())
    }

    fn append_file_print_text(output: &mut String, column: &mut usize, text: &str) {
        output.push_str(text);
        for ch in text.chars() {
            if ch == '\n' {
                *column = 0;
            } else {
                *column += 1;
            }
        }
    }

    fn append_file_print_spaces(
        output: &mut String,
        column: &mut usize,
        count: usize,
    ) -> Result<(), RuntimeError> {
        let next_column = column
            .checked_add(count)
            .ok_or_else(|| RuntimeError::Overflow {
                msg: "PRINT column exceeds the supported range".into(),
            })?;
        output
            .try_reserve(count)
            .map_err(|_| RuntimeError::BasicError { code: 7 })?;
        output.extend(std::iter::repeat_n(' ', count));
        *column = next_column;
        Ok(())
    }

    fn exec_file_print(&mut self, pf: &FilePrintStmt) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(&pf.file_num)?.to_i64()?;
        let mut column = self
            .file_handles
            .get(&file_num)
            .ok_or_else(|| RuntimeError::General {
                msg: format!("file #{file_num} is not open"),
            })?
            .print_col;
        let mut output = String::new();
        if let Some(fmt_expr) = &pf.format {
            let result = self.eval_format_using(fmt_expr, &pf.items)?;
            Self::append_file_print_text(&mut output, &mut column, &result);
            if pf.trailing == PrintSep::Comma {
                let spaces = 16 - column % 16;
                Self::append_file_print_spaces(&mut output, &mut column, spaces)?;
            }
        } else {
            for item in &pf.items {
                match item {
                    PrintItem::Expr(expr) => {
                        let text = self.eval_expr(expr)?.format_for_print();
                        Self::append_file_print_text(&mut output, &mut column, &text);
                    }
                    PrintItem::Tab(expr) | PrintItem::Spc(expr) => {
                        let count = self.eval_expr(expr)?.to_i64()?;
                        if count < 0 {
                            return Err(RuntimeError::IllegalFunctionCall {
                                msg: "TAB/SPC argument must be non-negative".into(),
                            });
                        }
                        let spaces = if matches!(item, PrintItem::Tab(_)) {
                            (count as usize).saturating_sub(1).saturating_sub(column)
                        } else {
                            count as usize
                        };
                        Self::append_file_print_spaces(&mut output, &mut column, spaces)?;
                    }
                    PrintItem::Comma => {
                        let spaces = 16 - column % 16;
                        Self::append_file_print_spaces(&mut output, &mut column, spaces)?;
                    }
                }
            }
        }
        if pf.trailing == PrintSep::Newline {
            Self::append_file_print_text(&mut output, &mut column, "\n");
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
        writer
            .write_all(output.as_bytes())
            .map_err(|e| RuntimeError::from_io("PRINT", e))?;
        fh.print_col = column;
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
                write!(writer, ",").map_err(|e| RuntimeError::from_io("WRITE", e))?;
            }
            match val {
                Value::Str(s) => {
                    write!(writer, "\"{}\"", s.replace('"', "\"\""))
                        .map_err(|e| RuntimeError::from_io("WRITE", e))?;
                }
                _ => {
                    write!(writer, "{}", val.format_for_write())
                        .map_err(|e| RuntimeError::from_io("WRITE", e))?;
                }
            }
        }
        writeln!(writer).map_err(|e| RuntimeError::from_io("WRITE", e))?;
        fh.print_col = 0;

        Ok(())
    }

    fn exec_file_input(&mut self, fi: &FileInputStmt) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(&fi.file_num)?.to_i64()?;
        for var in &fi.vars {
            self.require_writable(&var.name)?;
        }

        let mut fields = Vec::with_capacity(fi.vars.len());
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

            for _ in &fi.vars {
                let Some(field) = Self::read_next_field(reader)? else {
                    fh.eof_flag = true;
                    return Err(RuntimeError::IoError {
                        msg: "INPUT past end of file".into(),
                        code: 62,
                    });
                };
                fields.push(field);
            }
            fh.eof_flag = reader
                .fill_buf()
                .map_err(|e| RuntimeError::from_io("INPUT", e))?
                .is_empty();
        }

        let values = fi
            .vars
            .iter()
            .zip(fields)
            .map(|(var, field)| {
                let existing = self
                    .env
                    .borrow()
                    .get(&var.name)
                    .unwrap_or_else(|| self.default_for_var(&var.name));
                match existing {
                    Value::Str(_) => Ok(Value::Str(field)),
                    Value::Numeric(_) => field.parse::<f64>().map(Value::Numeric).map_err(|_| {
                        RuntimeError::TypeMismatch {
                            msg: format!("INPUT expected a number for {}", var.name),
                        }
                    }),
                    Value::Record { .. } => Err(RuntimeError::TypeMismatch {
                        msg: "INPUT cannot read a record value".into(),
                    }),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (var, value) in fi.vars.iter().zip(values) {
            self.env.borrow_mut().set(&var.name, value);
        }
        Ok(())
    }

    fn read_next_field(reader: &mut impl BufRead) -> Result<Option<String>, RuntimeError> {
        // Fields may span lines; ignore separators before the next field.
        loop {
            let buf = reader
                .fill_buf()
                .map_err(|e| RuntimeError::from_io("INPUT", e))?;
            if buf.is_empty() {
                return Ok(None);
            }
            if matches!(buf[0], b' ' | b'\t' | b'\r' | b'\n') {
                reader.consume(1);
            } else {
                break;
            }
        }

        let quoted = reader
            .fill_buf()
            .map_err(|e| RuntimeError::from_io("INPUT", e))?[0]
            == b'"';
        if quoted {
            reader.consume(1);
        }
        let mut field = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            let n = reader
                .read(&mut byte)
                .map_err(|e| RuntimeError::from_io("INPUT", e))?;
            if n == 0 {
                if quoted {
                    return Err(RuntimeError::IoError {
                        msg: "INPUT reached end of file inside a quoted string".into(),
                        code: 62,
                    });
                }
                break;
            }
            if quoted {
                if byte[0] == b'"' {
                    let next = reader
                        .fill_buf()
                        .map_err(|e| RuntimeError::from_io("INPUT", e))?;
                    if next.first() == Some(&b'"') {
                        reader.consume(1);
                        field.push(b'"');
                        continue;
                    }
                    // Permit whitespace between the closing quote and delimiter.
                    loop {
                        let next = reader
                            .fill_buf()
                            .map_err(|e| RuntimeError::from_io("INPUT", e))?;
                        match next.first() {
                            Some(b' ' | b'\t') => reader.consume(1),
                            Some(b',') => {
                                reader.consume(1);
                                break;
                            }
                            _ => break,
                        }
                    }
                    break;
                }
            } else if matches!(byte[0], b',' | b'\r' | b'\n') {
                if byte[0] == b'\r' {
                    let next = reader
                        .fill_buf()
                        .map_err(|e| RuntimeError::from_io("INPUT", e))?;
                    if next.first() == Some(&b'\n') {
                        reader.consume(1);
                    }
                }
                break;
            }
            field.push(byte[0]);
        }
        let field = String::from_utf8(field).map_err(|e| RuntimeError::General {
            msg: format!("INPUT file contains invalid UTF-8: {e}"),
        })?;
        Ok(Some(if quoted {
            field
        } else {
            field.trim().to_string()
        }))
    }

    fn exec_line_input_file(
        &mut self,
        file_num_expr: &Expr,
        var: &Variable,
    ) -> Result<(), RuntimeError> {
        let file_num = self.eval_expr(file_num_expr)?.to_i64()?;
        self.require_writable(&var.name)?;
        let existing = self
            .env
            .borrow()
            .get(&var.name)
            .unwrap_or_else(|| self.default_for_var(&var.name));
        if !matches!(existing, Value::Str(_)) {
            return Err(RuntimeError::TypeMismatch {
                msg: "LINE INPUT requires a string variable".into(),
            });
        }

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
                .map_err(|e| RuntimeError::from_io("read", e))?;

            if bytes_read == 0 {
                fh.eof_flag = true;
                return Err(RuntimeError::IoError {
                    msg: "LINE INPUT past end of file".into(),
                    code: 62,
                });
            }

            // Check if more data available
            let buf = reader
                .fill_buf()
                .map_err(|e| RuntimeError::from_io("LINE INPUT", e))?;
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
        if let Some(Value::Record { type_name, .. }) = self.env.borrow().get(name) {
            return BasicType::UserDefined(type_name);
        }
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
                    .map_err(|e| RuntimeError::from_io("write", e))?;
            }
            BasicType::Long => {
                let n = val.to_f64()? as i32;
                writer
                    .write_all(&n.to_le_bytes())
                    .map_err(|e| RuntimeError::from_io("write", e))?;
            }
            BasicType::Single => {
                let n = val.to_f64()? as f32;
                writer
                    .write_all(&n.to_le_bytes())
                    .map_err(|e| RuntimeError::from_io("write", e))?;
            }
            BasicType::Double | BasicType::Numeric => {
                let n = val.to_f64()?;
                writer
                    .write_all(&n.to_le_bytes())
                    .map_err(|e| RuntimeError::from_io("write", e))?;
            }
            BasicType::FixedLengthString(len) => {
                let s = val.to_string_val()?;
                let mut bytes = basic_string_to_bytes(&s);
                bytes.resize(*len, 0);
                writer
                    .write_all(&bytes)
                    .map_err(|e| RuntimeError::from_io("write", e))?;
            }
            BasicType::String => {
                let s = val.to_string_val()?;
                let bytes = basic_string_to_bytes(&s);
                let len =
                    u16::try_from(bytes.len()).map_err(|_| RuntimeError::IllegalFunctionCall {
                        msg: "binary string exceeds the maximum length of 65535 bytes".into(),
                    })?;
                writer
                    .write_all(&len.to_le_bytes())
                    .map_err(|e| RuntimeError::from_io("write", e))?;
                writer
                    .write_all(&bytes)
                    .map_err(|e| RuntimeError::from_io("write", e))?;
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
                    .map_err(|e| RuntimeError::from_io("read", e))?;
                let val = i16::from_le_bytes(buf) as f64;
                Ok(Value::Numeric(val))
            }
            BasicType::Long => {
                let mut buf = [0u8; 4];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| RuntimeError::from_io("read", e))?;
                let val = i32::from_le_bytes(buf) as f64;
                Ok(Value::Numeric(val))
            }
            BasicType::Single => {
                let mut buf = [0u8; 4];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| RuntimeError::from_io("read", e))?;
                let val = f32::from_le_bytes(buf) as f64;
                Ok(Value::Numeric(val))
            }
            BasicType::Double | BasicType::Numeric => {
                let mut buf = [0u8; 8];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| RuntimeError::from_io("read", e))?;
                let val = f64::from_le_bytes(buf);
                Ok(Value::Numeric(val))
            }
            BasicType::FixedLengthString(len) => {
                let mut buf = vec![0u8; *len];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| RuntimeError::from_io("read", e))?;
                while buf.last() == Some(&0) {
                    buf.pop();
                }
                Ok(Value::Str(bytes_to_basic_string(&buf)))
            }
            BasicType::String => {
                let mut len_buf = [0u8; 2];
                reader
                    .read_exact(&mut len_buf)
                    .map_err(|e| RuntimeError::from_io("read", e))?;
                let len = u16::from_le_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                reader
                    .read_exact(&mut buf)
                    .map_err(|e| RuntimeError::from_io("read", e))?;
                Ok(Value::Str(bytes_to_basic_string(&buf)))
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
                writer
                    .flush()
                    .map_err(|e| RuntimeError::from_io("GET", e))?;
            }

            if let Some(pos) = record {
                let byte_pos = Self::record_start(fh, pos)?;
                if let Some(reader) = &mut fh.reader {
                    reader
                        .seek(SeekFrom::Start(byte_pos))
                        .map_err(|e| RuntimeError::from_io("seek", e))?;
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
                    let bytes_read = reader
                        .read(&mut buf)
                        .map_err(|e| RuntimeError::from_io("GET", e))?;
                    if bytes_read == 0 {
                        fh.eof_flag = true;
                    }
                    buf.truncate(bytes_read);
                    let s = String::from_utf8_lossy(&buf)
                        .trim_end_matches('\0')
                        .to_string();
                    self.env.borrow_mut().set(&var.name, Value::Str(s));
                }
            } else if fh.record_len.is_some() && !fh.field_layout.is_empty() {
                let record_len = fh.record_len.unwrap_or(fh.field_buffer.len());
                fh.field_buffer.resize(record_len, b' ');
                fh.field_buffer.fill(b' ');
                let bytes_read = reader
                    .read(&mut fh.field_buffer)
                    .map_err(|e| RuntimeError::from_io("read", e))?;
                if bytes_read == 0 {
                    fh.eof_flag = true;
                }
                let updates = Self::field_variable_values(fh);
                for (name, value) in updates {
                    self.env.borrow_mut().set(&name, Value::Str(value));
                }
            }
        } else {
            if let Some(pos) = record {
                let byte_pos = Self::record_start(fh, pos)?;
                if let Some(writer) = &mut fh.writer {
                    writer
                        .seek(SeekFrom::Start(byte_pos))
                        .map_err(|e| RuntimeError::from_io("seek", e))?;
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
                    writer
                        .write_all(s.as_bytes())
                        .map_err(|e| RuntimeError::from_io("PUT", e))?;
                }
            } else if fh.record_len.is_some() && !fh.field_layout.is_empty() {
                writer
                    .write_all(&fh.field_buffer)
                    .map_err(|e| RuntimeError::from_io("write", e))?;
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

        if gp.is_get
            && let Some(var) = &gp.var
        {
            self.require_writable(&var.name)?;
        }

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

#[cfg(test)]
mod file_io_tests {
    use super::*;
    use std::io::Cursor;

    fn empty_handle() -> FileHandle {
        FileHandle {
            _access: FileAccess::OutIn,
            reader: None,
            writer: None,
            print_col: 0,
            eof_flag: false,
            record_len: None,
            field_layout: Vec::new(),
            field_buffer: Vec::new(),
        }
    }

    #[test]
    fn text_fields_preserve_unicode_quotes_and_delimiters() {
        let mut reader = Cursor::new("\"héllo \"\"世界\"\"\" , 42\r\nplainé".as_bytes());
        for expected in ["héllo \"世界\"", "42", "plainé"] {
            assert_eq!(
                Interpreter::read_next_field(&mut reader).unwrap(),
                Some(expected.into())
            );
        }
        assert_eq!(Interpreter::read_next_field(&mut reader).unwrap(), None);
    }

    #[test]
    fn quoted_field_requires_a_closing_quote() {
        let mut reader = Cursor::new(b"\"unfinished".as_slice());
        let error = Interpreter::read_next_field(&mut reader).unwrap_err();
        assert_eq!(error.basic_err_code(), 62);
    }

    #[test]
    fn field_read_errors_are_not_treated_as_end_of_file() {
        struct FailingReader(Cursor<Vec<u8>>);
        impl io::Read for FailingReader {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                let count = self.0.read(bytes)?;
                if count == 0 {
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "read failed",
                    ))
                } else {
                    Ok(count)
                }
            }
        }
        impl BufRead for FailingReader {
            fn fill_buf(&mut self) -> io::Result<&[u8]> {
                if self.0.position() == self.0.get_ref().len() as u64 {
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "read failed",
                    ))
                } else {
                    self.0.fill_buf()
                }
            }
            fn consume(&mut self, count: usize) {
                self.0.consume(count);
            }
        }
        for bytes in [b"unfinished".to_vec(), b"\"unfinished".to_vec()] {
            let error =
                Interpreter::read_next_field(&mut FailingReader(Cursor::new(bytes))).unwrap_err();
            assert_eq!(error.basic_err_code(), 70);
        }
    }

    #[test]
    fn record_offsets_validate_boundaries_without_overflow() {
        let mut handle = empty_handle();
        handle.record_len = Some(128);
        assert_eq!(Interpreter::record_start(&handle, 1).unwrap(), 0);
        assert_eq!(Interpreter::record_start(&handle, 3).unwrap(), 256);
        for record in [i64::MIN, -1, 0] {
            assert!(matches!(
                Interpreter::record_start(&handle, record),
                Err(RuntimeError::IllegalFunctionCall { .. })
            ));
        }
        assert!(matches!(
            Interpreter::record_start(&handle, i64::MAX),
            Err(RuntimeError::Overflow { .. })
        ));
    }

    #[test]
    fn close_reports_flush_failure_and_preserves_handle() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let read_only = File::open(file.path()).unwrap();
        let mut writer = BufWriter::new(read_only);
        writer.write_all(b"pending buffered output").unwrap();
        let mut handle = empty_handle();
        handle.writer = Some(writer);
        let mut interpreter =
            Interpreter::with_io(Box::new(Vec::new()), Box::new(Cursor::new(Vec::new())));
        interpreter.file_handles.insert(1, handle);
        assert!(matches!(
            interpreter.exec_close(&[]),
            Err(RuntimeError::IoError { .. })
        ));
        assert!(interpreter.file_handles.contains_key(&1));
    }

    #[test]
    fn binary_string_length_cannot_wrap_its_header() {
        let interpreter =
            Interpreter::with_io(Box::new(Vec::new()), Box::new(Cursor::new(Vec::new())));
        let mut writer = BufWriter::new(tempfile::tempfile().unwrap());
        let value = Value::Str("x".repeat(u16::MAX as usize + 1));
        assert!(matches!(
            interpreter.serialize_value(&mut writer, &value, &BasicType::String),
            Err(RuntimeError::IllegalFunctionCall { .. })
        ));
        assert_eq!(writer.stream_position().unwrap(), 0);
    }
}
