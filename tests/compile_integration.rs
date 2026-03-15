use std::io::{BufReader, Cursor};
use std::process::Command;

use rice::interpreter::SharedOutput;

/// Run source through the interpreter, return output
fn run_bas(source: &str) -> String {
    let output = SharedOutput::new();
    let input = Cursor::new(Vec::<u8>::new());
    let mut interp = rice::interpreter::Interpreter::with_io(
        Box::new(output.clone()),
        Box::new(BufReader::new(input)),
    );
    interp.run_source(source).unwrap();
    output.into_string()
}

/// Compile source to a native executable and run it, return stdout
fn compile_and_run(source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let bas_path = dir.path().join("test.bas");
    let exe_path = dir.path().join("test_exe");

    std::fs::write(&bas_path, source).unwrap();

    rice::compiler::compile_file(
        bas_path.to_str().unwrap(),
        exe_path.to_str().unwrap(),
    )
    .unwrap();

    let output = Command::new(&exe_path)
        .output()
        .expect("failed to run compiled program");

    assert!(
        output.status.success(),
        "compiled program failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).unwrap()
}

/// Differential test: interpret and compile, compare output
fn differential(source: &str) {
    let interpreted = run_bas(source);
    let compiled = compile_and_run(source);
    assert_eq!(
        interpreted, compiled,
        "Output mismatch!\nSource:\n{source}\nInterpreted:\n{interpreted}\nCompiled:\n{compiled}"
    );
}

#[test]
fn test_compiled_hello() {
    differential("PRINT \"Hello, World!\"\n");
}

#[test]
fn test_compiled_multiple_prints() {
    differential("PRINT \"Hello\"\nPRINT \"World\"\n");
}

#[test]
fn test_compiled_print_integer() {
    differential("PRINT 42\n");
}

#[test]
fn test_compiled_print_negative() {
    differential("PRINT -7\n");
}

#[test]
fn test_compiled_print_zero() {
    differential("PRINT 0\n");
}

#[test]
fn test_compiled_print_float() {
    differential("PRINT 3.14\n");
}

#[test]
fn test_compiled_print_expression() {
    differential("PRINT 3 + 4\n");
}

#[test]
fn test_compiled_print_multiply() {
    differential("PRINT 6 * 7\n");
}

#[test]
fn test_compiled_print_subtract() {
    differential("PRINT 10 - 3\n");
}

#[test]
fn test_compiled_print_semicolon() {
    differential("PRINT \"A\"; \"B\"\n");
}

#[test]
fn test_compiled_print_no_newline() {
    differential("PRINT \"Hello\";\nPRINT \" World\"\n");
}

#[test]
fn test_compiled_empty_print() {
    differential("PRINT\n");
}

#[test]
fn test_compiled_end() {
    differential("PRINT \"before\"\nEND\n");
}

#[test]
fn test_compiled_string_concat() {
    differential("PRINT \"Hello\" + \" \" + \"World\"\n");
}

#[test]
fn test_compiled_mixed_expressions() {
    differential("PRINT 2 + 3 * 4\nPRINT (2 + 3) * 4\n");
}

#[test]
fn test_compiled_hello_bas() {
    let source = std::fs::read_to_string("tests/programs/hello.bas").unwrap();
    differential(&source);
}
