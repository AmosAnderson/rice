/// AST → RiceIR lowering pass.
///
/// Walks the AST and produces a linear sequence of IR instructions.

use std::collections::HashMap;

use crate::ast::*;
use crate::compiler::ir::*;

/// Strip trailing type suffix (%&!#$) from a name
fn strip_suffix(name: &str) -> &str {
    if let Some(last) = name.as_bytes().last() {
        if matches!(last, b'%' | b'&' | b'!' | b'#' | b'$') {
            return &name[..name.len() - 1];
        }
    }
    name
}

/// Lowering context — tracks temp and label allocation
pub struct Lowerer {
    next_temp: TempId,
    next_label: IrLabel,
    instructions: Vec<Instruction>,
    /// Map variable name (UPPERCASE) to VarId
    vars: HashMap<String, VarId>,
    next_var: VarId,
    /// Collected function definitions (lowered separately)
    #[allow(dead_code)]
    functions: Vec<IrFunction>,
    /// Track which names are user-defined functions (for call resolution)
    func_names: std::collections::HashSet<String>,
    /// Name of the function being lowered (for return-value assignment)
    current_func_name: Option<String>,
}

impl Lowerer {
    pub fn new() -> Self {
        Self {
            next_temp: 0,
            next_label: 0,
            instructions: Vec::new(),
            vars: HashMap::new(),
            next_var: 0,
            functions: Vec::new(),
            func_names: std::collections::HashSet::new(),
            current_func_name: None,
        }
    }

    fn alloc_temp(&mut self) -> TempId {
        let t = self.next_temp;
        self.next_temp += 1;
        t
    }

    fn alloc_label(&mut self) -> IrLabel {
        let l = self.next_label;
        self.next_label += 1;
        l
    }

    fn emit(&mut self, inst: Instruction) {
        self.instructions.push(inst);
    }

    /// Get or create a variable slot for the given name
    fn var_id(&mut self, name: &str) -> VarId {
        let key = name.to_uppercase();
        if let Some(&id) = self.vars.get(&key) {
            return id;
        }
        let id = self.next_var;
        self.next_var += 1;
        self.vars.insert(key, id);
        id
    }

    /// Pre-scan for function/sub definitions to know which names are user-defined
    fn prescan_functions(&mut self, stmts: &[LabeledStmt]) {
        for ls in stmts {
            match &ls.stmt {
                Stmt::FunctionDef(fdef) => {
                    self.func_names.insert(fdef.name.to_uppercase());
                }
                Stmt::SubDef(sdef) => {
                    self.func_names.insert(sdef.name.to_uppercase());
                }
                Stmt::Declare(decl) => {
                    self.func_names.insert(decl.name.to_uppercase());
                }
                _ => {}
            }
        }
    }

    /// Lower an entire program to IR
    pub fn lower_program(mut self, program: &Program) -> Result<IrProgram, String> {
        // Pre-scan for function names
        self.prescan_functions(&program.statements);

        // Lower main-level statements (skip function/sub defs)
        for labeled_stmt in &program.statements {
            match &labeled_stmt.stmt {
                Stmt::FunctionDef(_) | Stmt::SubDef(_) => continue,
                _ => self.lower_stmt(&labeled_stmt.stmt)?,
            }
        }
        // Ensure program ends
        self.emit(Instruction::End);

        // Collect function and sub definitions
        let func_defs: Vec<_> = program.statements.iter()
            .filter_map(|ls| match &ls.stmt {
                Stmt::FunctionDef(fdef) => Some(fdef.clone()),
                _ => None,
            })
            .collect();
        let sub_defs: Vec<_> = program.statements.iter()
            .filter_map(|ls| match &ls.stmt {
                Stmt::SubDef(sdef) => Some(sdef.clone()),
                _ => None,
            })
            .collect();

        let main_instructions = std::mem::take(&mut self.instructions);
        let main_var_count = self.next_var;

        let mut functions = Vec::new();
        for fdef in &func_defs {
            let func = self.lower_function(fdef)?;
            functions.push(func);
        }
        for sdef in &sub_defs {
            let func = self.lower_sub(sdef)?;
            functions.push(func);
        }

        let main = IrFunction {
            name: "main".to_string(),
            params: Vec::new(),
            instructions: main_instructions,
            var_count: main_var_count,
        };

        Ok(IrProgram {
            main,
            functions,
        })
    }

