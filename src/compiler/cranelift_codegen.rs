/// RiceIR → Cranelift IR → native object file.
///
/// Translates the linear IR into Cranelift's SSA-based IR using FunctionBuilder,
/// then emits a native .o file via cranelift-object.

use std::collections::HashMap;

use cranelift_codegen::ir::{types, AbiParam, InstBuilder, Signature, Value as ClifValue};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{DataDescription, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule, ObjectProduct};

use crate::ast::{BinOp, UnaryOp};
use crate::compiler::ir::*;

/// Pre-resolved runtime function IDs (avoids string lookups in hot loop)
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

        // Build signatures first (immutable borrow), then declare (mutable borrow)
        let sigs = vec![
            ("rice_runtime_init",     make_sig(&module, &[],                         &[i64])),
            ("rice_runtime_shutdown",  make_sig(&module, &[i64],                      &[])),
            ("rice_value_new_int",     make_sig(&module, &[i64],                      &[i64, i64])),
            ("rice_value_new_double",  make_sig(&module, &[f64t],                     &[i64, i64])),
            ("rice_value_new_string",  make_sig(&module, &[i64],                      &[i64, i64])),
            ("rice_value_drop",        make_sig(&module, &[i64, i64],                 &[])),
            ("rice_print",             make_sig(&module, &[i64, i64, i64, i32t],      &[])),
            ("rice_print_newline",     make_sig(&module, &[i64],                      &[])),
            ("rice_print_comma",       make_sig(&module, &[i64],                      &[])),
            ("rice_value_binop",       make_sig(&module, &[i64, i64, i32t, i64, i64], &[i64, i64])),
            ("rice_value_unary_op",    make_sig(&module, &[i64, i64, i32t],           &[i64, i64])),
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
        };

        Ok(Self { module, rt })
    }

    /// Compile an IR program to a native object file, returning the raw bytes
    pub fn compile(mut self, program: &IrProgram) -> Result<Vec<u8>, String> {
        self.compile_main(&program.main)?;
        let product: ObjectProduct = self.module.finish();
        Ok(product.emit().map_err(|e| format!("emitting object: {e}"))?)
    }

    fn compile_main(&mut self, func: &IrFunction) -> Result<(), String> {
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

        // Pre-resolve all FuncRefs (avoids repeated declare_func_in_func calls)
        let fn_init = self.module.declare_func_in_func(self.rt.runtime_init, builder.func);
        let fn_shutdown = self.module.declare_func_in_func(self.rt.runtime_shutdown, builder.func);
        let fn_new_int = self.module.declare_func_in_func(self.rt.value_new_int, builder.func);
        let fn_new_double = self.module.declare_func_in_func(self.rt.value_new_double, builder.func);
        let fn_new_string = self.module.declare_func_in_func(self.rt.value_new_string, builder.func);
        let fn_print = self.module.declare_func_in_func(self.rt.print, builder.func);
        let fn_print_newline = self.module.declare_func_in_func(self.rt.print_newline, builder.func);
        let fn_print_comma = self.module.declare_func_in_func(self.rt.print_comma, builder.func);
        let fn_binop = self.module.declare_func_in_func(self.rt.value_binop, builder.func);
        let fn_unary_op = self.module.declare_func_in_func(self.rt.value_unary_op, builder.func);

        // Initialize runtime
        let call = builder.ins().call(fn_init, &[]);
        let runtime_ptr = builder.inst_results(call)[0];

        // Temp values: map TempId -> (tag: ClifValue, data: ClifValue)
        let mut temps: HashMap<TempId, (ClifValue, ClifValue)> = HashMap::new();

        // Create data sections for string constants (single pass)
        let mut string_globals: HashMap<TempId, cranelift_module::DataId> = HashMap::new();
        for inst in &func.instructions {
            if let Instruction::LoadConst(tid, Constant::Str(s)) = inst {
                let name = format!(".str.{}", tid);
                let data_id = self
                    .module
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

        let mut block_terminated = false;

        for inst in &func.instructions {
            if block_terminated {
                match inst {
                    Instruction::Label(_) => {}
                    Instruction::End => {}
                    _ => continue,
                }
            }

            match inst {
                Instruction::LoadConst(tid, constant) => match constant {
                    Constant::Integer(n) => {
                        let val = builder.ins().iconst(types::I64, *n);
                        let call = builder.ins().call(fn_new_int, &[val]);
                        let results = builder.inst_results(call);
                        temps.insert(*tid, (results[0], results[1]));
                    }
                    Constant::Double(n) => {
                        let val = builder.ins().f64const(*n);
                        let call = builder.ins().call(fn_new_double, &[val]);
                        let results = builder.inst_results(call);
                        temps.insert(*tid, (results[0], results[1]));
                    }
                    Constant::Str(_) => {
                        let data_id = string_globals[tid];
                        let gv = self.module.declare_data_in_func(data_id, builder.func);
                        let ptr = builder.ins().global_value(types::I64, gv);
                        let call = builder.ins().call(fn_new_string, &[ptr]);
                        let results = builder.inst_results(call);
                        temps.insert(*tid, (results[0], results[1]));
                    }
                },

                Instruction::PrintValue(tid, sep) => {
                    let (tag, data) = temps[tid];
                    let sep_val = builder.ins().iconst(types::I32, *sep as i64);
                    builder.ins().call(fn_print, &[runtime_ptr, tag, data, sep_val]);
                }

                Instruction::PrintComma => {
                    builder.ins().call(fn_print_comma, &[runtime_ptr]);
                }

                Instruction::PrintNewline => {
                    builder.ins().call(fn_print_newline, &[runtime_ptr]);
                }

                Instruction::BinOp(result_tid, op, left_tid, right_tid) => {
                    let (ltag, ldata) = temps[left_tid];
                    let (rtag, rdata) = temps[right_tid];
                    let op_val = builder.ins().iconst(types::I32, binop_to_i32(*op) as i64);
                    let call = builder.ins().call(fn_binop, &[ltag, ldata, op_val, rtag, rdata]);
                    let results = builder.inst_results(call);
                    temps.insert(*result_tid, (results[0], results[1]));
                }

                Instruction::UnaryOp(result_tid, op, operand_tid) => {
                    let (otag, odata) = temps[operand_tid];
                    let op_val = builder.ins().iconst(types::I32, unaryop_to_i32(*op) as i64);
                    let call = builder.ins().call(fn_unary_op, &[otag, odata, op_val]);
                    let results = builder.inst_results(call);
                    temps.insert(*result_tid, (results[0], results[1]));
                }

                Instruction::End => {
                    if !block_terminated {
                        builder.ins().call(fn_shutdown, &[runtime_ptr]);
                        let zero = builder.ins().iconst(types::I32, 0);
                        builder.ins().return_(&[zero]);
                        block_terminated = true;
                    }
                }

                _ => {
                    return Err(format!("unsupported IR instruction in codegen: {:?}", inst));
                }
            }
        }

        if !block_terminated {
            builder.ins().call(fn_shutdown, &[runtime_ptr]);
            let zero = builder.ins().iconst(types::I32, 0);
            builder.ins().return_(&[zero]);
        }

        builder.finalize();

        self.module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("defining main: {e}"))?;

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
