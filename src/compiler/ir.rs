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

    /// Terminate the program
    End,
}

/// An IR function (main program or SUB/FUNCTION)
#[derive(Debug)]
pub struct IrFunction {
    pub name: String,
    pub instructions: Vec<Instruction>,
}

/// The complete IR program
#[derive(Debug)]
pub struct IrProgram {
    /// The main program function
    pub main: IrFunction,
    // Future: subs and functions will go here
}

impl fmt::Display for IrProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "=== RiceIR: {} ===", self.main.name)?;
        for (i, inst) in self.main.instructions.iter().enumerate() {
            writeln!(f, "  {:4}: {}", i, format_instruction(inst))?;
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
        Instruction::End => "end".to_string(),
    }
}