    fn lower_function(&mut self, fdef: &FunctionDef) -> Result<IrFunction, String> {
        // Save and reset state for this function
        let saved_instructions = std::mem::take(&mut self.instructions);
        let saved_vars = std::mem::take(&mut self.vars);
        let saved_next_var = self.next_var;
        let saved_next_temp = self.next_temp;
        self.next_var = 0;
        self.next_temp = 0;

        let func_name = fdef.name.to_uppercase();
        self.current_func_name = Some(func_name.clone());

        // Allocate variable slots for parameters
        let mut param_names = Vec::new();
        for p in &fdef.params {
            let var_key = p.name.to_uppercase();
            self.var_id(&var_key);
            param_names.push(var_key);
        }

        // Allocate slot for the return value (function name = variable)
        // Register both bare name and suffixed name so `Double% = ...` finds the same slot
        let ret_vid = self.var_id(&func_name);
        if let Some(ref suffix) = fdef.suffix {
            let suffixed = format!("{}{}", func_name, suffix.to_char());
            self.vars.insert(suffixed, ret_vid);
        }

        // Lower function body
        for ls in &fdef.body {
            self.lower_stmt(&ls.stmt)?;
        }

        // Load return value and emit return
        let ret_var = self.vars[&func_name];
        let ret_temp = self.alloc_temp();
        self.emit(Instruction::LoadVar(ret_temp, ret_var));
        self.emit(Instruction::ReturnFunc(ret_temp));

        let func = IrFunction {
            name: func_name,
            params: param_names,
            instructions: std::mem::take(&mut self.instructions),
            var_count: self.next_var,
        };

        // Restore state
        self.instructions = saved_instructions;
        self.vars = saved_vars;
        self.next_var = saved_next_var;
        self.next_temp = saved_next_temp;
        self.current_func_name = None;

        Ok(func)
    }

    fn lower_sub(&mut self, sdef: &SubDef) -> Result<IrFunction, String> {
        let saved_instructions = std::mem::take(&mut self.instructions);
        let saved_vars = std::mem::take(&mut self.vars);
        let saved_next_var = self.next_var;
        let saved_next_temp = self.next_temp;
        self.next_var = 0;
        self.next_temp = 0;

        let sub_name = sdef.name.to_uppercase();
        self.current_func_name = None; // SUBs don't have return values

        let mut param_names = Vec::new();
        for p in &sdef.params {
            let var_key = p.name.to_uppercase();
            self.var_id(&var_key);
            param_names.push(var_key);
        }

        for ls in &sdef.body {
            self.lower_stmt(&ls.stmt)?;
        }

        // SUBs return void — emit a dummy return
        let zero = self.alloc_temp();
        self.emit(Instruction::LoadConst(zero, Constant::Integer(0)));
        self.emit(Instruction::ReturnFunc(zero));

        let func = IrFunction {
            name: sub_name,
            params: param_names,
            instructions: std::mem::take(&mut self.instructions),
            var_count: self.next_var,
        };

        self.instructions = saved_instructions;
        self.vars = saved_vars;
        self.next_var = saved_next_var;
        self.next_temp = saved_next_temp;
        self.current_func_name = None;

        Ok(func)
    }

