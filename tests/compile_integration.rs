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

// --- Phase 1: EXIT FOR / EXIT DO ---

#[test]
fn test_compiled_exit_for() {
    differential("FOR i = 1 TO 10\n  IF i = 5 THEN EXIT FOR\n  PRINT i\nNEXT\nPRINT \"done\"\n");
}

#[test]
fn test_compiled_exit_do() {
    differential("x = 0\nDO\n  x = x + 1\n  IF x = 3 THEN EXIT DO\nLOOP\nPRINT x\n");
}

// --- Phase 2: GOTO / GOSUB / RETURN ---

#[test]
fn test_compiled_goto() {
    differential("GOTO 20\n10 PRINT \"skip\"\nGOTO 30\n20 PRINT \"target\"\nGOTO 30\n30 PRINT \"done\"\n");
}

#[test]
fn test_compiled_gosub_bas() {
    differential_file("tests/programs/gosub.bas");
}

#[test]
fn test_compiled_on_goto_bas() {
    differential_file("tests/programs/on_goto.bas");
}

// --- Phase 3: DATA/READ/RESTORE, RANDOMIZE/RND ---

#[test]
fn test_compiled_data_read_bas() {
    differential_file("tests/programs/data_read.bas");
}

#[test]
fn test_compiled_randomize_bas() {
    differential_file("tests/programs/randomize.bas");
}

// --- Phase 5: WRITE ---

#[test]
fn test_compiled_write_stmt_bas() {
    differential_file("tests/programs/write_stmt.bas");
}

// --- Phase 6: Console/System ---

#[test]
fn test_compiled_deftype_bas() {
    differential_file("tests/programs/deftype.bas");
}

// --- Phase 7: File I/O ---

#[test]
fn test_compiled_file_text_io_bas() {
    differential_file("tests/programs/file_text_io.bas");
}

#[test]
fn test_compiled_file_write_read_bas() {
    differential_file("tests/programs/file_write_read.bas");
}

#[test]
fn test_compiled_file_append_bas() {
    differential_file("tests/programs/file_append.bas");
}

#[test]
fn test_compiled_file_binary_bas() {
    differential_file("tests/programs/file_binary.bas");
}

#[test]
fn test_compiled_file_freefile_bas() {
    differential_file("tests/programs/file_freefile.bas");
}

// --- Phase 8: SHARED / STATIC / DEF FN ---

#[test]
fn test_compiled_shared_test_bas() {
    differential_file("tests/programs/shared_test.bas");
}

#[test]
fn test_compiled_static_test_bas() {
    differential_file("tests/programs/static_test.bas");
}

#[test]
fn test_compiled_static_sub_bas() {
    differential_file("tests/programs/static_sub.bas");
}

#[test]
fn test_compiled_def_fn_bas() {
    differential_file("tests/programs/def_fn.bas");
}

// --- Phase 9: TYPE ---

#[test]
fn test_compiled_type_basic_bas() {
    differential_file("tests/programs/type_basic.bas");
}

#[test]
fn test_compiled_type_array_bas() {
    differential_file("tests/programs/type_array.bas");
}

#[test]
fn test_compiled_type_sub_bas() {
    differential_file("tests/programs/type_sub.bas");
}

// --- Phase 10: String ops ---

#[test]
fn test_compiled_mid_assign_bas() {
    differential_file("tests/programs/mid_assign.bas");
}

#[test]
fn test_compiled_lset_rset_bas() {
    differential_file("tests/programs/lset_rset.bas");
}

// --- Helpers ---

