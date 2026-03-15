/// Rice BASIC compiler: AST → RiceIR → Cranelift → native executable.

pub mod cranelift_codegen;
pub mod ir;
pub mod linker;
pub mod lower;

use std::path::Path;

use crate::ast::Program;
use crate::compiler::cranelift_codegen::CodeGenerator;
use crate::compiler::lower::Lowerer;

/// Parse BASIC source into an AST.
fn parse(source: &str) -> Result<Program, String> {
    let tokens = crate::lexer::Lexer::new(source)
        .tokenize()
        .map_err(|e| format!("lex error: {e}"))?;
    crate::parser::Parser::new(tokens)
        .parse_program()
        .map_err(|e| format!("parse error: {e}"))
}

/// Compile a BASIC source file to a native executable.
pub fn compile_file(source_path: &str, output_path: &str) -> Result<(), String> {
    let source = std::fs::read_to_string(source_path)
        .map_err(|e| format!("reading {source_path}: {e}"))?;
    compile_source(&source, output_path)
}

/// Compile BASIC source code to a native executable.
pub fn compile_source(source: &str, output_path: &str) -> Result<(), String> {
    let program = parse(source)?;
    let lowerer = Lowerer::new();
    let ir_program = lowerer.lower_program(&program)?;
    let codegen = CodeGenerator::new()?;
    let object_bytes = codegen.compile(&ir_program)?;
    linker::link(&object_bytes, Path::new(output_path))?;
    Ok(())
}

/// Compile BASIC source and return the IR text (for --emit-ir).
pub fn emit_ir(source: &str) -> Result<String, String> {
    let program = parse(source)?;
    let lowerer = Lowerer::new();
    let ir_program = lowerer.lower_program(&program)?;
    Ok(format!("{ir_program}"))
}
