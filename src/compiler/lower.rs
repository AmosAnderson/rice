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
    /// Collected function definitions (e.g., DEF FN lowered during main)
    functions: Vec<IrFunction>,
    /// Track which names are user-defined functions (for call resolution)
    func_names: std::collections::HashSet<String>,
    /// Store SUB/FUNCTION parameter info for TYPE handling
    func_params: HashMap<String, Vec<Param>>,
    /// Name of the function being lowered (for return-value assignment)
    current_func_name: Option<String>,
    /// Stack of loop exit labels: (exit_label, is_for). Used for EXIT FOR / EXIT DO.
    loop_exit_labels: Vec<(IrLabel, bool)>,
    /// Map BASIC labels (line numbers/names) to IR labels
    basic_labels: HashMap<String, IrLabel>,
    /// All GOSUB return continuation labels (for RETURN dispatch)
    gosub_return_labels: Vec<IrLabel>,
    /// Pending labels to emit before the next statement
    pending_labels: Vec<IrLabel>,
    /// Track which names are arrays (not functions) for disambiguation
    array_names: std::collections::HashSet<String>,
    /// Variables that are SHARED (read/write through runtime)
    shared_vars: std::collections::HashSet<String>,
    /// STATIC variable names for current function
    static_var_names: Vec<String>,
    /// Current ON ERROR GOTO handler label (None = no handler)
    current_error_handler: Option<IrLabel>,
    /// Resume points: (index, before_label, after_label) for each failable call
    resume_points: Vec<(i32, IrLabel, IrLabel)>,
    /// Counter for resume point indices
    next_resume_point: i32,
    /// DEF FN single-line definitions to inline at call sites: name -> (params, expr)
    def_fn_inlines: HashMap<String, (Vec<Param>, Expr)>,
    /// BYREF-eligible params for the current function/sub: (param_name, VarId)
    current_byref_params: Vec<(String, VarId)>,
    /// Variables bound to FIELD (need runtime sync for GET/PUT)
    field_var_names: Vec<String>,
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
            func_params: HashMap::new(),
            current_func_name: None,
            loop_exit_labels: Vec::new(),
            basic_labels: HashMap::new(),
            gosub_return_labels: Vec::new(),
            pending_labels: Vec::new(),
            array_names: std::collections::HashSet::new(),
            shared_vars: std::collections::HashSet::new(),
            static_var_names: Vec::new(),
            current_error_handler: None,
            resume_points: Vec::new(),
            next_resume_point: 0,
            def_fn_inlines: HashMap::new(),
            current_byref_params: Vec::new(),
            field_var_names: Vec::new(),
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

    /// Lower an optional expression, returning a temp with the given default if None
    fn lower_optional_expr(&mut self, opt: &Option<Expr>, default: Constant) -> Result<TempId, String> {
        if let Some(expr) = opt {
            self.lower_expr(expr)
        } else {
            let t = self.alloc_temp();
            self.emit(Instruction::LoadConst(t, default));
            Ok(t)
        }
    }

    /// Emit a RuntimeCall instruction, returning the result temp
    fn emit_runtime_call(&mut self, name: &str, args: Vec<TempId>) -> TempId {
        let result = self.alloc_temp();
        self.emit(Instruction::RuntimeCall(result, name.to_string(), args));
        result
    }

    /// Emit a failable RuntimeCall with error checking (for ON ERROR GOTO support).
    /// If an error handler is active, emits SetResumePoint before and CheckError after.
    fn emit_failable_runtime_call(&mut self, name: &str, args: Vec<TempId>) -> TempId {
        if let Some(handler_label) = self.current_error_handler {
            let idx = self.next_resume_point;
            self.next_resume_point += 1;
            let before_label = self.alloc_label();
            self.emit(Instruction::Label(before_label));
            self.emit(Instruction::SetResumePoint(idx));
            let result = self.emit_runtime_call(name, args);
            let after_label = self.alloc_label();
            self.emit(Instruction::CheckError(handler_label));
            self.emit(Instruction::Label(after_label));
            self.resume_points.push((idx, before_label, after_label));
            result
        } else {
            self.emit_runtime_call(name, args)
        }
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
                    let name = fdef.name.to_uppercase();
                    self.func_names.insert(name.clone());
                    self.func_params.insert(name, fdef.params.clone());
                }
                Stmt::SubDef(sdef) => {
                    let name = sdef.name.to_uppercase();
                    self.func_names.insert(name.clone());
                    self.func_params.insert(name, sdef.params.clone());
                }
                Stmt::Declare(decl) => {
                    let name = decl.name.to_uppercase();
                    self.func_names.insert(name.clone());
                    self.func_params.insert(name, decl.params.clone());
                }
                _ => {}
            }
        }
    }

    /// Prescan labels in a list of statements (recursing into nested blocks)
    fn prescan_labels(&mut self, stmts: &[LabeledStmt]) {
        for ls in stmts {
            if let Some(label) = &ls.label {
                let key = label.to_string().to_uppercase();
                if !self.basic_labels.contains_key(&key) {
                    let ir_label = self.alloc_label();
                    self.basic_labels.insert(key, ir_label);
                }
            }
            // Prescan labels in nested blocks
            match &ls.stmt {
                Stmt::If(if_stmt) => {
                    self.prescan_labels(&if_stmt.then_body);
                    for (_, body) in &if_stmt.elseif_clauses {
                        self.prescan_labels(body);
                    }
                    if let Some(else_body) = &if_stmt.else_body {
                        self.prescan_labels(else_body);
                    }
                }
                Stmt::For(for_stmt) => self.prescan_labels(&for_stmt.body),
                Stmt::WhileWend { body, .. } => self.prescan_labels(body),
                Stmt::DoLoop(dl) => self.prescan_labels(&dl.body),
                Stmt::SelectCase(sc) => {
                    for case in &sc.cases {
                        self.prescan_labels(&case.body);
                    }
                    if let Some(else_body) = &sc.else_body {
                        self.prescan_labels(else_body);
                    }
                }
                _ => {}
            }
            // Prescan array names from DIM with dimensions
            if let Stmt::Dim(decls) = &ls.stmt {
                for decl in decls {
                    if decl.dimensions.is_some() {
                        self.array_names.insert(decl.name.to_uppercase());
                    }
                }
            }
        }
    }

    /// Emit any pending BASIC labels before the current statement
    fn emit_pending_labels(&mut self) {
        for label in std::mem::take(&mut self.pending_labels) {
            self.emit(Instruction::Label(label));
        }
    }

    /// Emit the GOSUB return dispatch block (chain of BranchIf for each return point)
    fn emit_gosub_dispatch(&mut self) {
        if self.gosub_return_labels.is_empty() {
            return;
        }
        // Emit the dispatch label
        if let Some(&dispatch_label) = self.basic_labels.get("__GOSUB_DISPATCH__") {
            self.emit(Instruction::Label(dispatch_label));
            let ret_id_vid = self.var_id("__GOSUB_RET_ID__");
            let ret_id_temp = self.alloc_temp();
            self.emit(Instruction::LoadVar(ret_id_temp, ret_id_vid));

            let return_labels = self.gosub_return_labels.clone();
            for (i, &return_label) in return_labels.iter().enumerate() {
                let idx_temp = self.alloc_temp();
                self.emit(Instruction::LoadConst(idx_temp, Constant::Integer(i as i64)));
                let cmp_temp = self.alloc_temp();
                self.emit(Instruction::BinOp(cmp_temp, crate::ast::BinOp::Eq, ret_id_temp, idx_temp));
                self.emit(Instruction::BranchIf(cmp_temp, return_label));
            }
            // Fall through: runtime error (RETURN without GOSUB) — just end
        }
    }

    /// Prescan SUBs/FUNCTIONs for SHARED variable declarations
    fn prescan_shared_vars(&mut self, stmts: &[LabeledStmt]) {
        for ls in stmts {
            match &ls.stmt {
                Stmt::SubDef(sdef) => self.collect_shared_from_body(&sdef.body),
                Stmt::FunctionDef(fdef) => self.collect_shared_from_body(&fdef.body),
                _ => {}
            }
        }
    }

    fn collect_shared_from_body(&mut self, body: &[LabeledStmt]) {
        for ls in body {
            if let Stmt::Shared(vars) = &ls.stmt {
                for var in vars {
                    let key = self.make_var_key(var);
                    self.shared_vars.insert(key);
                }
            }
        }
    }

    /// Prescan and emit DATA items as runtime calls at program start
    fn emit_data_items(&mut self, stmts: &[LabeledStmt]) {
        for ls in stmts {
            if let Stmt::Data(items) = &ls.stmt {
                for item in items {
                    let val_temp = self.alloc_temp();
                    match item {
                        DataItem::Number(n) => {
                            if *n == (*n as i64) as f64 && n.abs() < 1e15 {
                                self.emit(Instruction::LoadConst(val_temp, Constant::Integer(*n as i64)));
                            } else {
                                self.emit(Instruction::LoadConst(val_temp, Constant::Double(*n)));
                            }
                        }
                        DataItem::Str(s) => {
                            self.emit(Instruction::LoadConst(val_temp, Constant::Str(s.clone())));
                        }
                    }
                    self.emit_runtime_call("rice_data_add", vec![val_temp]);
                }
            }
            // Recurse into nested blocks
            match &ls.stmt {
                Stmt::If(if_stmt) => {
                    self.emit_data_items(&if_stmt.then_body);
                    for (_, body) in &if_stmt.elseif_clauses {
                        self.emit_data_items(body);
                    }
                    if let Some(else_body) = &if_stmt.else_body {
                        self.emit_data_items(else_body);
                    }
                }
                Stmt::For(for_stmt) => self.emit_data_items(&for_stmt.body),
                Stmt::WhileWend { body, .. } => self.emit_data_items(body),
                Stmt::DoLoop(dl) => self.emit_data_items(&dl.body),
                Stmt::SelectCase(sc) => {
                    for case in &sc.cases {
                        self.emit_data_items(&case.body);
                    }
                    if let Some(else_body) = &sc.else_body {
                        self.emit_data_items(else_body);
                    }
                }
                Stmt::SubDef(sdef) => self.emit_data_items(&sdef.body),
                Stmt::FunctionDef(fdef) => self.emit_data_items(&fdef.body),
                _ => {}
            }
        }
    }

    /// Lower an entire program to IR
    pub fn lower_program(mut self, program: &Program) -> Result<IrProgram, String> {
        // Pre-scan for function names
        self.prescan_functions(&program.statements);
        // Pre-scan for BASIC labels (line numbers and named labels)
        self.prescan_labels(&program.statements);

        // Prescan for SHARED variable declarations in SUBs/FUNCTIONs
        self.prescan_shared_vars(&program.statements);

        // Emit DATA items at program start (collected from all statements)
        self.emit_data_items(&program.statements);

        // Lower main-level statements (skip function/sub defs)
        for labeled_stmt in &program.statements {
            // Emit BASIC label if present
            if let Some(label) = &labeled_stmt.label {
                let key = label.to_string().to_uppercase();
                if let Some(&ir_label) = self.basic_labels.get(&key) {
                    self.pending_labels.push(ir_label);
                }
            }

            match &labeled_stmt.stmt {
                Stmt::FunctionDef(_) | Stmt::SubDef(_) | Stmt::TypeDef { .. } => continue,
                _ => {
                    self.emit_pending_labels();
                    self.lower_stmt(&labeled_stmt.stmt)?;
                }
            }
        }
        // Emit GOSUB return dispatch block if any GOSUBs were used
        self.emit_gosub_dispatch();

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

        // Start with any functions created during main lowering (e.g., DEF FN)
        let mut functions = std::mem::take(&mut self.functions);
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
        let saved_byref_params = std::mem::take(&mut self.current_byref_params);
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

        // Track BYREF-eligible params
        self.current_byref_params.clear();
        for p in &fdef.params {
            if !p.by_val && !p.is_array {
                let var_key = p.name.to_uppercase();
                let vid = self.vars[&var_key];
                self.current_byref_params.push((var_key, vid));
            }
        }

        // Allocate slot for the return value (function name = variable)
        // Register both bare name and suffixed name so `Double% = ...` finds the same slot
        let ret_vid = self.var_id(&func_name);
        if let Some(ref suffix) = fdef.suffix {
            let suffixed = format!("{}{}", func_name, suffix.to_char());
            self.vars.insert(suffixed, ret_vid);
        }

        // Lower function body
        self.static_var_names.clear();
        for ls in &fdef.body {
            self.lower_stmt(&ls.stmt)?;
        }

        // Save BYREF params and STATIC variables before return
        self.emit_byref_stores();
        self.emit_static_saves(&func_name);

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
        self.static_var_names.clear();
        self.current_byref_params = saved_byref_params;

        Ok(func)
    }

    fn lower_sub(&mut self, sdef: &SubDef) -> Result<IrFunction, String> {
        let saved_instructions = std::mem::take(&mut self.instructions);
        let saved_vars = std::mem::take(&mut self.vars);
        let saved_next_var = self.next_var;
        let saved_next_temp = self.next_temp;
        let saved_byref_params = std::mem::take(&mut self.current_byref_params);
        self.next_var = 0;
        self.next_temp = 0;

        let sub_name = sdef.name.to_uppercase();
        self.current_func_name = Some(sub_name.clone());

        let mut param_names = Vec::new();
        for p in &sdef.params {
            let var_key = p.name.to_uppercase();
            self.var_id(&var_key);
            param_names.push(var_key);
        }

        // Track BYREF-eligible params
        self.current_byref_params.clear();
        for p in &sdef.params {
            if !p.by_val && !p.is_array {
                let var_key = p.name.to_uppercase();
                let vid = self.vars[&var_key];
                self.current_byref_params.push((var_key, vid));
            }
        }

        self.static_var_names.clear();
        let is_static_sub = sdef.is_static;

        for ls in &sdef.body {
            self.lower_stmt(&ls.stmt)?;
        }

        // For STATIC subs, all non-param locals are static
        if is_static_sub {
            for var_name in self.vars.keys().cloned().collect::<Vec<_>>() {
                if !param_names.contains(&var_name) && !var_name.starts_with("__") {
                    if !self.static_var_names.contains(&var_name) {
                        self.static_var_names.push(var_name);
                    }
                }
            }
        }

        // Save BYREF params and STATIC variables before return
        self.emit_byref_stores();
        self.emit_static_saves(&sub_name);

        // For STATIC subs, prepend static loads at the beginning of instructions
        if is_static_sub || !self.static_var_names.is_empty() {
            let static_names = self.static_var_names.clone();
            let mut prefix = Vec::new();
            // We need to emit LoadConst and RuntimeCall at the start
            // Build the prefix instructions
            for var_name in &static_names {
                if let Some(&vid) = self.vars.get(var_name) {
                    let func_t = self.alloc_temp();
                    prefix.push(Instruction::LoadConst(func_t, Constant::Str(sub_name.clone())));
                    let name_t = self.alloc_temp();
                    prefix.push(Instruction::LoadConst(name_t, Constant::Str(var_name.clone())));
                    let result = self.alloc_temp();
                    prefix.push(Instruction::RuntimeCall(result, "rice_static_load".to_string(), vec![func_t, name_t]));
                    prefix.push(Instruction::StoreVar(vid, result));
                }
            }
            // Prepend to instructions
            let mut new_insts = prefix;
            new_insts.append(&mut self.instructions);
            self.instructions = new_insts;
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
        self.static_var_names.clear();
        self.current_byref_params = saved_byref_params;

        Ok(func)
    }

    /// Emit runtime calls to save all STATIC variables for the current function
    fn emit_static_saves(&mut self, func_name: &str) {
        let statics = self.static_var_names.clone();
        for var_name in &statics {
            if let Some(&vid) = self.vars.get(var_name) {
                let func_t = self.alloc_temp();
                self.emit(Instruction::LoadConst(func_t, Constant::Str(func_name.to_string())));
                let name_t = self.alloc_temp();
                self.emit(Instruction::LoadConst(name_t, Constant::Str(var_name.clone())));
                let val_t = self.alloc_temp();
                self.emit(Instruction::LoadVar(val_t, vid));
                self.emit_runtime_call("rice_static_save", vec![func_t, name_t, val_t]);
            }
        }
    }

    /// Emit rice_byref_store for each BYREF-eligible param in the current function
    fn emit_byref_stores(&mut self) {
        let params = self.current_byref_params.clone();
        for (param_name, vid) in &params {
            let key_t = self.alloc_temp();
            self.emit(Instruction::LoadConst(key_t, Constant::Str(param_name.clone())));
            let val_t = self.alloc_temp();
            self.emit(Instruction::LoadVar(val_t, *vid));
            self.emit_runtime_call("rice_byref_store", vec![key_t, val_t]);
        }
    }

    /// Check if a function call has BYREF-eligible args and emit rice_byref_begin.
    /// Returns true if BYREF wrapping was emitted (caller must call `emit_byref_copyback`).
    fn emit_byref_begin_if_needed(&mut self, func_name: &str, args: &[Expr]) -> bool {
        let has_byref = self.func_params.get(func_name).is_some_and(|ps| {
            ps.iter().enumerate().any(|(i, p)| {
                !p.by_val && !p.is_array
                    && matches!(args.get(i), Some(Expr::Variable(_)))
            })
        });
        if has_byref {
            self.emit_runtime_call("rice_byref_begin", vec![]);
        }
        has_byref
    }

    /// After a user function call, copy BYREF-modified values back to caller variables.
    /// Only call this when `emit_byref_begin_if_needed` returned true.
    fn emit_byref_copyback(&mut self, func_name: &str, args: &[Expr]) {
        let params = match self.func_params.get(func_name).cloned() {
            Some(p) => p,
            None => return,
        };
        for (i, param) in params.iter().enumerate() {
            if param.by_val || param.is_array {
                continue;
            }
            if let Some(Expr::Variable(caller_var)) = args.get(i) {
                let param_name = param.name.to_uppercase();
                let key_t = self.alloc_temp();
                self.emit(Instruction::LoadConst(key_t, Constant::Str(param_name)));
                let loaded = self.emit_runtime_call("rice_byref_load", vec![key_t]);
                let var_key = self.make_var_key(caller_var);
                if self.shared_vars.contains(&var_key) {
                    let name_t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(name_t, Constant::Str(var_key)));
                    self.emit_runtime_call("rice_shared_set", vec![name_t, loaded]);
                } else {
                    let vid = self.var_id(&var_key);
                    self.emit(Instruction::StoreVar(vid, loaded));
                }
            }
        }
        self.emit_runtime_call("rice_byref_end", vec![]);
    }

    /// Lower LSET or RSET statement, syncing to runtime field_vars if needed
    fn lower_lset_rset(&mut self, var: &Variable, expr: &Expr, runtime_fn: &str) -> Result<(), String> {
        let var_key = self.make_var_key(var);
        let vid = self.var_id(&var_key);
        let var_temp = self.alloc_temp();
        self.emit(Instruction::LoadVar(var_temp, vid));
        let src = self.lower_expr(expr)?;
        let result = self.emit_runtime_call(runtime_fn, vec![var_temp, src]);
        self.emit(Instruction::StoreVar(vid, result));
        if self.field_var_names.contains(&var_key) {
            let key_t = self.alloc_temp();
            self.emit(Instruction::LoadConst(key_t, Constant::Str(var_key)));
            self.emit_runtime_call("rice_field_var_set", vec![key_t, result]);
        }
        Ok(())
    }

    /// Sync all field variables from runtime field_vars to local compiler variables
    fn emit_field_vars_from_runtime(&mut self) {
        for var_name in self.field_var_names.clone() {
            let key_t = self.alloc_temp();
            self.emit(Instruction::LoadConst(key_t, Constant::Str(var_name.clone())));
            let result = self.emit_runtime_call("rice_field_var_get", vec![key_t]);
            let vid = self.var_id(&var_name);
            self.emit(Instruction::StoreVar(vid, result));
        }
    }

    /// Sync all field variables from local compiler variables to runtime field_vars
    fn emit_field_vars_to_runtime(&mut self) {
        for var_name in self.field_var_names.clone() {
            let vid = self.var_id(&var_name);
            let val_t = self.alloc_temp();
            self.emit(Instruction::LoadVar(val_t, vid));
            let key_t = self.alloc_temp();
            self.emit(Instruction::LoadConst(key_t, Constant::Str(var_name)));
            self.emit_runtime_call("rice_field_var_set", vec![key_t, val_t]);
        }
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
                // Check for array assignment encoded as BinaryOp(Eq, ArrayIndex, rhs)
                if let Expr::BinaryOp { left, op: crate::ast::BinOp::Eq, right } = expr {
                    if let Expr::ArrayIndex { name, suffix, indices } = left.as_ref() {
                        let base_name = name.to_uppercase();
                        let base_stripped = strip_suffix(&base_name).to_string();
                        // Only treat as array if it's known as an array, not a function
                        if !self.func_names.contains(&base_stripped) {
                            self.array_names.insert(base_stripped.clone());
                            let val = self.lower_expr(right)?;
                            let suffix_char = suffix.as_ref().map_or(' ', |s| s.to_char());
                            let name_t = self.alloc_temp();
                            self.emit(Instruction::LoadConst(name_t, Constant::Str(format!("{}{}", base_stripped, suffix_char).trim().to_string())));
                            let mut rt_args = vec![name_t];
                            for idx in indices {
                                let t = self.lower_expr(idx)?;
                                rt_args.push(t);
                            }
                            rt_args.push(val);
                            self.emit_runtime_call("rice_array_set", rt_args);
                            return Ok(());
                        }
                    }
                }
                let temp = self.lower_expr(expr)?;
                let var_key = self.make_var_key(var);
                if self.shared_vars.contains(&var_key) {
                    // SHARED variable — write through runtime
                    let name_t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(name_t, Constant::Str(var_key)));
                    self.emit_runtime_call("rice_shared_set", vec![name_t, temp]);
                } else {
                    let vid = self.var_id(&var_key);
                    self.emit(Instruction::StoreVar(vid, temp));
                }
                Ok(())
            }
            Stmt::If(if_stmt) => self.lower_if(if_stmt),
            Stmt::For(for_stmt) => self.lower_for(for_stmt),
            Stmt::WhileWend { condition, body } => self.lower_while(condition, body),
            Stmt::DoLoop(do_loop) => self.lower_do_loop(do_loop),
            Stmt::OnTimer { .. } | Stmt::TimerOp(_) | Stmt::OnKey { .. } | Stmt::KeyOp { .. } => {
                Err("Event trapping not supported in compiler mode".into())
            }
            Stmt::Declare(_) => Ok(()), // forward declarations are no-ops
            Stmt::FunctionDef(_) => Ok(()), // handled separately
            Stmt::SubDef(_) => Ok(()),      // handled separately
            Stmt::Dim(decls) => {
                for decl in decls {
                    let var_key = decl.name.to_uppercase();
                    if let Some(ref dims) = decl.dimensions {
                        // Array declaration — register with runtime
                        self.array_names.insert(var_key.clone());
                        let suffix_char = decl.suffix.as_ref().map_or(' ', |s| s.to_char());
                        let name_t = self.alloc_temp();
                        self.emit(Instruction::LoadConst(name_t, Constant::Str(format!("{}{}", var_key, suffix_char).trim().to_string())));
                        let mut args = vec![name_t];
                        for (upper, lower) in dims {
                            let u = self.lower_expr(upper)?;
                            args.push(u);
                            if let Some(l) = lower {
                                let lt = self.lower_expr(l)?;
                                args.push(lt);
                            } else {
                                let t = self.alloc_temp();
                                self.emit(Instruction::LoadConst(t, Constant::Integer(-1))); // sentinel
                                args.push(t);
                            }
                        }
                        // If the type is user-defined, pass the type name
                        if let Some(BasicType::UserDefined(ref type_name)) = decl.as_type {
                            let type_t = self.alloc_temp();
                            self.emit(Instruction::LoadConst(type_t, Constant::Str(type_name.to_uppercase())));
                            args.push(type_t);
                        }
                        self.emit_runtime_call("rice_array_dim", args);
                    } else if let Some(BasicType::UserDefined(ref type_name)) = decl.as_type {
                        // TYPE variable — create instance via runtime
                        let name_t = self.alloc_temp();
                        self.emit(Instruction::LoadConst(name_t, Constant::Str(var_key.clone())));
                        let type_t = self.alloc_temp();
                        self.emit(Instruction::LoadConst(type_t, Constant::Str(type_name.to_uppercase())));
                        self.emit_runtime_call("rice_type_create", vec![name_t, type_t]);
                        self.var_id(&var_key);
                    } else {
                        // Scalar variable — just ensure it exists
                        self.var_id(&var_key);
                    }
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
                // Find nearest FOR loop exit label
                if let Some(&(exit_label, _)) = self.loop_exit_labels.iter().rev().find(|(_, is_for)| *is_for) {
                    self.emit(Instruction::Jump(exit_label));
                    Ok(())
                } else {
                    Err("EXIT FOR without enclosing FOR loop".to_string())
                }
            }
            Stmt::ExitDo => {
                // Find nearest DO/WHILE loop exit label
                if let Some(&(exit_label, _)) = self.loop_exit_labels.iter().rev().find(|(_, is_for)| !*is_for) {
                    self.emit(Instruction::Jump(exit_label));
                    Ok(())
                } else {
                    Err("EXIT DO without enclosing DO loop".to_string())
                }
            }
            Stmt::ExitFunction => {
                // Emit BYREF stores before early return
                self.emit_byref_stores();
                // Load function return value and return
                if let Some(ref fname) = self.current_func_name.clone() {
                    let ret_var = self.vars[fname];
                    let ret_temp = self.alloc_temp();
                    self.emit(Instruction::LoadVar(ret_temp, ret_var));
                    self.emit(Instruction::ReturnFunc(ret_temp));
                }
                Ok(())
            }
            Stmt::ExitSub => {
                // Emit BYREF stores before early return
                self.emit_byref_stores();
                let zero = self.alloc_temp();
                self.emit(Instruction::LoadConst(zero, Constant::Integer(0)));
                self.emit(Instruction::ReturnFunc(zero));
                Ok(())
            }
            Stmt::Call { name, args } => {
                let uname = name.to_uppercase();
                let mut arg_temps = Vec::new();
                for arg in args {
                    let t = self.lower_expr(arg)?;
                    arg_temps.push(t);
                }
                // Copy TYPE instance fields before call (for TYPE parameters)
                let params = self.func_params.get(&uname).cloned();
                if let Some(ref params) = params {
                    for (i, param) in params.iter().enumerate() {
                        if let Some(BasicType::UserDefined(_)) = &param.as_type {
                            if let Some(arg_expr) = args.get(i) {
                                // Copy type instance from arg name to param name
                                let arg_name = self.extract_var_name(arg_expr);
                                let param_name = param.name.to_uppercase();
                                let arg_t = self.alloc_temp();
                                self.emit(Instruction::LoadConst(arg_t, Constant::Str(arg_name.clone())));
                                let param_t = self.alloc_temp();
                                self.emit(Instruction::LoadConst(param_t, Constant::Str(param_name.clone())));
                                self.emit_runtime_call("rice_type_copy", vec![arg_t, param_t]);
                            }
                        }
                    }
                }

                let is_user_func = self.func_names.contains(&uname);
                let needs_byref = is_user_func && self.emit_byref_begin_if_needed(&uname, args);

                let result = self.alloc_temp();
                if is_user_func {
                    self.emit(Instruction::CallFunc(result, uname.clone(), arg_temps));
                } else {
                    self.emit(Instruction::CallBuiltin(result, uname.clone(), arg_temps));
                }

                if needs_byref {
                    self.emit_byref_copyback(&uname, args);
                }

                // Copy TYPE instance fields back after call (BYREF)
                if let Some(ref params) = params {
                    for (i, param) in params.iter().enumerate() {
                        if let Some(BasicType::UserDefined(_)) = &param.as_type {
                            if let Some(arg_expr) = args.get(i) {
                                let arg_name = self.extract_var_name(arg_expr);
                                let param_name = param.name.to_uppercase();
                                let param_t = self.alloc_temp();
                                self.emit(Instruction::LoadConst(param_t, Constant::Str(param_name)));
                                let arg_t = self.alloc_temp();
                                self.emit(Instruction::LoadConst(arg_t, Constant::Str(arg_name)));
                                self.emit_runtime_call("rice_type_copy", vec![param_t, arg_t]);
                            }
                        }
                    }
                }
                Ok(())
            }
            Stmt::ExprStmt(expr) => {
                // Evaluate expression for side effects (e.g., SUB calls parsed as expressions)
                let _t = self.lower_expr(expr)?;
                Ok(())
            }
            Stmt::Goto(label) => {
                let key = label.to_string().to_uppercase();
                if let Some(&ir_label) = self.basic_labels.get(&key) {
                    self.emit(Instruction::Jump(ir_label));
                    Ok(())
                } else {
                    Err(format!("undefined label in GOTO: {key}"))
                }
            }
            Stmt::Gosub(label) => {
                let key = label.to_string().to_uppercase();
                if let Some(&ir_label) = self.basic_labels.get(&key) {
                    // Allocate a return continuation label
                    let return_label = self.alloc_label();
                    let return_id = self.gosub_return_labels.len() as i64;
                    self.gosub_return_labels.push(return_label);

                    // Push return_id onto the gosub stack
                    let id_temp = self.alloc_temp();
                    self.emit(Instruction::LoadConst(id_temp, Constant::Integer(return_id)));
                    self.emit_runtime_call("rice_gosub_push", vec![id_temp]);

                    // Jump to the target
                    self.emit(Instruction::Jump(ir_label));

                    // Emit the return continuation label
                    self.emit(Instruction::Label(return_label));
                    Ok(())
                } else {
                    Err(format!("undefined label in GOSUB: {key}"))
                }
            }
            Stmt::Return => {
                // Pop return_id from gosub stack
                let id_temp = self.emit_runtime_call("rice_gosub_pop", vec![]);

                // Store the return id in a special variable
                let ret_id_var_key = "__GOSUB_RET_ID__".to_string();
                let ret_id_vid = self.var_id(&ret_id_var_key);
                self.emit(Instruction::StoreVar(ret_id_vid, id_temp));

                // Ensure the dispatch label exists (only allocate once)
                let dispatch_label = if let Some(&l) = self.basic_labels.get("__GOSUB_DISPATCH__") {
                    l
                } else {
                    let l = self.alloc_label();
                    self.basic_labels.insert("__GOSUB_DISPATCH__".to_string(), l);
                    l
                };
                self.emit(Instruction::Jump(dispatch_label));
                Ok(())
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
            Stmt::OnGoto { expr, labels } => {
                let val = self.lower_expr(expr)?;
                let fall_through = self.alloc_label();
                for (i, label) in labels.iter().enumerate() {
                    let key = label.to_string().to_uppercase();
                    if let Some(&ir_label) = self.basic_labels.get(&key) {
                        let idx_temp = self.alloc_temp();
                        self.emit(Instruction::LoadConst(idx_temp, Constant::Integer((i + 1) as i64)));
                        let cmp = self.alloc_temp();
                        self.emit(Instruction::BinOp(cmp, crate::ast::BinOp::Eq, val, idx_temp));
                        self.emit(Instruction::BranchIf(cmp, ir_label));
                    }
                }
                self.emit(Instruction::Label(fall_through));
                Ok(())
            }
            Stmt::OnGosub { expr, labels } => {
                let val = self.lower_expr(expr)?;
                let fall_through = self.alloc_label();
                // For each target, create a trampoline that pushes return_id and jumps
                let mut trampoline_labels = Vec::new();
                for (i, label) in labels.iter().enumerate() {
                    let key = label.to_string().to_uppercase();
                    if let Some(&ir_label) = self.basic_labels.get(&key) {
                        let trampoline = self.alloc_label();
                        trampoline_labels.push((i, trampoline, ir_label));
                    }
                }
                // Branch to appropriate trampoline
                for &(i, trampoline, _) in &trampoline_labels {
                    let idx_temp = self.alloc_temp();
                    self.emit(Instruction::LoadConst(idx_temp, Constant::Integer((i + 1) as i64)));
                    let cmp = self.alloc_temp();
                    self.emit(Instruction::BinOp(cmp, crate::ast::BinOp::Eq, val, idx_temp));
                    self.emit(Instruction::BranchIf(cmp, trampoline));
                }
                self.emit(Instruction::Jump(fall_through));
                // Emit trampolines
                let return_label = self.alloc_label();
                let return_id = self.gosub_return_labels.len() as i64;
                self.gosub_return_labels.push(return_label);
                for &(_, trampoline, target) in &trampoline_labels {
                    self.emit(Instruction::Label(trampoline));
                    let id_temp = self.alloc_temp();
                    self.emit(Instruction::LoadConst(id_temp, Constant::Integer(return_id)));
                    self.emit_runtime_call("rice_gosub_push", vec![id_temp]);
                    self.emit(Instruction::Jump(target));
                }
                // Return continuation
                self.emit(Instruction::Label(return_label));
                self.emit(Instruction::Jump(fall_through));
                self.emit(Instruction::Label(fall_through));
                // Register dispatch label if not already
                if !self.basic_labels.contains_key("__GOSUB_DISPATCH__") {
                    let dispatch_label = self.alloc_label();
                    self.basic_labels.insert("__GOSUB_DISPATCH__".to_string(), dispatch_label);
                }
                Ok(())
            }
            // DATA is collected during prescan, no-op during execution
            Stmt::Data(_) => Ok(()),
            Stmt::Read(vars) => {
                for var in vars {
                    let result = self.emit_failable_runtime_call("rice_data_read", vec![]);
                    let var_key = self.make_var_key(var);
                    let vid = self.var_id(&var_key);
                    self.emit(Instruction::StoreVar(vid, result));
                }
                Ok(())
            }
            Stmt::Restore(opt_label) => {
                let label_temp = if let Some(label) = opt_label {
                    // Restore to a specific label position
                    let key = label.to_string().to_uppercase();
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Str(key)));
                    t
                } else {
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Str(String::new())));
                    t
                };
                self.emit_runtime_call("rice_data_restore", vec![label_temp]);
                Ok(())
            }
            Stmt::Randomize(opt_expr) => {
                let arg = if let Some(expr) = opt_expr {
                    self.lower_expr(expr)?
                } else {
                    // Sentinel: use -999 tag to indicate "use system time"
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Str("__TIME_SEED__".to_string())));
                    t
                };
                self.emit_runtime_call("rice_randomize", vec![arg]);
                Ok(())
            }
            Stmt::Input(input) => self.lower_input(input),
            Stmt::LineInput { prompt, var } => {
                let prompt_temp = {
                    let t = self.alloc_temp();
                    let s = prompt.as_deref().unwrap_or("");
                    self.emit(Instruction::LoadConst(t, Constant::Str(s.to_string())));
                    t
                };
                let result = self.emit_runtime_call("rice_line_input", vec![prompt_temp]);
                let var_key = self.make_var_key(var);
                let vid = self.var_id(&var_key);
                self.emit(Instruction::StoreVar(vid, result));
                Ok(())
            }
            Stmt::Write(exprs) => {
                let mut arg_temps = Vec::new();
                for expr in exprs {
                    let t = self.lower_expr(expr)?;
                    arg_temps.push(t);
                }
                self.emit_runtime_call("rice_write_values", arg_temps);
                Ok(())
            }
            Stmt::Sleep(opt_expr) => {
                let arg = self.lower_optional_expr(opt_expr, Constant::Integer(0))?;
                self.emit_runtime_call("rice_sleep", vec![arg]);
                Ok(())
            }
            Stmt::Clear => {
                self.emit_runtime_call("rice_clear", vec![]);
                Ok(())
            }
            Stmt::Shell(opt_expr) => {
                let arg = self.lower_optional_expr(opt_expr, Constant::Str(String::new()))?;
                self.emit_failable_runtime_call("rice_shell", vec![arg]);
                Ok(())
            }
            Stmt::Name { old, new } => {
                let old_t = self.lower_expr(old)?;
                let new_t = self.lower_expr(new)?;
                self.emit_failable_runtime_call("rice_name", vec![old_t, new_t]);
                Ok(())
            }
            Stmt::Kill(expr) => {
                let t = self.lower_expr(expr)?;
                self.emit_failable_runtime_call("rice_kill", vec![t]);
                Ok(())
            }
            Stmt::Mkdir(expr) => {
                let t = self.lower_expr(expr)?;
                self.emit_failable_runtime_call("rice_mkdir", vec![t]);
                Ok(())
            }
            Stmt::Rmdir(expr) => {
                let t = self.lower_expr(expr)?;
                self.emit_failable_runtime_call("rice_rmdir", vec![t]);
                Ok(())
            }
            Stmt::Chdir(expr) => {
                let t = self.lower_expr(expr)?;
                self.emit_failable_runtime_call("rice_chdir", vec![t]);
                Ok(())
            }
            Stmt::Cls => {
                self.emit_runtime_call("rice_cls", vec![]);
                Ok(())
            }
            Stmt::Beep => {
                self.emit_runtime_call("rice_beep", vec![]);
                Ok(())
            }
            Stmt::Locate { row, col } => {
                let row_t = if let Some(r) = row {
                    self.lower_expr(r)?
                } else {
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Integer(0)));
                    t
                };
                let col_t = if let Some(c) = col {
                    self.lower_expr(c)?
                } else {
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Integer(0)));
                    t
                };
                self.emit_runtime_call("rice_locate", vec![row_t, col_t]);
                Ok(())
            }
            Stmt::Color { fg, bg } => {
                let fg_t = if let Some(f) = fg {
                    self.lower_expr(f)?
                } else {
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Integer(-1)));
                    t
                };
                let bg_t = if let Some(b) = bg {
                    self.lower_expr(b)?
                } else {
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Integer(-1)));
                    t
                };
                self.emit_runtime_call("rice_color", vec![fg_t, bg_t]);
                Ok(())
            }
            Stmt::Width { columns, rows } => {
                let cols_t = if let Some(c) = columns {
                    self.lower_expr(c)?
                } else {
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Integer(0)));
                    t
                };
                let rows_t = if let Some(r) = rows {
                    self.lower_expr(r)?
                } else {
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Integer(0)));
                    t
                };
                self.emit_runtime_call("rice_width", vec![cols_t, rows_t]);
                Ok(())
            }
            Stmt::ViewPrint { top, bottom } => {
                let top_t = if let Some(t_expr) = top {
                    self.lower_expr(t_expr)?
                } else {
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Integer(0)));
                    t
                };
                let bot_t = if let Some(b_expr) = bottom {
                    self.lower_expr(b_expr)?
                } else {
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Integer(0)));
                    t
                };
                self.emit_runtime_call("rice_view_print", vec![top_t, bot_t]);
                Ok(())
            }
            Stmt::Stop => {
                self.emit(Instruction::End);
                Ok(())
            }
            Stmt::Redim { preserve, decls } => {
                for decl in decls {
                    let name_t = self.alloc_temp();
                    let name_key = decl.name.to_uppercase();
                    self.emit(Instruction::LoadConst(name_t, Constant::Str(name_key)));
                    let preserve_t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(preserve_t, Constant::Integer(if *preserve { 1 } else { 0 })));
                    self.emit_runtime_call("rice_array_redim", vec![name_t, preserve_t]);
                }
                Ok(())
            }
            Stmt::Erase(names) => {
                for name in names {
                    let name_t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(name_t, Constant::Str(name.to_uppercase())));
                    self.emit_runtime_call("rice_array_erase", vec![name_t]);
                }
                Ok(())
            }
            Stmt::OptionBase(base) => {
                let base_t = self.alloc_temp();
                self.emit(Instruction::LoadConst(base_t, Constant::Integer(*base as i64)));
                self.emit_runtime_call("rice_option_base", vec![base_t]);
                Ok(())
            }
            Stmt::Open(open_stmt) => self.lower_open(open_stmt),
            Stmt::Close(file_nums) => {
                if file_nums.is_empty() {
                    self.emit_failable_runtime_call("rice_file_close_all", vec![]);
                } else {
                    for expr in file_nums {
                        let t = self.lower_expr(expr)?;
                        self.emit_failable_runtime_call("rice_file_close", vec![t]);
                    }
                }
                Ok(())
            }
            Stmt::PrintFile(pf) => self.lower_print_file(pf),
            Stmt::WriteFile(wf) => self.lower_write_file(wf),
            Stmt::InputFile(if_stmt) => self.lower_input_file(if_stmt),
            Stmt::LineInputFile { file_num, var } => {
                let fnum = self.lower_expr(file_num)?;
                let result = self.emit_failable_runtime_call("rice_file_line_input", vec![fnum]);
                let var_key = self.make_var_key(var);
                let vid = self.var_id(&var_key);
                self.emit(Instruction::StoreVar(vid, result));
                Ok(())
            }
            Stmt::GetPut(gp) => self.lower_get_put(gp),
            Stmt::Field { file_num, fields } => {
                let fnum = self.lower_expr(file_num)?;
                let mut args = vec![fnum];
                for field in fields {
                    let width_t = self.lower_expr(&field.width)?;
                    let name_t = self.alloc_temp();
                    let var_key = self.make_var_key(&field.var);
                    self.emit(Instruction::LoadConst(name_t, Constant::Str(var_key.clone())));
                    args.push(width_t);
                    args.push(name_t);
                    if !self.field_var_names.contains(&var_key) {
                        self.field_var_names.push(var_key);
                    }
                }
                self.emit_failable_runtime_call("rice_file_field", args);
                // After FIELD, sync the initialized field vars from runtime to local
                self.emit_field_vars_from_runtime();
                Ok(())
            }
            Stmt::Seek { file_num, position } => {
                let fnum = self.lower_expr(file_num)?;
                let pos = self.lower_expr(position)?;
                self.emit_failable_runtime_call("rice_file_seek", vec![fnum, pos]);
                Ok(())
            }
            Stmt::OnErrorGoto(opt_label) => {
                let label_temp = if let Some(label) = opt_label {
                    let key = label.to_string().to_uppercase();
                    if key == "0" {
                        // ON ERROR GOTO 0 disables error handling
                        self.current_error_handler = None;
                        let t = self.alloc_temp();
                        self.emit(Instruction::LoadConst(t, Constant::Integer(0)));
                        t
                    } else {
                        // Resolve the BASIC label to an IR label
                        let ir_label = if let Some(&existing) = self.basic_labels.get(&key) {
                            existing
                        } else {
                            let l = self.alloc_label();
                            self.basic_labels.insert(key.clone(), l);
                            l
                        };
                        self.current_error_handler = Some(ir_label);
                        let t = self.alloc_temp();
                        self.emit(Instruction::LoadConst(t, Constant::Str(key)));
                        t
                    }
                } else {
                    self.current_error_handler = None;
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Integer(0)));
                    t
                };
                self.emit_runtime_call("rice_set_error_handler", vec![label_temp]);
                Ok(())
            }
            Stmt::Resume(target) => {
                match target {
                    ResumeTarget::Default => {
                        self.emit_runtime_call("rice_resume", vec![]);
                        // Emit dispatch to retry the failable call
                        let targets: Vec<(i32, IrLabel)> = self.resume_points.iter()
                            .map(|&(idx, before, _)| (idx, before))
                            .collect();
                        if !targets.is_empty() {
                            self.emit(Instruction::ResumeDispatch(targets));
                        }
                    }
                    ResumeTarget::Next => {
                        self.emit_runtime_call("rice_resume_next", vec![]);
                        // Emit dispatch to continue after the failable call
                        let targets: Vec<(i32, IrLabel)> = self.resume_points.iter()
                            .map(|&(idx, _, after)| (idx, after))
                            .collect();
                        if !targets.is_empty() {
                            self.emit(Instruction::ResumeDispatch(targets));
                        }
                    }
                    ResumeTarget::Label(label) => {
                        let key = label.to_string().to_uppercase();
                        let t = self.alloc_temp();
                        self.emit(Instruction::LoadConst(t, Constant::Str(key.clone())));
                        self.emit_runtime_call("rice_resume_label", vec![t]);
                        if let Some(&ir_label) = self.basic_labels.get(&key) {
                            self.emit(Instruction::Jump(ir_label));
                        }
                    }
                }
                Ok(())
            }
            Stmt::MidAssign { var, start, length, replacement } => {
                let var_key = self.make_var_key(var);
                let vid = self.var_id(&var_key);
                let var_temp = self.alloc_temp();
                self.emit(Instruction::LoadVar(var_temp, vid));
                let start_t = self.lower_expr(start)?;
                let len_t = if let Some(l) = length {
                    self.lower_expr(l)?
                } else {
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(t, Constant::Integer(-1)));
                    t
                };
                let repl_t = self.lower_expr(replacement)?;
                let result = self.emit_runtime_call("rice_mid_assign", vec![var_temp, start_t, len_t, repl_t]);
                self.emit(Instruction::StoreVar(vid, result));
                Ok(())
            }
            Stmt::Lset { var, expr } => self.lower_lset_rset(var, expr, "rice_lset"),
            Stmt::Rset { var, expr } => self.lower_lset_rset(var, expr, "rice_rset"),
            Stmt::Shared(vars) => {
                for var in vars {
                    let var_key = self.make_var_key(var);
                    self.shared_vars.insert(var_key.clone());
                    // Register with runtime and load the global value
                    let name_t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(name_t, Constant::Str(var_key.clone())));
                    let result = self.emit_runtime_call("rice_shared_get", vec![name_t]);
                    let vid = self.var_id(&var_key);
                    self.emit(Instruction::StoreVar(vid, result));
                }
                Ok(())
            }
            Stmt::Static(decls) => {
                let func_name = self.current_func_name.clone();
                for decl in decls {
                    let var_key = decl.name.to_uppercase();
                    self.static_var_names.push(var_key.clone());
                    if let Some(ref fname) = func_name {
                        let func_t = self.alloc_temp();
                        self.emit(Instruction::LoadConst(func_t, Constant::Str(fname.clone())));
                        let name_t = self.alloc_temp();
                        self.emit(Instruction::LoadConst(name_t, Constant::Str(var_key.clone())));
                        let result = self.emit_runtime_call("rice_static_load", vec![func_t, name_t]);
                        let vid = self.var_id(&var_key);
                        self.emit(Instruction::StoreVar(vid, result));
                    }
                }
                Ok(())
            }
            Stmt::DefType { typ, ranges } => {
                let type_id = match typ {
                    BasicType::Integer => 0i64,
                    BasicType::Long => 1,
                    BasicType::Single => 2,
                    BasicType::Double => 3,
                    BasicType::String => 4,
                    _ => 0,
                };
                for (start, end) in ranges {
                    let start_t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(start_t, Constant::Integer(*start as i64)));
                    let end_t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(end_t, Constant::Integer(*end as i64)));
                    let type_t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(type_t, Constant::Integer(type_id)));
                    self.emit_runtime_call("rice_deftype", vec![start_t, end_t, type_t]);
                }
                Ok(())
            }
            Stmt::DefFn { name, params, body } => {
                match body {
                    DefFnBody::SingleLine(expr) => {
                        // Store for inlining at call sites (shares caller scope)
                        self.def_fn_inlines.insert(
                            name.to_uppercase(),
                            (params.clone(), expr.clone()),
                        );
                        self.func_names.insert(name.to_uppercase());
                    }
                    DefFnBody::MultiLine(stmts) => {
                        // Multi-line: lower as a separate function (correct behavior)
                        let body_stmts = stmts.clone();
                        let fdef = FunctionDef {
                            name: name.clone(),
                            suffix: None,
                            params: params.clone(),
                            as_type: None,
                            body: body_stmts,
                            is_static: false,
                        };
                        let func = self.lower_function(&fdef)?;
                        self.functions.push(func);
                        self.func_names.insert(name.to_uppercase());
                    }
                }
                Ok(())
            }
            Stmt::TypeDef { .. } => Ok(()), // Handled during prescan
            Stmt::MemberAssign { target, value } => {
                let obj_temp = self.lower_member_target(target)?;
                let val_t = self.lower_expr(value)?;
                self.emit_runtime_call("rice_member_set_dynamic", vec![obj_temp, val_t]);
                Ok(())
            }
            Stmt::Chain { .. } => {
                return Err("CHAIN is not supported in compiled mode; use the interpreter instead".to_string());
            }
            Stmt::Common(common) => {
                // Register COMMON variables for CHAIN
                for cvar in &common.vars {
                    let var_key = {
                        let mut k = cvar.name.to_uppercase();
                        if let Some(suffix) = &cvar.suffix {
                            k.push(suffix.to_char());
                        }
                        k
                    };
                    let name_t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(name_t, Constant::Str(var_key.clone())));
                    let vid = self.var_id(&var_key);
                    let val_t = self.alloc_temp();
                    self.emit(Instruction::LoadVar(val_t, vid));
                    self.emit_runtime_call("rice_common_register", vec![name_t, val_t]);
                }
                if common.shared {
                    for cvar in &common.vars {
                        let var_key = {
                            let mut k = cvar.name.to_uppercase();
                            if let Some(suffix) = &cvar.suffix {
                                k.push(suffix.to_char());
                            }
                            k
                        };
                        self.shared_vars.insert(var_key);
                    }
                }
                Ok(())
            }
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
        if let Some(ref format_expr) = print_stmt.format {
            // PRINT USING
            let fmt_t = self.lower_expr(format_expr)?;
            let mut arg_temps = vec![fmt_t];
            for item in &print_stmt.items {
                if let PrintItem::Expr(expr) = item {
                    let t = self.lower_expr(expr)?;
                    arg_temps.push(t);
                }
            }
            // Pass trailing separator as last arg
            let trailing = Self::print_sep_to_i64(&print_stmt.trailing);
            let trailing_t = self.alloc_temp();
            self.emit(Instruction::LoadConst(trailing_t, Constant::Integer(trailing)));
            arg_temps.push(trailing_t);
            self.emit_runtime_call("rice_print_using", arg_temps);
            return Ok(());
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
                PrintItem::Tab(expr) => {
                    let t = self.lower_expr(expr)?;
                    self.emit_runtime_call("rice_print_tab", vec![t]);
                }
                PrintItem::Spc(expr) => {
                    let t = self.lower_expr(expr)?;
                    self.emit_runtime_call("rice_print_spc", vec![t]);
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

        // Body (with exit label tracking)
        self.loop_exit_labels.push((end_label, true));
        for ls in &for_stmt.body {
            self.lower_stmt(&ls.stmt)?;
        }
        self.loop_exit_labels.pop();

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

        self.loop_exit_labels.push((end_label, false));
        for ls in body {
            self.lower_stmt(&ls.stmt)?;
        }
        self.loop_exit_labels.pop();

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

        self.loop_exit_labels.push((end_label, false));
        for ls in &do_loop.body {
            self.lower_stmt(&ls.stmt)?;
        }
        self.loop_exit_labels.pop();

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
                let base_name = strip_suffix(&var_key).to_string();
                // Check for special runtime variables/functions (no-args functions)
                if Self::is_runtime_variable(&base_name) {
                    Ok(self.emit_runtime_call(&format!("rice_fn_{}", base_name.to_lowercase()), vec![]))
                } else if self.shared_vars.contains(&var_key) {
                    let name_t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(name_t, Constant::Str(var_key)));
                    Ok(self.emit_runtime_call("rice_shared_get", vec![name_t]))
                } else {
                    let vid = self.var_id(&var_key);
                    let t = self.alloc_temp();
                    self.emit(Instruction::LoadVar(t, vid));
                    Ok(t)
                }
            }
            Expr::FunctionCall { name, suffix, args } => {
                let uname = name.to_uppercase();
                let base_name = strip_suffix(&uname).to_string();
                let mut arg_temps = Vec::new();
                for arg in args {
                    let t = self.lower_expr(arg)?;
                    arg_temps.push(t);
                }
                // Check if this is a runtime function that needs RiceRuntime state
                if Self::is_runtime_function(&base_name) {
                    Ok(self.emit_runtime_call(&format!("rice_fn_{}", base_name.to_lowercase()), arg_temps))
                } else if self.array_names.contains(&base_name) {
                    // Known array — use runtime array access (same as ArrayIndex)
                    let suffix_char = suffix.as_ref().map_or(' ', |s| s.to_char());
                    let name_t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(name_t, Constant::Str(format!("{}{}", base_name, suffix_char).trim().to_string())));
                    let mut rt_args = vec![name_t];
                    rt_args.extend(arg_temps);
                    Ok(self.emit_runtime_call("rice_array_get", rt_args))
                } else if let Some((params, expr)) = self.def_fn_inlines.get(&base_name).cloned() {
                    // Inline single-line DEF FN at call site (shares caller scope)
                    // Save old param values, bind args to params, eval expr, restore
                    let mut saved = Vec::new();
                    for (i, param) in params.iter().enumerate() {
                        let param_key = param.name.to_uppercase();
                        let vid = self.var_id(&param_key);
                        let save_t = self.alloc_temp();
                        self.emit(Instruction::LoadVar(save_t, vid));
                        saved.push((vid, save_t));
                        if let Some(&arg_t) = arg_temps.get(i) {
                            self.emit(Instruction::StoreVar(vid, arg_t));
                        }
                    }
                    let result = self.lower_expr(&expr)?;
                    // Restore old param values
                    for (vid, save_t) in saved {
                        self.emit(Instruction::StoreVar(vid, save_t));
                    }
                    Ok(result)
                } else if self.func_names.contains(&base_name) {
                    let needs_byref = self.emit_byref_begin_if_needed(&base_name, args);
                    let result = self.alloc_temp();
                    self.emit(Instruction::CallFunc(result, base_name.clone(), arg_temps));
                    if needs_byref { self.emit_byref_copyback(&base_name, args); }
                    Ok(result)
                } else {
                    // For builtins, keep the original name (including suffix like LEFT$)
                    let result = self.alloc_temp();
                    self.emit(Instruction::CallBuiltin(result, uname, arg_temps));
                    Ok(result)
                }
            }
            Expr::ArrayIndex { name, suffix, indices } => {
                let uname = name.to_uppercase();
                let base_name = strip_suffix(&uname).to_string();
                let mut arg_temps = Vec::new();
                for idx in indices {
                    let t = self.lower_expr(idx)?;
                    arg_temps.push(t);
                }
                if self.func_names.contains(&base_name) {
                    let needs_byref = self.emit_byref_begin_if_needed(&base_name, indices);
                    let result = self.alloc_temp();
                    self.emit(Instruction::CallFunc(result, base_name.clone(), arg_temps));
                    if needs_byref { self.emit_byref_copyback(&base_name, indices); }
                    Ok(result)
                } else if self.array_names.contains(&base_name) {
                    // Known array — use runtime array access
                    let suffix_char = suffix.as_ref().map_or(' ', |s| s.to_char());
                    let name_t = self.alloc_temp();
                    self.emit(Instruction::LoadConst(name_t, Constant::Str(format!("{}{}", base_name, suffix_char).trim().to_string())));
                    let mut rt_args = vec![name_t];
                    rt_args.extend(arg_temps);
                    Ok(self.emit_runtime_call("rice_array_get", rt_args))
                } else {
                    // Might be a builtin or an undeclared array — try builtin first
                    let result = self.alloc_temp();
                    self.emit(Instruction::CallBuiltin(result, uname, arg_temps));
                    Ok(result)
                }
            }
            Expr::MemberAccess { object, field } => {
                // Build the object path, then append .FIELD
                let obj_temp = self.lower_member_object(object)?;
                let field_t = self.alloc_temp();
                self.emit(Instruction::LoadConst(field_t, Constant::Str(field.to_uppercase())));
                Ok(self.emit_runtime_call("rice_member_get_dynamic", vec![obj_temp, field_t]))
            }
        }
    }

    /// Extract variable name from an expression (for TYPE parameter passing)
    fn extract_var_name(&self, expr: &Expr) -> String {
        match expr {
            Expr::Variable(var) => self.make_var_key(var),
            Expr::ArrayIndex { name, suffix, .. } => {
                let base = name.to_uppercase();
                let s = suffix.as_ref().map_or(String::new(), |s| s.to_char().to_string());
                format!("{}{}", base, s).trim().to_string()
            }
            _ => "UNKNOWN".to_string(),
        }
    }

    /// Lower a member access object expression to a path string temp
    /// For Variable: returns a const string like "VARNAME"
    /// For ArrayIndex: returns a dynamic path string like "VARNAME_1_2"
    /// For nested MemberAccess: returns "OBJ.FIELD"
    fn lower_member_object(&mut self, expr: &Expr) -> Result<TempId, String> {
        match expr {
            Expr::Variable(var) => {
                let key = self.make_var_key(var);
                let t = self.alloc_temp();
                self.emit(Instruction::LoadConst(t, Constant::Str(key)));
                Ok(t)
            }
            Expr::ArrayIndex { name, suffix, indices }
            | Expr::FunctionCall { name, suffix, args: indices } => {
                // Build a path like "POINTS_1" dynamically
                let base = name.to_uppercase();
                let suffix_char = suffix.as_ref().map_or(String::new(), |s| s.to_char().to_string());
                let name_str = format!("{}{}", base, suffix_char).trim().to_string();
                let name_t = self.alloc_temp();
                self.emit(Instruction::LoadConst(name_t, Constant::Str(name_str)));
                let mut rt_args = vec![name_t];
                for idx in indices {
                    let t = self.lower_expr(idx)?;
                    rt_args.push(t);
                }
                Ok(self.emit_runtime_call("rice_build_array_path", rt_args))
            }
            Expr::MemberAccess { object, field } => {
                let obj_t = self.lower_member_object(object)?;
                let field_t = self.alloc_temp();
                self.emit(Instruction::LoadConst(field_t, Constant::Str(field.to_uppercase())));
                Ok(self.emit_runtime_call("rice_build_member_path", vec![obj_t, field_t]))
            }
            _ => {
                let t = self.alloc_temp();
                self.emit(Instruction::LoadConst(t, Constant::Str("UNKNOWN".to_string())));
                Ok(t)
            }
        }
    }

    fn print_sep_to_i64(sep: &PrintSep) -> i64 {
        match sep {
            PrintSep::Newline => 0,
            PrintSep::Semicolon => 1,
            PrintSep::Comma => 2,
        }
    }

    /// Lower a member assign target to a path string temp
    fn lower_member_target(&mut self, expr: &Expr) -> Result<TempId, String> {
        match expr {
            Expr::MemberAccess { object, field } => {
                let obj_t = self.lower_member_object(object)?;
                let field_t = self.alloc_temp();
                self.emit(Instruction::LoadConst(field_t, Constant::Str(field.to_uppercase())));
                Ok(self.emit_runtime_call("rice_build_member_path", vec![obj_t, field_t]))
            }
            _ => self.lower_member_object(expr),
        }
    }

    /// Check if a function name is a runtime function that needs RiceRuntime state
    fn is_runtime_function(name: &str) -> bool {
        matches!(name, "RND" | "EOF" | "LOF" | "LOC" | "FREEFILE" | "SEEK"
            | "CSRLIN" | "POS" | "ERR" | "ERL"
            | "INKEY$" | "INPUT$" | "SCREEN")
    }

    /// Check if a variable name is actually a no-arg runtime function
    fn is_runtime_variable(name: &str) -> bool {
        matches!(name, "FREEFILE" | "CSRLIN" | "ERR" | "ERL" | "INKEY$")
    }

    /// Build a dotted member access path string from a MemberAccess expression
    fn lower_input(&mut self, input: &InputStmt) -> Result<(), String> {
        // Build prompt string
        let prompt_t = {
            let t = self.alloc_temp();
            let s = input.prompt.as_deref().unwrap_or("");
            self.emit(Instruction::LoadConst(t, Constant::Str(s.to_string())));
            t
        };

        // Build suffix string for all variables
        let suffixes: String = input.vars.iter().map(|v| {
            v.suffix.as_ref().map_or(' ', |s| s.to_char())
        }).collect();
        let suffixes_t = self.alloc_temp();
        self.emit(Instruction::LoadConst(suffixes_t, Constant::Str(suffixes)));

        let count_t = self.alloc_temp();
        self.emit(Instruction::LoadConst(count_t, Constant::Integer(input.vars.len() as i64)));

        // Call rice_input_start to read and parse
        self.emit_runtime_call("rice_input_start", vec![prompt_t, count_t, suffixes_t]);

        // Then get each variable's value
        for (i, var) in input.vars.iter().enumerate() {
            let idx_t = self.alloc_temp();
            self.emit(Instruction::LoadConst(idx_t, Constant::Integer(i as i64)));
            let val = self.emit_runtime_call("rice_input_get", vec![idx_t]);
            let var_key = self.make_var_key(var);
            let vid = self.var_id(&var_key);
            self.emit(Instruction::StoreVar(vid, val));
        }
        Ok(())
    }

    fn lower_open(&mut self, open: &OpenStmt) -> Result<(), String> {
        let filename = self.lower_expr(&open.filename)?;
        let file_num = self.lower_expr(&open.file_num)?;
        let mode_t = self.alloc_temp();
        let mode_val = match open.mode {
            FileMode::Input => 0i64,
            FileMode::Output => 1,
            FileMode::Append => 2,
            FileMode::Random => 3,
            FileMode::Binary => 4,
        };
        self.emit(Instruction::LoadConst(mode_t, Constant::Integer(mode_val)));
        let rec_len = if let Some(ref len_expr) = open.rec_len {
            self.lower_expr(len_expr)?
        } else {
            let t = self.alloc_temp();
            self.emit(Instruction::LoadConst(t, Constant::Integer(128)));
            t
        };
        self.emit_failable_runtime_call("rice_file_open", vec![filename, file_num, mode_t, rec_len]);
        Ok(())
    }

    fn lower_print_file(&mut self, pf: &FilePrintStmt) -> Result<(), String> {
        let fnum = self.lower_expr(&pf.file_num)?;
        if let Some(ref format_expr) = pf.format {
            // PRINT# USING
            let fmt_t = self.lower_expr(format_expr)?;
            let mut args = vec![fnum, fmt_t];
            for item in &pf.items {
                if let PrintItem::Expr(expr) = item {
                    let t = self.lower_expr(expr)?;
                    args.push(t);
                }
            }
            let trailing = Self::print_sep_to_i64(&pf.trailing);
            let trailing_t = self.alloc_temp();
            self.emit(Instruction::LoadConst(trailing_t, Constant::Integer(trailing)));
            args.push(trailing_t);
            self.emit_failable_runtime_call("rice_file_print_using", args);
        } else {
            // Regular PRINT#
            let mut args = vec![fnum];
            for item in &pf.items {
                match item {
                    PrintItem::Expr(expr) => {
                        let t = self.lower_expr(expr)?;
                        args.push(t);
                        // Mark as expression with tag
                        let tag = self.alloc_temp();
                        self.emit(Instruction::LoadConst(tag, Constant::Integer(0))); // 0 = expr
                        args.push(tag);
                    }
                    PrintItem::Comma => {
                        let tag = self.alloc_temp();
                        self.emit(Instruction::LoadConst(tag, Constant::Integer(1))); // 1 = comma
                        let dummy = self.alloc_temp();
                        self.emit(Instruction::LoadConst(dummy, Constant::Integer(0)));
                        args.push(dummy);
                        args.push(tag);
                    }
                    PrintItem::Tab(expr) => {
                        let t = self.lower_expr(expr)?;
                        args.push(t);
                        let tag = self.alloc_temp();
                        self.emit(Instruction::LoadConst(tag, Constant::Integer(2))); // 2 = tab
                        args.push(tag);
                    }
                    PrintItem::Spc(expr) => {
                        let t = self.lower_expr(expr)?;
                        args.push(t);
                        let tag = self.alloc_temp();
                        self.emit(Instruction::LoadConst(tag, Constant::Integer(3))); // 3 = spc
                        args.push(tag);
                    }
                }
            }
            let trailing = Self::print_sep_to_i64(&pf.trailing);
            let trailing_t = self.alloc_temp();
            self.emit(Instruction::LoadConst(trailing_t, Constant::Integer(trailing)));
            args.push(trailing_t);
            self.emit_failable_runtime_call("rice_file_print", args);
        }
        Ok(())
    }

    fn lower_write_file(&mut self, wf: &FileWriteStmt) -> Result<(), String> {
        let fnum = self.lower_expr(&wf.file_num)?;
        let mut args = vec![fnum];
        for expr in &wf.exprs {
            let t = self.lower_expr(expr)?;
            args.push(t);
        }
        self.emit_failable_runtime_call("rice_file_write", args);
        Ok(())
    }

    fn lower_input_file(&mut self, if_stmt: &FileInputStmt) -> Result<(), String> {
        let fnum = self.lower_expr(&if_stmt.file_num)?;
        for var in &if_stmt.vars {
            let suffix_char = var.suffix.as_ref().map_or(' ', |s| s.to_char());
            let suffix_t = self.alloc_temp();
            self.emit(Instruction::LoadConst(suffix_t, Constant::Str(suffix_char.to_string())));
            let result = self.emit_failable_runtime_call("rice_file_input_var", vec![fnum, suffix_t]);
            let var_key = self.make_var_key(var);
            let vid = self.var_id(&var_key);
            self.emit(Instruction::StoreVar(vid, result));
        }
        Ok(())
    }

    fn lower_get_put(&mut self, gp: &GetPutStmt) -> Result<(), String> {
        let fnum = self.lower_expr(&gp.file_num)?;
        let rec = if let Some(ref r) = gp.record {
            self.lower_expr(r)?
        } else {
            let t = self.alloc_temp();
            self.emit(Instruction::LoadConst(t, Constant::Integer(-1)));
            t
        };

        if let Some(ref var) = gp.var {
            let var_key = self.make_var_key(var);
            let var_name_t = self.alloc_temp();
            self.emit(Instruction::LoadConst(var_name_t, Constant::Str(var_key.clone())));
            if gp.is_get {
                let result = self.emit_failable_runtime_call("rice_file_get", vec![fnum, rec, var_name_t]);
                let vid = self.var_id(&var_key);
                self.emit(Instruction::StoreVar(vid, result));
            } else {
                let vid = self.var_id(&var_key);
                let val = self.alloc_temp();
                self.emit(Instruction::LoadVar(val, vid));
                self.emit_failable_runtime_call("rice_file_put", vec![fnum, rec, var_name_t, val]);
            }
        } else {
            if gp.is_get {
                self.emit_failable_runtime_call("rice_file_get_fielded", vec![fnum, rec]);
                // After GET with fields: sync field vars from runtime to local
                self.emit_field_vars_from_runtime();
            } else {
                // Before PUT with fields: sync field vars from local to runtime
                self.emit_field_vars_to_runtime();
                self.emit_failable_runtime_call("rice_file_put_fielded", vec![fnum, rec]);
            }
        }
        Ok(())
    }
}
