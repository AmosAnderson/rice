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
    let exe_name = if cfg!(target_os = "windows") {
        "test_exe.exe"
    } else {
        "test_exe"
    };
    let exe_path = dir.path().join(exe_name);

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

/// Compile-only test (no interpreter comparison, e.g., for programs that crash the interpreter)
fn compile_and_verify(source: &str, expected: &str) {
    let compiled = compile_and_run(source);
    assert_eq!(
        compiled.trim(), expected.trim(),
        "Output mismatch!\nSource:\n{source}\nExpected:\n{expected}\nCompiled:\n{compiled}"
    );
}

// --- Test .bas files ---

#[test]
fn test_compiled_hello_bas() {
    let source = std::fs::read_to_string("tests/programs/hello.bas").unwrap();
    differential(&source);
}

#[test]
fn test_compiled_arithmetic_bas() {
    let source = std::fs::read_to_string("tests/programs/arithmetic.bas").unwrap();
    differential(&source);
}

#[test]
fn test_compiled_variables_bas() {
    let source = std::fs::read_to_string("tests/programs/variables.bas").unwrap();
    differential(&source);
}

#[test]
fn test_compiled_while_loop_bas() {
    let source = std::fs::read_to_string("tests/programs/while_loop.bas").unwrap();
    differential(&source);
}

#[test]
fn test_compiled_do_loop_bas() {
    let source = std::fs::read_to_string("tests/programs/do_loop.bas").unwrap();
    differential(&source);
}

#[test]
fn test_compiled_fizzbuzz_bas() {
    let source = std::fs::read_to_string("tests/programs/fizzbuzz.bas").unwrap();
    differential(&source);
}

#[test]
fn test_compiled_select_case_bas() {
    let source = std::fs::read_to_string("tests/programs/select_case.bas").unwrap();
    differential(&source);
}

#[test]
fn test_compiled_string_funcs_bas() {
    let source = std::fs::read_to_string("tests/programs/string_funcs.bas").unwrap();
    differential(&source);
}

#[test]
fn test_compiled_sub_test_bas() {
    let source = std::fs::read_to_string("tests/programs/sub_test.bas").unwrap();
    differential(&source);
}

#[test]
fn test_compiled_factorial_bas() {
    let source = std::fs::read_to_string("tests/programs/factorial.bas").unwrap();
    differential(&source);
}

// --- Inline feature tests ---

#[test]
fn test_compiled_if_else() {
    differential("DIM x AS INTEGER\nx = 5\nIF x > 3 THEN\n  PRINT \"big\"\nELSE\n  PRINT \"small\"\nEND IF\n");
}

#[test]
fn test_compiled_for_loop() {
    differential("DIM i AS INTEGER\nFOR i = 1 TO 5\n  PRINT i\nNEXT i\n");
}

#[test]
fn test_compiled_for_step() {
    differential("DIM i AS INTEGER\nFOR i = 10 TO 1 STEP -2\n  PRINT i\nNEXT i\n");
}

#[test]
fn test_compiled_while() {
    differential("DIM n AS INTEGER\nn = 1\nWHILE n <= 3\n  PRINT n\n  n = n + 1\nWEND\n");
}

#[test]
fn test_compiled_function_call() {
    differential("DECLARE FUNCTION Double%(n AS INTEGER)\nPRINT Double%(7)\nFUNCTION Double%(n AS INTEGER)\n  Double% = n * 2\nEND FUNCTION\n");
}

#[test]
fn test_compiled_builtin_len() {
    differential("PRINT LEN(\"Hello\")\n");
}

#[test]
fn test_compiled_builtin_left() {
    differential("PRINT LEFT$(\"Hello\", 3)\n");
}

#[test]
fn test_compiled_builtin_abs() {
    differential("PRINT ABS(-42)\n");
}
