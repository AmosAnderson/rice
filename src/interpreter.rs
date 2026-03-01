use std::cell::RefCell;
use std::collections::HashMap;
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
}

#[derive(Clone)]
struct UserFunction {
    name: String,
    suffix: Option<TypeSuffix>,
    params: Vec<Param>,
    body: Vec<LabeledStmt>,
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
    print_col: usize,
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
        Self::with_io(
            Box::new(io::stdout()),
            Box::new(io::BufReader::new(io::stdin())),
        )
    }

    pub fn with_io(output: Box<dyn Write>, input: Box<dyn BufRead>) -> Self {
        Self {
            env: Environment::new_global(),
            builtins: BuiltinRegistry::new(),
            subs: HashMap::new(),
            functions: HashMap::new(),
            print_col: 0,
            data_values: Vec::new(),
            data_pos: 0,
            output,
            input,
            file_handles: HashMap::new(),
            error_handler: None,
            current_error: None,
            error_resume_pc: None,
            in_error_handler: false,
        }
    }

    pub fn run_source(&mut self, source: &str) -> Result<(), Box<dyn std::error::Error>> {
        let tokens = crate::lexer::Lexer::new(source).tokenize()?;
        let program = crate::parser::Parser::new(tokens).parse_program()?;
        self.run_program(&program)?;
        Ok(())
    }

    pub fn run_program(&mut self, program: &Program) -> Result<(), RuntimeError> {
        // Pre-scan: collect labels, DATA statements, SUB/FUNCTION definitions
        self.prescan(&program.statements);

        // Execute top-level statements
        self.exec_block(&program.statements)?;
        Ok(())
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
                        },
                    );
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
                    let default = Value::default_for(Self::resolve_decl_type(decl));
                    self.env.borrow_mut().set(&decl.name, decl.suffix, default);
                }
                Ok(ControlFlow::Normal)
            }
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
            Stmt::ExitFunction => Ok(ControlFlow::ExitFunction(Value::Integer(0))),
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
                }
                Ok(ControlFlow::Normal)
            }
            Stmt::Erase(names) => {
                for name in names {
                    self.env.borrow_mut().set(name, None, Value::Integer(0));
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
        }
    }

    fn exec_print(&mut self, ps: &PrintStmt) -> Result<(), RuntimeError> {
        // Handle PRINT USING
        if let Some(ref fmt_expr) = ps.format {
            let result = self.eval_format_using(fmt_expr, &ps.items)?;
            write!(self.output, "{}", result).ok();
            self.print_col += result.len();
            match ps.trailing {
                PrintSep::Newline => {
                    writeln!(self.output).ok();
                    self.print_col = 0;
                }
                PrintSep::Semicolon => {}
                PrintSep::Comma => {
                    let next_zone = ((self.print_col / 14) + 1) * 14;
                    let spaces = next_zone - self.print_col;
                    write!(self.output, "{}", " ".repeat(spaces)).ok();
                    self.print_col = next_zone;
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
                    write!(self.output, "{}", s).ok();
                    self.print_col += s.len();
                }
                PrintItem::Tab(expr) => {
                    let n = self.eval_expr(expr)?.to_i64()? as usize;
                    if n > self.print_col {
                        let spaces = n - self.print_col;
                        write!(self.output, "{}", " ".repeat(spaces)).ok();
                        self.print_col = n;
                    }
                }
                PrintItem::Spc(expr) => {
                    let n = self.eval_expr(expr)?.to_i64()? as usize;
                    write!(self.output, "{}", " ".repeat(n)).ok();
                    self.print_col += n;
                }
                PrintItem::Comma => {
                    // Advance to next 14-column zone
                    let next_zone = ((self.print_col / 14) + 1) * 14;
                    let spaces = next_zone - self.print_col;
                    write!(self.output, "{}", " ".repeat(spaces)).ok();
                    self.print_col = next_zone;
                }
            }
        }
        match ps.trailing {
            PrintSep::Newline => {
                writeln!(self.output).ok();
                self.print_col = 0;
            }
            PrintSep::Semicolon => {}
            PrintSep::Comma => {
                let next_zone = ((self.print_col / 14) + 1) * 14;
                let spaces = next_zone - self.print_col;
                write!(self.output, "{}", " ".repeat(spaces)).ok();
                self.print_col = next_zone;
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
                ControlFlow::ExitDo => break,
                ControlFlow::End => return Ok(ControlFlow::End),
                ControlFlow::Goto(l) => return Ok(ControlFlow::Goto(l)),
                ControlFlow::ExitSub => return Ok(ControlFlow::ExitSub),
                ControlFlow::ExitFunction(v) => return Ok(ControlFlow::ExitFunction(v)),
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

            let prev_env = self.env.clone();
            self.env = child_env;
            let result = self.exec_block(&sub.body);
            self.env = prev_env;

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

                    let is_implicit_builtin = matches!(builtin_name.as_str(), "DATE$" | "TIME$" | "TIMER");
                    if is_implicit_builtin
                        && let Some(result) = self.builtins.call(&builtin_name, &[])?
                    {
                        return Ok(result);
                    }

                    let default = Value::default_for_suffix(var.suffix);
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
                Ok(self.env.borrow().get(&key, None).unwrap_or_else(|| {
                    Value::default_for_suffix(*suffix)
                }))
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
                    "ERR" | "ERL" => {
                        if !arg_vals.is_empty() {
                            return Err(RuntimeError::ArityMismatch { expected: 0, got: arg_vals.len() });
                        }
                        return Ok(self.get_error_value(name));
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

                // Try user-defined function
                let func = self.functions.get(&func_name).or_else(|| self.functions.get(name)).cloned();
                if let Some(func) = func {
                    return self.call_user_function(&func, &arg_vals);
                }

                Err(RuntimeError::General {
                    msg: format!("undefined function: {func_name}"),
                })
            }
            Expr::Paren(inner) => self.eval_expr(inner),
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

        // Initialize function return variable
        let return_default = Value::default_for_suffix(func.suffix);
        child_env
            .borrow_mut()
            .set(&func.name, func.suffix, return_default);

        let prev_env = self.env.clone();
        self.env = child_env.clone();
        let result = self.exec_block(&func.body);
        self.env = prev_env;

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
            BasicType::Double | BasicType::String => Value::Double(n),
        })
    }

    fn resolve_decl_type(decl: &DimDecl) -> BasicType {
        if let Some(t) = decl.as_type {
            t
        } else if let Some(s) = decl.suffix {
            s.to_basic_type()
        } else {
            BasicType::Single
        }
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
