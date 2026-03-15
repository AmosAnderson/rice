/// RiceIR → Cranelift IR → native object file.
///
/// Translates the linear IR into Cranelift's SSA-based IR using FunctionBuilder,
/// then emits a native .o file via cranelift-object.

use std::collections::HashMap;

use cranelift_codegen::ir::{types, AbiParam, Block, InstBuilder, MemFlags, Signature, StackSlotData, StackSlotKind, Value as ClifValue};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable as ClifVariable};
use cranelift_module::{DataDescription, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule, ObjectProduct};

use crate::ast::{BinOp, UnaryOp};
use crate::compiler::ir::*;

/// Pre-resolved runtime function IDs
struct RuntimeFuncIds {
    runtime_init: FuncId,
    runtime_shutdown: FuncId,
    value_new_int: FuncId,
    value_new_double: FuncId,
    value_new_string: FuncId,
    #[allow(dead_code)]
    value_drop: FuncId,
    print: FuncId,
    print_newline: FuncId,
    print_comma: FuncId,
    value_binop: FuncId,
    value_unary_op: FuncId,
    value_is_truthy: FuncId,
    builtin_call: FuncId,
}

/// Code generator that translates RiceIR to a native object file
pub struct CodeGenerator {
    module: ObjectModule,
    rt: RuntimeFuncIds,
}

fn make_sig(module: &ObjectModule, params: &[cranelift_codegen::ir::Type], returns: &[cranelift_codegen::ir::Type]) -> Signature {
    let mut sig = module.make_signature();
    for &p in params {
        sig.params.push(AbiParam::new(p));
    }
    for &r in returns {
        sig.returns.push(AbiParam::new(r));
    }
    sig
}

impl CodeGenerator {
    pub fn new() -> Result<Self, String> {
        let mut flag_builder = settings::builder();
        flag_builder
            .set("is_pic", "true")
            .map_err(|e| format!("setting is_pic: {e}"))?;
        flag_builder
            .set("opt_level", "speed")
            .map_err(|e| format!("setting opt_level: {e}"))?;

        let isa_builder = cranelift_native::builder()
            .map_err(|e| format!("native ISA: {e}"))?;
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| format!("finishing ISA: {e}"))?;

        let obj_builder =
            ObjectBuilder::new(isa, "rice_compiled", cranelift_module::default_libcall_names())
                .map_err(|e| format!("object builder: {e}"))?;

        let mut module = ObjectModule::new(obj_builder);

        let i64 = types::I64;
        let i32t = types::I32;
        let f64t = types::F64;

        let sigs = vec![
            ("rice_runtime_init",     make_sig(&module, &[],                                    &[i64])),
            ("rice_runtime_shutdown",  make_sig(&module, &[i64],                                 &[])),
            ("rice_value_new_int",     make_sig(&module, &[i64, i64, i64],                       &[])),
            ("rice_value_new_double",  make_sig(&module, &[f64t, i64, i64],                      &[])),
            ("rice_value_new_string",  make_sig(&module, &[i64, i64, i64],                       &[])),
            ("rice_value_drop",        make_sig(&module, &[i64, i64],                            &[])),
            ("rice_print",             make_sig(&module, &[i64, i64, i64, i32t],                 &[])),
            ("rice_print_newline",     make_sig(&module, &[i64],                                 &[])),
            ("rice_print_comma",       make_sig(&module, &[i64],                                 &[])),
            ("rice_value_binop",       make_sig(&module, &[i64, i64, i32t, i64, i64, i64, i64],  &[])),
            ("rice_value_unary_op",    make_sig(&module, &[i64, i64, i32t, i64, i64],            &[])),
            ("rice_value_is_truthy",   make_sig(&module, &[i64, i64],                            &[i32t])),
            // rice_builtin_call(name: *const c_char, argc: i32, args: *const i64, out_tag: *i64, out_data: *i64)
            ("rice_builtin_call",      make_sig(&module, &[i64, i32t, i64, i64, i64],           &[])),
        ];