/// Compile source in `compile_dir`, run compiled binary there, return stdout.
fn compile_and_run_in(source: &str, compile_dir: &std::path::Path) -> String {
    let bas_path = compile_dir.join("test.bas");
    let exe_name = if cfg!(target_os = "windows") { "test_exe.exe" } else { "test_exe" };
    let exe_path = compile_dir.join(exe_name);

    std::fs::write(&bas_path, source).unwrap();
    rice::compiler::compile_file(
        bas_path.to_str().unwrap(),
        exe_path.to_str().unwrap(),
    ).unwrap();

    let output = Command::new(&exe_path)
        .current_dir(compile_dir)
        .output()
        .expect("failed to run compiled program");
    assert!(output.status.success(),
        "compiled program failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

/// Run source through the interpreter in `interp_dir`, return captured output.
fn interpret_in(source: &str, interp_dir: &std::path::Path) -> String {
    let output = SharedOutput::new();
    let input = Cursor::new(Vec::<u8>::new());
    let mut interp = rice::interpreter::Interpreter::with_io(
        Box::new(output.clone()),
        Box::new(BufReader::new(input)),
    );
    let old_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(interp_dir).unwrap();
    interp.run_source(source).unwrap();
    std::env::set_current_dir(&old_dir).unwrap();
    output.into_string()
}

/// Differential test using a .bas file
fn differential_file(path: &str) {
    let source = std::fs::read_to_string(path).unwrap();

    let dir1 = tempfile::tempdir().unwrap();
    let compiled = compile_and_run_in(&source, dir1.path());

    let dir2 = tempfile::tempdir().unwrap();
    let interpreted = interpret_in(&source, dir2.path());

    assert_eq!(interpreted, compiled,
        "Output mismatch for {path}!\nInterpreted:\n{interpreted}\nCompiled:\n{compiled}");
}

/// Differential test with {DIR} placeholder for temp directory file I/O
fn differential_tmpdir(source_template: &str) {
    let dir1 = tempfile::tempdir().unwrap();
    let dir1_str = dir1.path().to_str().unwrap().replace('\\', "/");
    let source1 = source_template.replace("{DIR}", &dir1_str);
    let compiled = compile_and_run_in(&source1, dir1.path());

    let dir2 = tempfile::tempdir().unwrap();
    let dir2_str = dir2.path().to_str().unwrap().replace('\\', "/");
    let source2 = source_template.replace("{DIR}", &dir2_str);
    let interpreted = interpret_in(&source2, dir2.path());

    assert_eq!(interpreted, compiled,
        "Output mismatch!\nInterpreted:\n{interpreted}\nCompiled:\n{compiled}");
}

// --- Phase 1B: SCREEN() function ---
#[test]
fn test_compiled_screen_function() {
    differential("CLS\nPRINT \"AB\"\nPRINT SCREEN(1, 1); SCREEN(1, 2)\n");
}

// --- Phase 1C: Runtime error codes ---
#[test]
fn test_compiled_error_codes() {
    // Error code should be set after failed operation (use ON ERROR to catch it)
    differential(r#"
ON ERROR GOTO handler
NAME "nonexistent_src_12345.txt" AS "dst.txt"
PRINT "continued"
END
handler:
PRINT "ERR ="; ERR
RESUME NEXT
"#);
}

// --- Phase 3: REDIM PRESERVE ---
#[test]
fn test_compiled_redim_preserve() {
    differential(r#"
DIM a(5)
a(1) = 10
a(2) = 20
REDIM PRESERVE a(10)
PRINT a(1); a(2); a(6)
"#);
}

#[test]
fn test_compiled_redim_no_preserve() {
    differential(r#"
DIM a(5)
a(1) = 10
a(2) = 20
REDIM a(10)
PRINT a(1); a(2); a(6)
"#);
}

// --- Phase 4: CHAIN compile-time error ---
#[test]
fn test_compiled_chain_error() {
    let dir = tempfile::tempdir().unwrap();
    let bas_path = dir.path().join("chain_test.bas");
    let exe_path = dir.path().join("chain_test");
    std::fs::write(&bas_path, "CHAIN \"other.bas\"\n").unwrap();
    let result = rice::compiler::compile_file(
        bas_path.to_str().unwrap(),
        exe_path.to_str().unwrap(),
    );
    assert!(result.is_err(), "CHAIN should fail at compile time");
    let err = result.unwrap_err();
    assert!(err.contains("CHAIN is not supported"), "Error should mention CHAIN: {err}");
}

// --- Phase 5: DEF FN single-line scope ---
#[test]
fn test_compiled_def_fn_scope() {
    differential(r#"
x = 10
DEF FNadd(a) = a + x
PRINT FNadd(5)
x = 20
PRINT FNadd(5)
"#);
}

// --- Phase 2: ON ERROR GOTO / RESUME NEXT ---
#[test]
fn test_compiled_on_error_resume_next() {
    differential(r#"
ON ERROR GOTO handler
KILL "nonexistent_file_12345.txt"
PRINT "continued"
END
handler:
PRINT "ERR ="; ERR
RESUME NEXT
"#);
}

#[test]
fn test_compiled_on_error_goto_0() {
    // ON ERROR GOTO 0 disables error handling — handler should not be called
    differential(r#"
ON ERROR GOTO handler
KILL "nonexistent_file_12345.txt"
PRINT "ERR after first ="; ERR
ON ERROR GOTO 0
PRINT "disabled handler"
END
handler:
PRINT "caught error"; ERR
RESUME NEXT
"#);
}

#[test]
fn test_compiled_on_error_err_value() {
    differential(r#"
ON ERROR GOTO handler
KILL "nonexistent_file_12345.txt"
END
handler:
PRINT ERR
RESUME NEXT
"#);
}

#[test]
fn test_compiled_on_error_resume_label() {
    differential(r#"
ON ERROR GOTO handler
KILL "nonexistent_file_12345.txt"
PRINT "after kill"
END
handler:
PRINT "caught error"
RESUME skipover
skipover:
PRINT "skipped to label"
END
"#);
}

// --- BYREF / BYVAL ---

#[test]
fn test_compiled_byref_sub() {
    differential(r#"
DIM x AS INTEGER
x = 10
CALL AddFive(x)
PRINT x

SUB AddFive(n AS INTEGER)
    n = n + 5
END SUB
"#);
}

#[test]
fn test_compiled_byval_sub() {
    differential(r#"
DIM x AS INTEGER
x = 10
CALL AddFive(x)
PRINT x

SUB AddFive(BYVAL n AS INTEGER)
    n = n + 5
END SUB
"#);
}

#[test]
fn test_compiled_byval_paren_forces_byval() {
    differential(r#"
DIM x AS INTEGER
x = 10
CALL AddFive((x))
PRINT x

SUB AddFive(n AS INTEGER)
    n = n + 5
END SUB
"#);
}

#[test]
fn test_compiled_byval_expression_arg() {
    differential(r#"
DIM x AS INTEGER
x = 10
CALL AddFive(x + 0)
PRINT x

SUB AddFive(n AS INTEGER)
    n = n + 5
END SUB
"#);
}

#[test]
fn test_compiled_byref_function() {
    differential(r#"
DIM x AS INTEGER
x = 10
DIM r AS INTEGER
r = Dbl(x)
PRINT x
PRINT r

FUNCTION Dbl(n AS INTEGER)
    n = n * 2
    Dbl = n
END FUNCTION
"#);
}

#[test]
fn test_compiled_byval_function() {
    differential(r#"
DIM x AS INTEGER
x = 10
DIM r AS INTEGER
r = Dbl(x)
PRINT x
PRINT r

FUNCTION Dbl(BYVAL n AS INTEGER)
    n = n * 2
    Dbl = n
END FUNCTION
"#);
}

// --- EXIT SUB ---

#[test]
fn test_compiled_exit_sub() {
    differential(r#"
CALL TestExit(5)
CALL TestExit(15)

SUB TestExit(n AS INTEGER)
    IF n > 10 THEN EXIT SUB
    PRINT n
END SUB
"#);
}

// --- CONST ---

#[test]
fn test_compiled_const() {
    differential(r#"
CONST PI = 3.14159
PRINT PI
"#);
}

// --- Single-line IF ---

#[test]
fn test_compiled_single_line_if() {
    differential("IF 5 > 3 THEN PRINT \"yes\" ELSE PRINT \"no\"\n");
}

// --- PRINT USING ---

#[test]
fn test_compiled_print_using_digits() {
    differential(r####"
PRINT USING "###.##"; 1.5
PRINT USING "###.##"; 123.456
PRINT USING "###.##"; -1.5
"####);
}

#[test]
fn test_compiled_print_using_dollar() {
    differential(r####"
PRINT USING "$$###.##"; 1.5
PRINT USING "$$###.##"; 123.45
"####);
}

#[test]
fn test_compiled_print_using_sign() {
    differential(r####"
PRINT USING "+###"; 5
PRINT USING "+###"; -5
PRINT USING "###-"; 5
PRINT USING "###-"; -5
"####);
}

#[test]
fn test_compiled_print_using_asterisk() {
    differential(r####"
PRINT USING "**###.##"; 1.5
PRINT USING "**$###.##"; 1.5
"####);
}

#[test]
fn test_compiled_print_using_comma() {
    differential(r####"
PRINT USING "#,###.##"; 1234.56
"####);
}

#[test]
fn test_compiled_print_using_scientific() {
    differential(r####"
PRINT USING "##.##^^^^"; 1234.5
"####);
}

#[test]
fn test_compiled_print_using_string() {
    differential(r####"
PRINT USING "!"; "Hello"
PRINT USING "\   \"; "Hello"
PRINT USING "&"; "Hello"
"####);
}

#[test]
fn test_compiled_print_using_repeat() {
    differential(r####"
PRINT USING "###"; 1; 2; 3
"####);
}

// --- Console operations ---

#[test]
fn test_compiled_cls() {
    differential("CLS\n");
}

#[test]
fn test_compiled_beep() {
    differential("BEEP\n");
}

#[test]
fn test_compiled_locate() {
    differential("LOCATE 5, 10\nPRINT \"X\"\n");
}

#[test]
fn test_compiled_color() {
    differential("COLOR 4, 1\nPRINT \"red on blue\"\n");
}

#[test]
fn test_compiled_width() {
    differential("WIDTH 40\nPRINT \"ok\"\n");
}

// --- Binary conversion ---

#[test]
fn test_compiled_binary_conversion() {
    differential(r#"
a$ = MKI$(1000)
PRINT LEN(a$)
PRINT CVI(a$)
b$ = MKL$(123456)
PRINT LEN(b$)
PRINT CVL(b$)
"#);
}

// --- ENVIRON$ ---

#[test]
fn test_compiled_environ() {
    // Only test NONEXISTENT to avoid env var differences between interpreter and compiled
    differential(r#"
y$ = ENVIRON$("NONEXISTENT_VAR_12345")
PRINT LEN(y$)
"#);
}

// --- CLEAR ---
// Note: CLEAR in compiled mode clears runtime-managed state (arrays, DATA) but
// not SSA local variables. This is a known limitation.
#[test]
fn test_compiled_clear() {
    // Just verify CLEAR doesn't crash the compiled program
    compile_and_verify("x = 42\nCLEAR\nPRINT \"ok\"\n", "ok");
}

// --- FIELD / SEEK ---

#[test]
fn test_compiled_field_basic() {
    differential_tmpdir(r#"
OPEN "{DIR}/field.dat" FOR RANDOM AS #1 LEN = 30
FIELD #1, 10 AS N$, 15 AS A$, 5 AS Z$
LSET N$ = "Alice"
LSET A$ = "123 Main St"
LSET Z$ = "12345"
PUT #1, 1
LSET N$ = "Bob"
LSET A$ = "456 Oak Ave"
LSET Z$ = "67890"
PUT #1, 2
GET #1, 1
PRINT RTRIM$(N$)
PRINT RTRIM$(A$)
PRINT RTRIM$(Z$)
GET #1, 2
PRINT RTRIM$(N$)
PRINT RTRIM$(A$)
PRINT RTRIM$(Z$)
CLOSE #1
"#);
}

#[test]
fn test_compiled_seek_binary() {
    differential_tmpdir(r#"
OPEN "{DIR}/seek.dat" FOR BINARY AS #1
DIM s$
s$ = "ABCDEFGHIJ"
PUT #1, 1, s$
PRINT SEEK(1)
SEEK #1, 1
PRINT SEEK(1)
CLOSE #1
"#);
}

#[test]
fn test_compiled_file_eof_loop() {
    differential_tmpdir(r#"
OPEN "{DIR}/test.txt" FOR OUTPUT AS #1
PRINT #1, "alpha"
PRINT #1, "beta"
PRINT #1, "gamma"
CLOSE #1

OPEN "{DIR}/test.txt" FOR INPUT AS #1
DO WHILE NOT EOF(1)
    LINE INPUT #1, x$
    PRINT x$
LOOP
CLOSE #1
"#);
}