    fn lower_stmt(&mut self, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Print(print_stmt) => self.lower_print(print_stmt),
            Stmt::End | Stmt::System => {
                self.emit(Instruction::End);
                Ok(())
            }
            Stmt::Rem => Ok(()),
            Stmt::Let { var, expr } => {
                let temp = self.lower_expr(expr)?;
                let var_key = self.make_var_key(var);
                let vid = self.var_id(&var_key);
                self.emit(Instruction::StoreVar(vid, temp));
                Ok(())
            }
            Stmt::If(if_stmt) => self.lower_if(if_stmt),
            Stmt::For(for_stmt) => self.lower_for(for_stmt),
            Stmt::WhileWend { condition, body } => self.lower_while(condition, body),
            Stmt::DoLoop(do_loop) => self.lower_do_loop(do_loop),
            Stmt::Declare(_) => Ok(()), // forward declarations are no-ops
            Stmt::FunctionDef(_) => Ok(()), // handled separately
            Stmt::SubDef(_) => Ok(()),      // handled separately
            Stmt::Dim(decls) => {
                // Just ensure variables exist with default values
                for decl in decls {
                    let var_key = decl.name.to_uppercase();
                    self.var_id(&var_key);
                }
                Ok(())
            }
            Stmt::Const { name, value } => {
                let temp = self.lower_expr(value)?;
                let vid = self.var_id(&name.to_uppercase());
                self.emit(Instruction::StoreVar(vid, temp));
                Ok(())
            }
            Stmt::ExitFor => {
                // Will be patched by enclosing for loop — emit a placeholder jump
                // Actually, we need a label for exit. We handle this via the for loop context.
                Err("EXIT FOR not yet supported in compiler".to_string())
            }
            Stmt::ExitDo => {
                Err("EXIT DO not yet supported in compiler".to_string())
            }
            Stmt::ExitFunction => {
                // Load function return value and return
                if let Some(ref fname) = self.current_func_name.clone() {
                    let ret_var = self.vars[fname];
                    let ret_temp = self.alloc_temp();
                    self.emit(Instruction::LoadVar(ret_temp, ret_var));
                    self.emit(Instruction::ReturnFunc(ret_temp));
                }
                Ok(())
            }
            Stmt::Call { name, args } => {
                let uname = name.to_uppercase();
                let mut arg_temps = Vec::new();
                for arg in args {
                    let t = self.lower_expr(arg)?;
                    arg_temps.push(t);
                }
                let result = self.alloc_temp();
                if self.func_names.contains(&uname) {
                    self.emit(Instruction::CallFunc(result, uname, arg_temps));
                } else {
                    self.emit(Instruction::CallBuiltin(result, uname, arg_temps));
                }
                Ok(())
            }
            Stmt::ExprStmt(expr) => {
                // Evaluate expression for side effects (e.g., SUB calls parsed as expressions)
                let _t = self.lower_expr(expr)?;
                Ok(())
            }
            Stmt::Goto(_) | Stmt::Gosub(_) | Stmt::Return => {
                Err(format!("GOTO/GOSUB/RETURN not yet supported in compiler"))
            }
            Stmt::SelectCase(sc) => self.lower_select_case(sc),
            Stmt::Swap { a, b } => {
                let ak = self.make_var_key(a);
                let bk = self.make_var_key(b);
                let a_vid = self.var_id(&ak);
                let b_vid = self.var_id(&bk);
                let ta = self.alloc_temp();
                let tb = self.alloc_temp();
                self.emit(Instruction::LoadVar(ta, a_vid));
                self.emit(Instruction::LoadVar(tb, b_vid));
                self.emit(Instruction::StoreVar(a_vid, tb));
                self.emit(Instruction::StoreVar(b_vid, ta));
                Ok(())
            }
            _ => Err(format!("unsupported statement in compiler: {:?}", std::mem::discriminant(stmt))),
        }
    }

    fn make_var_key(&self, var: &Variable) -> String {
        let mut key = var.name.to_uppercase();
        if let Some(suffix) = &var.suffix {
            key.push(suffix.to_char());
        }
        key
    }

    fn lower_print(&mut self, print_stmt: &PrintStmt) -> Result<(), String> {
        if print_stmt.format.is_some() {
            return Err("PRINT USING not yet supported in compiler".to_string());
        }

        for item in &print_stmt.items {
            match item {
                PrintItem::Expr(expr) => {
                    let temp = self.lower_expr(expr)?;
                    self.emit(Instruction::PrintValue(temp, 0));
                }
                PrintItem::Comma => {
                    self.emit(Instruction::PrintComma);
                }
                PrintItem::Tab(_) | PrintItem::Spc(_) => {
                    return Err("TAB/SPC not yet supported in compiler".to_string());
                }
            }
        }

        match print_stmt.trailing {
            PrintSep::Newline => {
                self.emit(Instruction::PrintNewline);
            }
            PrintSep::Semicolon => {}
            PrintSep::Comma => {
                self.emit(Instruction::PrintComma);
            }
        }

        Ok(())
    }

    fn lower_if(&mut self, if_stmt: &IfStmt) -> Result<(), String> {
        let end_label = self.alloc_label();

        // Main condition
        let cond = self.lower_expr(&if_stmt.condition)?;
        let else_label = self.alloc_label();
        self.emit(Instruction::BranchIfNot(cond, else_label));

        // Then body
        for ls in &if_stmt.then_body {
            self.lower_stmt(&ls.stmt)?;
        }
        self.emit(Instruction::Jump(end_label));

        self.emit(Instruction::Label(else_label));

        // Elseif clauses
        for (elseif_cond, elseif_body) in &if_stmt.elseif_clauses {
            let next_label = self.alloc_label();
            let c = self.lower_expr(elseif_cond)?;
            self.emit(Instruction::BranchIfNot(c, next_label));
            for ls in elseif_body {
                self.lower_stmt(&ls.stmt)?;
            }
            self.emit(Instruction::Jump(end_label));
            self.emit(Instruction::Label(next_label));
        }

        // Else body
        if let Some(else_body) = &if_stmt.else_body {
            for ls in else_body {
                self.lower_stmt(&ls.stmt)?;
            }
        }

        self.emit(Instruction::Label(end_label));
        Ok(())
    }

    fn lower_for(&mut self, for_stmt: &ForStmt) -> Result<(), String> {
        let var_key = self.make_var_key(&for_stmt.var);
        let vid = self.var_id(&var_key);

        // Initialize loop variable
        let start = self.lower_expr(&for_stmt.start)?;
        self.emit(Instruction::StoreVar(vid, start));

        let end_val = self.lower_expr(&for_stmt.end)?;
        // Store end value in a temp var so it's evaluated once
        let end_var_key = format!("__FOR_END_{}", vid);
        let end_vid = self.var_id(&end_var_key);
        self.emit(Instruction::StoreVar(end_vid, end_val));

        let step_val = if let Some(step_expr) = &for_stmt.step {
            let s = self.lower_expr(step_expr)?;
            let step_var_key = format!("__FOR_STEP_{}", vid);
            let step_vid = self.var_id(&step_var_key);
            self.emit(Instruction::StoreVar(step_vid, s));
            Some(step_vid)
        } else {
            None
        };

        let loop_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.emit(Instruction::Label(loop_label));

        // Check condition: if step >= 0, check var <= end; else check var >= end
        let cur = self.alloc_temp();
        self.emit(Instruction::LoadVar(cur, vid));
        let end_t = self.alloc_temp();
        self.emit(Instruction::LoadVar(end_t, end_vid));

        if let Some(step_vid) = step_val {
            // Dynamic step: need runtime check of step sign
            let step_t = self.alloc_temp();
            self.emit(Instruction::LoadVar(step_t, step_vid));
            let zero = self.alloc_temp();
            self.emit(Instruction::LoadConst(zero, Constant::Integer(0)));

            // if step >= 0
            let step_ge = self.alloc_temp();
            self.emit(Instruction::BinOp(step_ge, BinOp::Ge, step_t, zero));
            let neg_step_label = self.alloc_label();
            let check_done_label = self.alloc_label();
            self.emit(Instruction::BranchIfNot(step_ge, neg_step_label));

            // Positive step: check var <= end
            let cond_pos = self.alloc_temp();
            self.emit(Instruction::BinOp(cond_pos, BinOp::Le, cur, end_t));
            self.emit(Instruction::BranchIfNot(cond_pos, end_label));
            self.emit(Instruction::Jump(check_done_label));

            // Negative step: check var >= end
            self.emit(Instruction::Label(neg_step_label));
            let cur2 = self.alloc_temp();
            self.emit(Instruction::LoadVar(cur2, vid));
            let end_t2 = self.alloc_temp();
            self.emit(Instruction::LoadVar(end_t2, end_vid));
            let cond_neg = self.alloc_temp();
            self.emit(Instruction::BinOp(cond_neg, BinOp::Ge, cur2, end_t2));
            self.emit(Instruction::BranchIfNot(cond_neg, end_label));

            self.emit(Instruction::Label(check_done_label));
        } else {
            // Default step is 1 (positive): var <= end
            let cond = self.alloc_temp();
            self.emit(Instruction::BinOp(cond, BinOp::Le, cur, end_t));
            self.emit(Instruction::BranchIfNot(cond, end_label));
        }

        // Body
        for ls in &for_stmt.body {
            self.lower_stmt(&ls.stmt)?;
        }

        // Increment
        let cur_after = self.alloc_temp();
        self.emit(Instruction::LoadVar(cur_after, vid));
        let step_t = if let Some(step_vid) = step_val {
            let t = self.alloc_temp();
            self.emit(Instruction::LoadVar(t, step_vid));
            t
        } else {
            let t = self.alloc_temp();
            self.emit(Instruction::LoadConst(t, Constant::Integer(1)));
            t
        };
        let new_val = self.alloc_temp();
        self.emit(Instruction::BinOp(new_val, BinOp::Add, cur_after, step_t));
        self.emit(Instruction::StoreVar(vid, new_val));

        self.emit(Instruction::Jump(loop_label));
        self.emit(Instruction::Label(end_label));
        Ok(())
    }

    fn lower_while(&mut self, condition: &Expr, body: &[LabeledStmt]) -> Result<(), String> {
        let loop_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.emit(Instruction::Label(loop_label));
        let cond = self.lower_expr(condition)?;
        self.emit(Instruction::BranchIfNot(cond, end_label));

        for ls in body {
            self.lower_stmt(&ls.stmt)?;
        }

        self.emit(Instruction::Jump(loop_label));
        self.emit(Instruction::Label(end_label));
        Ok(())
    }

    fn lower_do_loop(&mut self, do_loop: &DoLoopStmt) -> Result<(), String> {
        let loop_label = self.alloc_label();
        let end_label = self.alloc_label();

        self.emit(Instruction::Label(loop_label));

        // Check condition at top?
        if do_loop.check_at_top {
            if let Some(ref cond_expr) = do_loop.condition {
                let cond = self.lower_expr(cond_expr)?;
                if do_loop.is_while {
                    self.emit(Instruction::BranchIfNot(cond, end_label));
                } else {
                    // UNTIL: exit when true
                    self.emit(Instruction::BranchIf(cond, end_label));
                }
            }
        }

        for ls in &do_loop.body {
            self.lower_stmt(&ls.stmt)?;
        }

        // Check condition at bottom?
        if !do_loop.check_at_top {
            if let Some(ref cond_expr) = do_loop.condition {
                let cond = self.lower_expr(cond_expr)?;
                if do_loop.is_while {
                    // LOOP WHILE: continue if true
                    self.emit(Instruction::BranchIf(cond, loop_label));
                } else {
                    // LOOP UNTIL: continue if false
                    self.emit(Instruction::BranchIfNot(cond, loop_label));
                }
            } else {
                // Infinite loop (DO...LOOP with no condition)
                self.emit(Instruction::Jump(loop_label));
            }
        } else {
            self.emit(Instruction::Jump(loop_label));
        }

        self.emit(Instruction::Label(end_label));
        Ok(())
    }

    fn lower_select_case(&mut self, sc: &SelectCaseStmt) -> Result<(), String> {
        let test_temp = self.lower_expr(&sc.expr)?;
        let end_label = self.alloc_label();

        for case in &sc.cases {
            let next_case_label = self.alloc_label();

            // Evaluate conditions (OR them together)
            // Each CaseTest can be a value, range, or comparison
            let mut first = true;
            let mut combined: Option<TempId> = None;

            for test in &case.tests {
                let match_temp = match test {
                    CaseTest::Value(expr) => {
                        let val = self.lower_expr(expr)?;
                        let result = self.alloc_temp();
                        self.emit(Instruction::BinOp(result, BinOp::Eq, test_temp, val));
                        result
                    }
                    CaseTest::Range(lo, hi) => {
                        let lo_t = self.lower_expr(lo)?;
                        let hi_t = self.lower_expr(hi)?;
                        let ge = self.alloc_temp();
                        self.emit(Instruction::BinOp(ge, BinOp::Ge, test_temp, lo_t));
                        let le = self.alloc_temp();
                        self.emit(Instruction::BinOp(le, BinOp::Le, test_temp, hi_t));
                        let result = self.alloc_temp();
                        self.emit(Instruction::BinOp(result, BinOp::And, ge, le));
                        result
                    }
                    CaseTest::Comparison(cmp_op, expr) => {
                        let val = self.lower_expr(expr)?;
                        let binop = match cmp_op {
                            CompareOp::Eq => BinOp::Eq,
                            CompareOp::Ne => BinOp::Ne,
                            CompareOp::Lt => BinOp::Lt,
                            CompareOp::Gt => BinOp::Gt,
                            CompareOp::Le => BinOp::Le,
                            CompareOp::Ge => BinOp::Ge,
                        };
                        let result = self.alloc_temp();
                        self.emit(Instruction::BinOp(result, binop, test_temp, val));
                        result
                    }
                };

                if first {
                    combined = Some(match_temp);
                    first = false;
                } else {
                    let prev = combined.unwrap();
                    let ored = self.alloc_temp();
                    self.emit(Instruction::BinOp(ored, BinOp::Or, prev, match_temp));
                    combined = Some(ored);
                }
            }

            if let Some(cond) = combined {
                self.emit(Instruction::BranchIfNot(cond, next_case_label));
            }

            for ls in &case.body {
                self.lower_stmt(&ls.stmt)?;
            }
            self.emit(Instruction::Jump(end_label));
            self.emit(Instruction::Label(next_case_label));
        }

        // CASE ELSE
        if let Some(else_body) = &sc.else_body {
            for ls in else_body {
                self.lower_stmt(&ls.stmt)?;
            }
        }

        self.emit(Instruction::Label(end_label));
        Ok(())
    }

    fn lower_expr(&mut self, expr: &Expr) -> Result<TempId, String> {
        match expr {
            Expr::StringLit(s) => {
                let t = self.alloc_temp();
                self.emit(Instruction::LoadConst(t, Constant::Str(s.clone())));
                Ok(t)
            }
            Expr::IntegerLit(n) => {
                let t = self.alloc_temp();
                self.emit(Instruction::LoadConst(t, Constant::Integer(*n)));
                Ok(t)
            }
            Expr::DoubleLit(n) => {
                let t = self.alloc_temp();
                self.emit(Instruction::LoadConst(t, Constant::Double(*n)));
                Ok(t)
            }
            Expr::Paren(inner) => self.lower_expr(inner),
            Expr::BinaryOp { left, op, right } => {
                let l = self.lower_expr(left)?;
                let r = self.lower_expr(right)?;
                let result = self.alloc_temp();
                self.emit(Instruction::BinOp(result, *op, l, r));
                Ok(result)
            }
            Expr::UnaryOp { op, operand } => {
                let o = self.lower_expr(operand)?;
                let result = self.alloc_temp();
                self.emit(Instruction::UnaryOp(result, *op, o));
                Ok(result)
            }
            Expr::Variable(var) => {
                let var_key = self.make_var_key(var);
                let vid = self.var_id(&var_key);
                let t = self.alloc_temp();
                self.emit(Instruction::LoadVar(t, vid));
                Ok(t)
            }
            Expr::FunctionCall { name, args, .. } => {
                let uname = name.to_uppercase();
                let base_name = strip_suffix(&uname).to_string();
                let mut arg_temps = Vec::new();
                for arg in args {
                    let t = self.lower_expr(arg)?;
                    arg_temps.push(t);
                }
                let result = self.alloc_temp();
                if self.func_names.contains(&base_name) {
                    self.emit(Instruction::CallFunc(result, base_name, arg_temps));
                } else {
                    // For builtins, keep the original name (including suffix like LEFT$)
                    self.emit(Instruction::CallBuiltin(result, uname, arg_temps));
                }
                Ok(result)
            }
            Expr::ArrayIndex { name, indices, .. } => {
                let uname = name.to_uppercase();
                let base_name = strip_suffix(&uname).to_string();
                let mut arg_temps = Vec::new();
                for idx in indices {
                    let t = self.lower_expr(idx)?;
                    arg_temps.push(t);
                }
                let result = self.alloc_temp();
                if self.func_names.contains(&base_name) {
                    self.emit(Instruction::CallFunc(result, base_name, arg_temps));
                } else {
                    self.emit(Instruction::CallBuiltin(result, uname, arg_temps));
                }
                Ok(result)
            }
            _ => Err(format!("unsupported expression in compiler: {:?}", std::mem::discriminant(expr))),
        }
    }
}
