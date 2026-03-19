/// RiceIR: intermediate representation between the AST and Cranelift codegen.
///
/// Flattens nested AST blocks into linear label+branch sequences.
/// All operations on values are delegated to runtime function calls.

use std::fmt;

use crate::ast::{BinOp, UnaryOp};

/// Virtual register (SSA-like temporary)
pub type TempId = u32;

/// Label for control flow
pub type IrLabel = u32;

/// Variable slot identifier
pub type VarId = u32;

/// A constant value in the IR
#[derive(Debug, Clone)]
pub enum Constant {
    Integer(i64),
    Double(f64),
    Str(String),
}

/// A single IR instruction
#[derive(Debug, Clone)]
pub enum Instruction {
    /// Load a constant into a temp: temp = constant
    LoadConst(TempId, Constant),

    /// Print a value with a given separator code (0=none, 1=semicolon, 2=comma)
    PrintValue(TempId, i32),

    /// Print zone tab (comma separator) — no value needed
    PrintComma,

    /// Print a newline
    PrintNewline,

    /// Binary operation: result = lhs op rhs (via runtime)
    BinOp(TempId, BinOp, TempId, TempId),

    /// Unary operation: result = op operand (via runtime)
    UnaryOp(TempId, UnaryOp, TempId),

    /// Label marker (target for jumps)
    Label(IrLabel),

    /// Unconditional jump
    Jump(IrLabel),

    /// Jump if temp is truthy
    BranchIf(TempId, IrLabel),

    /// Jump if temp is falsy
    BranchIfNot(TempId, IrLabel),

    /// Store temp into a variable slot
    StoreVar(VarId, TempId),

    /// Load a variable slot into a temp
    LoadVar(TempId, VarId),

    /// Call a user-defined function: result = call func_name(args...)
    CallFunc(TempId, String, Vec<TempId>),

    /// Call a builtin function: result = builtin_name(args...)
    CallBuiltin(TempId, String, Vec<TempId>),

    /// Call a runtime function (passes runtime_ptr automatically): result = rt_func_name(args...)
    RuntimeCall(TempId, String, Vec<TempId>),

    /// Return from a function (sets return value)
    ReturnFunc(TempId),

    /// Terminate the program
    End,

    /// Check runtime error flag; if set, branch to label (for ON ERROR GOTO)
    CheckError(IrLabel),

    /// Set resume point index before a failable call
    SetResumePoint(i32),

    /// Resume dispatch: given resume target from runtime, jump to matching label
    /// Vec of (resume_point_index, target_label)
    ResumeDispatch(Vec<(i32, IrLabel)>),
}

/// An IR function (main program or SUB/FUNCTION)
#[derive(Debug)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<String>,   // parameter variable names (uppercased)
    pub instructions: Vec<Instruction>,
    pub var_count: u32,        // total number of variable slots used
}

/// The complete IR program
#[derive(Debug)]
pub struct IrProgram {
    /// The main program function
    pub main: IrFunction,
    /// User-defined functions
    pub functions: Vec<IrFunction>,
}

impl fmt::Display for IrProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== RiceIR: {} ===", self.main.name)?;
        for (i, inst) in self.main.instructions.iter().enumerate() {
            writeln!(f, "  {:4}: {}", i, format_instruction(inst))?;
        }
        for func in &self.functions {
            writeln!(f, "=== FUNCTION {} ({}) ===", func.name, func.params.join(", "))?;
            for (i, inst) in func.instructions.iter().enumerate() {
                writeln!(f, "  {:4}: {}", i, format_instruction(inst))?;
            }
        }
        Ok(())
    }
}

fn format_instruction(inst: &Instruction) -> String {
    match inst {
        Instruction::LoadConst(t, c) => match c {
            Constant::Integer(n) => format!("t{t} = const_int {n}"),
            Constant::Double(n) => format!("t{t} = const_double {n}"),
            Constant::Str(s) => format!("t{t} = const_str {:?}", s),
        },
        Instruction::PrintValue(t, sep) => format!("print t{t} sep={sep}"),
        Instruction::PrintComma => "print_comma".to_string(),
        Instruction::PrintNewline => "print_newline".to_string(),
        Instruction::BinOp(r, op, l, rr) => format!("t{r} = binop {:?} t{l} t{rr}", op),
        Instruction::UnaryOp(r, op, o) => format!("t{r} = unaryop {:?} t{o}", op),
        Instruction::Label(l) => format!("L{l}:"),
        Instruction::Jump(l) => format!("jump L{l}"),
        Instruction::BranchIf(t, l) => format!("branch_if t{t} L{l}"),
        Instruction::BranchIfNot(t, l) => format!("branch_if_not t{t} L{l}"),
        Instruction::StoreVar(v, t) => format!("var{v} = t{t}"),
        Instruction::LoadVar(t, v) => format!("t{t} = var{v}"),
        Instruction::CallFunc(t, name, args) => {
            let arg_str: Vec<String> = args.iter().map(|a| format!("t{a}")).collect();
            format!("t{t} = call {}({})", name, arg_str.join(", "))
        }
        Instruction::CallBuiltin(t, name, args) => {
            let arg_str: Vec<String> = args.iter().map(|a| format!("t{a}")).collect();
            format!("t{t} = builtin {}({})", name, arg_str.join(", "))
        }
        Instruction::RuntimeCall(t, name, args) => {
            let arg_str: Vec<String> = args.iter().map(|a| format!("t{a}")).collect();
            format!("t{t} = runtime {}({})", name, arg_str.join(", "))
        }
        Instruction::ReturnFunc(t) => format!("return t{t}"),
        Instruction::End => "end".to_string(),
        Instruction::CheckError(l) => format!("check_error L{l}"),
        Instruction::SetResumePoint(idx) => format!("set_resume_point {idx}"),
        Instruction::ResumeDispatch(targets) => {
            let pairs: Vec<String> = targets.iter().map(|(idx, l)| format!("{idx}→L{l}")).collect();
            format!("resume_dispatch [{}]", pairs.join(", "))
        }
    }
}
