use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
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
            Stmt::GetPut(_) => {
                // TODO
                Ok(ControlFlow::Normal)
            }
            Stmt::OnErrorGoto(_) | Stmt::Resume(_) => {
                // TODO: error handling
                Ok(ControlFlow::Normal)
            }
        }
    }

    fn exec_print(&mut self, ps: &PrintStmt) -> Result<(), RuntimeError> {
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

    // ==================== File I/O stubs ====================

    fn exec_open(&mut self, _open: &OpenStmt) -> Result<(), RuntimeError> {
        // TODO: implement file I/O
        Ok(())
    }

    fn exec_close(&mut self, _file_nums: &[Expr]) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn exec_file_print(&mut self, _pf: &FilePrintStmt) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn exec_file_write(&mut self, _wf: &FileWriteStmt) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn exec_file_input(&mut self, _fi: &FileInputStmt) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn exec_line_input_file(
        &mut self,
        _file_num: &Expr,
        _var: &Variable,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }
}