        let mut ids: HashMap<&str, FuncId> = HashMap::new();
        for (name, sig) in sigs {
            let fid = module
                .declare_function(name, Linkage::Import, &sig)
                .map_err(|e| format!("declaring {name}: {e}"))?;
            ids.insert(name, fid);
        }

        let rt = RuntimeFuncIds {
            runtime_init:     ids["rice_runtime_init"],
            runtime_shutdown:  ids["rice_runtime_shutdown"],
            value_new_int:     ids["rice_value_new_int"],
            value_new_double:  ids["rice_value_new_double"],
            value_new_string:  ids["rice_value_new_string"],
            value_drop:        ids["rice_value_drop"],
            print:             ids["rice_print"],
            print_newline:     ids["rice_print_newline"],
            print_comma:       ids["rice_print_comma"],
            value_binop:       ids["rice_value_binop"],
            value_unary_op:    ids["rice_value_unary_op"],
            value_is_truthy:   ids["rice_value_is_truthy"],
            builtin_call:      ids["rice_builtin_call"],
        };

        Ok(Self { module, rt })
    }

    pub fn compile(mut self, program: &IrProgram) -> Result<Vec<u8>, String> {
        // Declare user functions first so main can call them
        let mut user_func_ids: HashMap<String, FuncId> = HashMap::new();
        for func in &program.functions {
            // Each user function takes: runtime_ptr, param_tags..., param_datas...
            // Returns: (tag, data) via output pointers → actually returns void,
            // uses last two params as out_tag, out_data
            let param_count = func.params.len();
            // Signature: (runtime_ptr: i64, [tag_i: i64, data_i: i64]... , out_tag: i64, out_data: i64) -> void
            let mut sig = self.module.make_signature();
            sig.params.push(AbiParam::new(types::I64)); // runtime_ptr
            for _ in 0..param_count {
                sig.params.push(AbiParam::new(types::I64)); // param tag
                sig.params.push(AbiParam::new(types::I64)); // param data
            }
            sig.params.push(AbiParam::new(types::I64)); // out_tag ptr
            sig.params.push(AbiParam::new(types::I64)); // out_data ptr

            let name = format!("rice_user_{}", func.name);
            let fid = self.module
                .declare_function(&name, Linkage::Local, &sig)
                .map_err(|e| format!("declaring user func {}: {e}", func.name))?;
            user_func_ids.insert(func.name.clone(), fid);
        }

        self.compile_main(&program.main, &user_func_ids)?;

        for func in &program.functions {
            self.compile_user_func(func, &user_func_ids)?;
        }

        let product: ObjectProduct = self.module.finish();
        Ok(product.emit().map_err(|e| format!("emitting object: {e}"))?)
    }

    fn compile_main(&mut self, func: &IrFunction, user_func_ids: &HashMap<String, FuncId>) -> Result<(), String> {
        let mut sig = self.module.make_signature();
        sig.returns.push(AbiParam::new(types::I32));

        let func_id = self
            .module
            .declare_function("main", Linkage::Export, &sig)
            .map_err(|e| format!("declaring main: {e}"))?;

        let mut ctx = self.module.make_context();
        ctx.func.signature = sig;

        let mut builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // Output slots for runtime function returns
        let out_tag_slot = builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot, 8, 3,
        ));
        let out_data_slot = builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot, 8, 3,
        ));

        // Variable storage: each variable gets two Cranelift variables (tag, data)
        let mut var_tags: Vec<ClifVariable> = Vec::new();
        let mut var_datas: Vec<ClifVariable> = Vec::new();
        let mut next_clif_var = 0u32;
        for _ in 0..func.var_count {
            let tag_var = ClifVariable::from_u32(next_clif_var);
            next_clif_var += 1;
            let data_var = ClifVariable::from_u32(next_clif_var);
            next_clif_var += 1;
            builder.declare_var(tag_var, types::I64);
            builder.declare_var(data_var, types::I64);
            // Initialize to zero (Integer 0)
            let zero = builder.ins().iconst(types::I64, 0);
            builder.def_var(tag_var, zero);
            builder.def_var(data_var, zero);
            var_tags.push(tag_var);
            var_datas.push(data_var);
        }

        // Resolve function refs
        let fn_init = self.module.declare_func_in_func(self.rt.runtime_init, builder.func);

        // Initialize runtime
        let call = builder.ins().call(fn_init, &[]);
        let runtime_ptr = builder.inst_results(call)[0];

        // Store runtime_ptr in a variable so it survives across blocks
        let rt_var = ClifVariable::from_u32(next_clif_var);
        next_clif_var += 1;
        builder.declare_var(rt_var, types::I64);
        builder.def_var(rt_var, runtime_ptr);

        self.compile_body(
            func,
            &mut builder,
            user_func_ids,
            &var_tags,
            &var_datas,
            rt_var,
            out_tag_slot,
            out_data_slot,
            next_clif_var,
            true, // is_main
        )?;

        builder.seal_all_blocks();
        builder.finalize();

        self.module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("defining main: {e}"))?;

        Ok(())
    }

    fn compile_user_func(&mut self, func: &IrFunction, user_func_ids: &HashMap<String, FuncId>) -> Result<(), String> {
        let fid = user_func_ids[&func.name];

        let param_count = func.params.len();
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(types::I64)); // runtime_ptr
        for _ in 0..param_count {
            sig.params.push(AbiParam::new(types::I64)); // tag
            sig.params.push(AbiParam::new(types::I64)); // data
        }
        sig.params.push(AbiParam::new(types::I64)); // out_tag
        sig.params.push(AbiParam::new(types::I64)); // out_data

        let mut ctx = self.module.make_context();
        ctx.func.signature = sig;

        let mut builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

        let entry_block = builder.create_block();
        builder.append_block_params_for_function_params(entry_block);
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        let block_params: Vec<ClifValue> = builder.block_params(entry_block).to_vec();
        let runtime_ptr = block_params[0];
        // out_tag and out_data are the last two params
        let out_tag_ptr_param = block_params[1 + param_count * 2];
        let out_data_ptr_param = block_params[1 + param_count * 2 + 1];

        // Output slots for runtime calls within this function
        let out_tag_slot = builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot, 8, 3,
        ));
        let out_data_slot = builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot, 8, 3,
        ));

        // Variables
        let mut var_tags: Vec<ClifVariable> = Vec::new();
        let mut var_datas: Vec<ClifVariable> = Vec::new();
        let mut next_clif_var = 0u32;
        for _ in 0..func.var_count {
            let tag_var = ClifVariable::from_u32(next_clif_var);
            next_clif_var += 1;
            let data_var = ClifVariable::from_u32(next_clif_var);
            next_clif_var += 1;
            builder.declare_var(tag_var, types::I64);
            builder.declare_var(data_var, types::I64);
            let zero = builder.ins().iconst(types::I64, 0);
            builder.def_var(tag_var, zero);
            builder.def_var(data_var, zero);
            var_tags.push(tag_var);
            var_datas.push(data_var);
        }

        // Initialize parameters from block params
        for (i, _param_name) in func.params.iter().enumerate() {
            let tag_val = block_params[1 + i * 2];
            let data_val = block_params[1 + i * 2 + 1];
            builder.def_var(var_tags[i], tag_val);
            builder.def_var(var_datas[i], data_val);
        }

        let rt_var = ClifVariable::from_u32(next_clif_var);
        next_clif_var += 1;
        builder.declare_var(rt_var, types::I64);
        builder.def_var(rt_var, runtime_ptr);

        // Store out pointers in variables for use by ReturnFunc
        let out_tag_ptr_var = ClifVariable::from_u32(next_clif_var);
        next_clif_var += 1;
        let out_data_ptr_var = ClifVariable::from_u32(next_clif_var);
        next_clif_var += 1;
        builder.declare_var(out_tag_ptr_var, types::I64);
        builder.declare_var(out_data_ptr_var, types::I64);
        builder.def_var(out_tag_ptr_var, out_tag_ptr_param);
        builder.def_var(out_data_ptr_var, out_data_ptr_param);

        self.compile_body(
            func,
            &mut builder,
            user_func_ids,
            &var_tags,
            &var_datas,
            rt_var,
            out_tag_slot,
            out_data_slot,
            next_clif_var,
            false, // not main
        )?;

        builder.seal_all_blocks();
        builder.finalize();

        self.module
            .define_function(fid, &mut ctx)
            .map_err(|e| format!("defining func {}: {e}", func.name))?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn compile_body(
        &mut self,
        func: &IrFunction,
        builder: &mut FunctionBuilder,
        user_func_ids: &HashMap<String, FuncId>,
        var_tags: &[ClifVariable],
        var_datas: &[ClifVariable],
        rt_var: ClifVariable,
        out_tag_slot: cranelift_codegen::ir::StackSlot,
        out_data_slot: cranelift_codegen::ir::StackSlot,
        _next_clif_var: u32,
        is_main: bool,
    ) -> Result<(), String> {
        // Pre-resolve runtime function refs
        let fn_shutdown = self.module.declare_func_in_func(self.rt.runtime_shutdown, builder.func);
        let fn_new_int = self.module.declare_func_in_func(self.rt.value_new_int, builder.func);
        let fn_new_double = self.module.declare_func_in_func(self.rt.value_new_double, builder.func);
        let fn_new_string = self.module.declare_func_in_func(self.rt.value_new_string, builder.func);
        let fn_print = self.module.declare_func_in_func(self.rt.print, builder.func);
        let fn_print_newline = self.module.declare_func_in_func(self.rt.print_newline, builder.func);
        let fn_print_comma = self.module.declare_func_in_func(self.rt.print_comma, builder.func);
        let fn_binop = self.module.declare_func_in_func(self.rt.value_binop, builder.func);
        let fn_unary_op = self.module.declare_func_in_func(self.rt.value_unary_op, builder.func);
        let fn_is_truthy = self.module.declare_func_in_func(self.rt.value_is_truthy, builder.func);
        let fn_builtin_call = self.module.declare_func_in_func(self.rt.builtin_call, builder.func);

        // Resolve user function refs
        let mut user_func_refs: HashMap<String, cranelift_codegen::ir::FuncRef> = HashMap::new();
        for (name, &fid) in user_func_ids {
            let fref = self.module.declare_func_in_func(fid, builder.func);
            user_func_refs.insert(name.clone(), fref);
        }

        // String constants
        let mut string_globals: HashMap<TempId, cranelift_module::DataId> = HashMap::new();
        for inst in &func.instructions {
            if let Instruction::LoadConst(tid, Constant::Str(s)) = inst {
                let name = format!(".str.{}.{}", func.name, tid);
                let data_id = self.module
                    .declare_data(&name, Linkage::Local, false, false)
                    .map_err(|e| format!("declaring string data: {e}"))?;
                let mut data_desc = DataDescription::new();
                let mut bytes = s.as_bytes().to_vec();
                bytes.push(0);
                data_desc.define(bytes.into_boxed_slice());
                self.module
                    .define_data(data_id, &data_desc)
                    .map_err(|e| format!("defining string data: {e}"))?;
                string_globals.insert(*tid, data_id);
            }
        }

        // Pre-scan for labels and create blocks
        let mut label_blocks: HashMap<IrLabel, Block> = HashMap::new();
        for inst in &func.instructions {
            if let Instruction::Label(l) = inst {
                let block = builder.create_block();
                label_blocks.insert(*l, block);
            }
        }

        // Temp values
        let mut temps: HashMap<TempId, (ClifValue, ClifValue)> = HashMap::new();

        let _out_tag_addr = builder.ins().stack_addr(types::I64, out_tag_slot, 0);
        let _out_data_addr = builder.ins().stack_addr(types::I64, out_data_slot, 0);

        let mut block_terminated = false;

        for inst in &func.instructions {
            if block_terminated {
                match inst {
                    Instruction::Label(_) => {}
                    _ => continue,
                }
            }

            match inst {
                Instruction::LoadConst(tid, constant) => {
                    let runtime_ptr = builder.use_var(rt_var);
                    let _ = runtime_ptr; // runtime_ptr not needed for const loading but kept for consistency
                    match constant {
                        Constant::Integer(n) => {
                            let val = builder.ins().iconst(types::I64, *n);
                            let ota = builder.ins().stack_addr(types::I64, out_tag_slot, 0);
                            let oda = builder.ins().stack_addr(types::I64, out_data_slot, 0);
                            builder.ins().call(fn_new_int, &[val, ota, oda]);
                            let tag = builder.ins().load(types::I64, MemFlags::new(), ota, 0);
                            let data = builder.ins().load(types::I64, MemFlags::new(), oda, 0);
                            temps.insert(*tid, (tag, data));
                        }
                        Constant::Double(n) => {
                            let val = builder.ins().f64const(*n);
                            let ota = builder.ins().stack_addr(types::I64, out_tag_slot, 0);
                            let oda = builder.ins().stack_addr(types::I64, out_data_slot, 0);
                            builder.ins().call(fn_new_double, &[val, ota, oda]);
                            let tag = builder.ins().load(types::I64, MemFlags::new(), ota, 0);
                            let data = builder.ins().load(types::I64, MemFlags::new(), oda, 0);
                            temps.insert(*tid, (tag, data));
                        }
                        Constant::Str(_) => {
                            let data_id = string_globals[tid];
                            let gv = self.module.declare_data_in_func(data_id, builder.func);
                            let ptr = builder.ins().global_value(types::I64, gv);
                            let ota = builder.ins().stack_addr(types::I64, out_tag_slot, 0);
                            let oda = builder.ins().stack_addr(types::I64, out_data_slot, 0);
                            builder.ins().call(fn_new_string, &[ptr, ota, oda]);
                            let tag = builder.ins().load(types::I64, MemFlags::new(), ota, 0);
                            let data = builder.ins().load(types::I64, MemFlags::new(), oda, 0);
                            temps.insert(*tid, (tag, data));
                        }
                    }
                }

                Instruction::PrintValue(tid, sep) => {
                    let (tag, data) = temps[tid];
                    let runtime_ptr = builder.use_var(rt_var);
                    let sep_val = builder.ins().iconst(types::I32, *sep as i64);
                    builder.ins().call(fn_print, &[runtime_ptr, tag, data, sep_val]);
                }

                Instruction::PrintComma => {
                    let runtime_ptr = builder.use_var(rt_var);
                    builder.ins().call(fn_print_comma, &[runtime_ptr]);
                }

                Instruction::PrintNewline => {
                    let runtime_ptr = builder.use_var(rt_var);
                    builder.ins().call(fn_print_newline, &[runtime_ptr]);
                }

                Instruction::BinOp(result_tid, op, left_tid, right_tid) => {
                    let (ltag, ldata) = temps[left_tid];
                    let (rtag, rdata) = temps[right_tid];
                    let op_val = builder.ins().iconst(types::I32, binop_to_i32(*op) as i64);
                    let ota = builder.ins().stack_addr(types::I64, out_tag_slot, 0);
                    let oda = builder.ins().stack_addr(types::I64, out_data_slot, 0);
                    builder.ins().call(fn_binop, &[ltag, ldata, op_val, rtag, rdata, ota, oda]);
                    let tag = builder.ins().load(types::I64, MemFlags::new(), ota, 0);
                    let data = builder.ins().load(types::I64, MemFlags::new(), oda, 0);
                    temps.insert(*result_tid, (tag, data));
                }

                Instruction::UnaryOp(result_tid, op, operand_tid) => {
                    let (otag, odata) = temps[operand_tid];
                    let op_val = builder.ins().iconst(types::I32, unaryop_to_i32(*op) as i64);
                    let ota = builder.ins().stack_addr(types::I64, out_tag_slot, 0);
                    let oda = builder.ins().stack_addr(types::I64, out_data_slot, 0);
                    builder.ins().call(fn_unary_op, &[otag, odata, op_val, ota, oda]);
                    let tag = builder.ins().load(types::I64, MemFlags::new(), ota, 0);
                    let data = builder.ins().load(types::I64, MemFlags::new(), oda, 0);
                    temps.insert(*result_tid, (tag, data));
                }

                Instruction::StoreVar(vid, tid) => {
                    let (tag, data) = temps[tid];
                    builder.def_var(var_tags[*vid as usize], tag);
                    builder.def_var(var_datas[*vid as usize], data);
                }

                Instruction::LoadVar(tid, vid) => {
                    let tag = builder.use_var(var_tags[*vid as usize]);
                    let data = builder.use_var(var_datas[*vid as usize]);
                    temps.insert(*tid, (tag, data));
                }

                Instruction::Label(l) => {
                    let target_block = label_blocks[l];
                    if !block_terminated {
                        builder.ins().jump(target_block, &[]);
                    }
                    builder.switch_to_block(target_block);
                    // Don't seal here — back edges from loops may not be emitted yet.
                    // All blocks are sealed via seal_all_blocks() after the loop.
                    block_terminated = false;
                }

                Instruction::Jump(l) => {
                    let target_block = label_blocks[l];
                    builder.ins().jump(target_block, &[]);
                    block_terminated = true;
                }

                Instruction::BranchIf(tid, l) => {
                    let (tag, data) = temps[tid];
                    let call = builder.ins().call(fn_is_truthy, &[tag, data]);
                    let truthy = builder.inst_results(call)[0];
                    let target_block = label_blocks[l];
                    let fall_through = builder.create_block();
                    builder.ins().brif(truthy, target_block, &[], fall_through, &[]);
                    builder.switch_to_block(fall_through);
                }

                Instruction::BranchIfNot(tid, l) => {
                    let (tag, data) = temps[tid];
                    let call = builder.ins().call(fn_is_truthy, &[tag, data]);
                    let truthy = builder.inst_results(call)[0];
                    let target_block = label_blocks[l];
                    let fall_through = builder.create_block();
                    builder.ins().brif(truthy, fall_through, &[], target_block, &[]);
                    builder.switch_to_block(fall_through);
                }

                Instruction::CallFunc(result_tid, name, arg_tids) => {
                    if let Some(&fref) = user_func_refs.get(name) {
                        let runtime_ptr = builder.use_var(rt_var);
                        let mut args = vec![runtime_ptr];
                        for tid in arg_tids {
                            let (tag, data) = temps[tid];
                            args.push(tag);
                            args.push(data);
                        }
                        let ota = builder.ins().stack_addr(types::I64, out_tag_slot, 0);
                        let oda = builder.ins().stack_addr(types::I64, out_data_slot, 0);
                        args.push(ota);
                        args.push(oda);
                        builder.ins().call(fref, &args);
                        let tag = builder.ins().load(types::I64, MemFlags::new(), ota, 0);
                        let data = builder.ins().load(types::I64, MemFlags::new(), oda, 0);
                        temps.insert(*result_tid, (tag, data));
                    } else {
                        return Err(format!("unknown function in codegen: {name}"));
                    }
                }

                Instruction::CallBuiltin(result_tid, name, arg_tids) => {
                    // Build args array on the stack: [tag0, data0, tag1, data1, ...]
                    let argc = arg_tids.len();
                    let args_slot = builder.create_sized_stack_slot(StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        (argc as u32) * 16, // 2 * i64 per arg
                        3,
                    ));

                    for (i, tid) in arg_tids.iter().enumerate() {
                        let (tag, data) = temps[tid];
                        let offset = (i * 16) as i32;
                        let addr = builder.ins().stack_addr(types::I64, args_slot, offset);
                        builder.ins().store(MemFlags::new(), tag, addr, 0);
                        builder.ins().store(MemFlags::new(), data, addr, 8);
                    }

                    // Create string constant for function name
                    let name_data_name = format!(".builtin_name.{}.{}", name, result_tid);
                    let name_data_id = self.module
                        .declare_data(&name_data_name, Linkage::Local, false, false)
                        .map_err(|e| format!("declaring builtin name data: {e}"))?;
                    let mut name_desc = DataDescription::new();
                    let mut name_bytes = name.as_bytes().to_vec();
                    name_bytes.push(0);
                    name_desc.define(name_bytes.into_boxed_slice());
                    self.module
                        .define_data(name_data_id, &name_desc)
                        .map_err(|e| format!("defining builtin name data: {e}"))?;
                    let name_gv = self.module.declare_data_in_func(name_data_id, builder.func);
                    let name_ptr = builder.ins().global_value(types::I64, name_gv);

                    let argc_val = builder.ins().iconst(types::I32, argc as i64);
                    let args_ptr = builder.ins().stack_addr(types::I64, args_slot, 0);
                    let ota = builder.ins().stack_addr(types::I64, out_tag_slot, 0);
                    let oda = builder.ins().stack_addr(types::I64, out_data_slot, 0);
                    builder.ins().call(fn_builtin_call, &[name_ptr, argc_val, args_ptr, ota, oda]);
                    let tag = builder.ins().load(types::I64, MemFlags::new(), ota, 0);
                    let data = builder.ins().load(types::I64, MemFlags::new(), oda, 0);
                    temps.insert(*result_tid, (tag, data));
                }

                Instruction::ReturnFunc(tid) => {
                    if is_main {
                        let runtime_ptr = builder.use_var(rt_var);
                        builder.ins().call(fn_shutdown, &[runtime_ptr]);
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().return_(&[zero]);
                    } else {
                        // Write result to output pointers
                        // We need to find the out_tag_ptr and out_data_ptr variables
                        // They are stored in variables with indices next_clif_var-2 and next_clif_var-1
                        // But we don't have direct access. Instead, use a convention:
                        // out_tag_ptr_var and out_data_ptr_var are always at var_count*2 + 1, var_count*2 + 2
                        let n_vars = var_tags.len() as u32;
                        let out_tag_ptr_var = ClifVariable::from_u32(n_vars * 2 + 1);
                        let out_data_ptr_var = ClifVariable::from_u32(n_vars * 2 + 2);
                        let out_tag_ptr = builder.use_var(out_tag_ptr_var);
                        let out_data_ptr = builder.use_var(out_data_ptr_var);
                        let (tag, data) = temps[tid];
                        builder.ins().store(MemFlags::new(), tag, out_tag_ptr, 0);
                        builder.ins().store(MemFlags::new(), data, out_data_ptr, 0);
                        builder.ins().return_(&[]);
                    }
                    block_terminated = true;
                }

                Instruction::End => {
                    if !block_terminated {
                        if is_main {
                            let runtime_ptr = builder.use_var(rt_var);
                            builder.ins().call(fn_shutdown, &[runtime_ptr]);
                            let zero = builder.ins().iconst(types::I32, 0);
                            builder.ins().return_(&[zero]);
                        } else {
                            builder.ins().return_(&[]);
                        }
                        block_terminated = true;
                    }
                }
            }
        }

        if !block_terminated {
            if is_main {
                let runtime_ptr = builder.use_var(rt_var);
                builder.ins().call(fn_shutdown, &[runtime_ptr]);
                let zero = builder.ins().iconst(types::I32, 0);
                builder.ins().return_(&[zero]);
            } else {
                builder.ins().return_(&[]);
            }
        }

        Ok(())
    }
}

fn unaryop_to_i32(op: UnaryOp) -> i32 {
    match op {
        UnaryOp::Neg => 0,
        UnaryOp::Not => 1,
        UnaryOp::Pos => 2,
    }
}

fn binop_to_i32(op: BinOp) -> i32 {
    match op {
        BinOp::Add => 0,
        BinOp::Sub => 1,
        BinOp::Mul => 2,
        BinOp::Div => 3,
        BinOp::IntDiv => 4,
        BinOp::Mod => 5,
        BinOp::Pow => 6,
        BinOp::Eq => 7,
        BinOp::Ne => 8,
        BinOp::Lt => 9,
        BinOp::Gt => 10,
        BinOp::Le => 11,
        BinOp::Ge => 12,
        BinOp::And => 13,
        BinOp::Or => 14,
        BinOp::Xor => 15,
        BinOp::Eqv => 16,
        BinOp::Imp => 17,
    }
}
