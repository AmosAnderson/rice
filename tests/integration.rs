use rice::interpreter::SharedOutput;
use std::io::Cursor;

fn run_bas_with_tmpdir(source_template: &str) -> (String, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let dir_path = dir.path().to_str().unwrap().replace('\\', "/");
    let source = source_template.replace("{DIR}", &dir_path);
    let output = run_bas(&source);
    (output, dir)
}

fn run_bas(source: &str) -> String {
    let output = SharedOutput::new();
    let input = Cursor::new(Vec::<u8>::new());
    let mut interp =
        rice::interpreter::Interpreter::with_io(Box::new(output.clone()), Box::new(input));
    interp.run_source(source).unwrap();
    output.into_string()
}

fn run_bas_may_fail(source: &str) -> (String, Result<(), Box<dyn std::error::Error>>) {
    let output = SharedOutput::new();
    let input = Cursor::new(Vec::<u8>::new());
    let mut interp =
        rice::interpreter::Interpreter::with_io(Box::new(output.clone()), Box::new(input));
    let result = interp.run_source(source);
    (output.into_string(), result)
}

fn run_file(path: &str) -> String {
    let source = std::fs::read_to_string(path).unwrap();
    run_bas(&source)
}

#[test]
fn test_hello() {
    assert_eq!(run_file("tests/programs/hello.bas"), "Hello, World!\n");
}

#[test]
fn test_arithmetic() {
    let output = run_file("tests/programs/arithmetic.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "5");
    assert_eq!(lines[1].trim(), "6");
    assert_eq!(lines[2].trim(), "21");
    assert_eq!(lines[3].trim(), "3.75");
    assert_eq!(lines[4].trim(), "3");
    assert_eq!(lines[5].trim(), "2");
    assert_eq!(lines[6].trim(), "1024");
}

#[test]
fn test_variables() {
    let output = run_file("tests/programs/variables.bas");
    assert!(output.contains("30"));
    assert!(output.contains("Hello, Rice"));
}

#[test]
fn test_fizzbuzz() {
    let output = run_file("tests/programs/fizzbuzz.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 30);
    assert_eq!(lines[0].trim(), "1");
    assert_eq!(lines[2], "Fizz");
    assert_eq!(lines[4], "Buzz");
    assert_eq!(lines[14], "FizzBuzz");
}

#[test]
fn test_while_loop() {
    let output = run_file("tests/programs/while_loop.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 5);
}

#[test]
fn test_do_loop() {
    let output = run_file("tests/programs/do_loop.bas");
    assert!(output.contains("while:1"));
    assert!(output.contains("while:3"));
    assert!(output.contains("until:1"));
    assert!(output.contains("until:3"));
}

#[test]
fn test_select_case() {
    let output = run_file("tests/programs/select_case.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "one");
    assert_eq!(lines[1], "two or three");
    assert_eq!(lines[2], "two or three");
    assert_eq!(lines[3], "four or five");
    assert_eq!(lines[4], "four or five");
}

#[test]
fn test_factorial() {
    let output = run_file("tests/programs/factorial.bas");
    assert!(output.contains("3628800"));
}

#[test]
fn test_string_functions() {
    let output = run_file("tests/programs/string_funcs.bas");
    assert!(output.contains("5"));
    assert!(output.contains("Hello"));
    assert!(output.contains("World"));
    assert!(output.contains("A"));
    assert!(output.contains("65"));
    assert!(output.contains("42"));
    assert!(output.contains("3.14"));
    assert!(output.contains("*****"));
}

#[test]
fn test_decimal_place_math_functions() {
    let output = run_bas("PRINT ROUND(3.145, 2)\nPRINT TRUNCATE(3.789, 2)\n");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "3.15");
    assert_eq!(lines[1], "3.78");
}

#[test]
fn test_zero_arg_builtins_reject_extra_args() {
    let (_output, result) = run_bas_may_fail("PRINT PI(1)\n");
    assert!(result.is_err());
}

#[test]
fn test_instr_start_uses_character_positions() {
    let output = run_bas("PRINT INSTR(2, \"éabcabc\", \"abc\")\n");
    assert_eq!(output.trim(), "2");
}

#[test]
fn test_qbasic_string_functions() {
    let output = run_file("tests/programs/qbasic_strings.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "Hello");
    assert_eq!(lines[1], "World");
    assert_eq!(lines[2], "World");
    assert_eq!(lines[3], "Wor");
    assert_eq!(lines[4], "HELLO");
    assert_eq!(lines[5], "hello");
    assert_eq!(lines[6], "FF");
    assert_eq!(lines[7], "10");
    assert_eq!(lines[8], "10");
    assert_eq!(lines[9], "377");
    assert_eq!(lines[10], "Hi");
    assert_eq!(lines[11], "Hi");
    assert_eq!(lines[12], "");
    assert_eq!(lines[13], "0");
    assert_eq!(lines[14], "");
    assert_eq!(lines[15], "0");
    assert_eq!(lines[16], "0");
}

#[test]
fn test_left_negative_errors() {
    let (_output, result) = run_bas_may_fail("PRINT LEFT$(\"test\", -1)\n");
    assert!(result.is_err());
}

#[test]
fn test_mid_zero_start_errors() {
    let (_output, result) = run_bas_may_fail("PRINT MID$(\"test\", 0)\n");
    assert!(result.is_err());
}

#[test]
fn test_right_negative_errors() {
    let (_output, result) = run_bas_may_fail("PRINT RIGHT$(\"test\", -1)\n");
    assert!(result.is_err());
}

#[test]
fn test_mid_negative_length_errors() {
    let (_output, result) = run_bas_may_fail("PRINT MID$(\"test\", 1, -1)\n");
    assert!(result.is_err());
}

#[test]
fn test_mid_wrong_arg_count_errors() {
    let (_output, result) = run_bas_may_fail("PRINT MID$(\"test\")\n");
    assert!(result.is_err());
}

#[test]
fn test_data_read() {
    let output = run_file("tests/programs/data_read.bas");
    assert!(output.contains("10"));
    assert!(output.contains("20"));
    assert!(output.contains("30"));
    assert!(output.contains("Alice"));
    assert!(output.contains("Bob"));
    assert!(output.contains("Carol"));
    assert!(output.contains("Restored:"));
}

#[test]
fn test_sub_call() {
    let output = run_file("tests/programs/sub_test.bas");
    assert!(output.contains("Hello, World!"));
    assert!(output.contains("Hello, BASIC!"));
}

// Expression evaluation tests
#[test]
fn test_operator_precedence() {
    let output = run_bas("PRINT 2 + 3 * 4\n");
    assert!(output.contains("14"));
}

#[test]
fn test_colon_statement_separator() {
    let output = run_bas("PRINT \"A\": PRINT \"B\"\n");
    assert_eq!(output, "A\nB\n");
}

#[test]
fn test_let_array_assignment() {
    let output = run_bas("DIM A(1)\nLET A(1) = 42\nPRINT A(1)\n");
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_string_dollar_keyword_identifier() {
    let output = run_bas("PRINT STRING$(5, \"*\")\nNAME$ = \"rice\"\nPRINT NAME$\n");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "*****");
    assert_eq!(lines[1], "rice");
}

#[test]
fn test_unicode_string_slice_and_instr() {
    let output = run_bas(
        "A$ = \"éx\"\nPRINT A$(1:1)\nA$(1:1) = \"z\"\nPRINT A$\nPRINT INSTR(2, \"éx\", \"x\")\n",
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "é");
    assert_eq!(lines[1], "zx");
    assert_eq!(lines[2], "2");
}

#[test]
fn test_option_base_rejects_invalid_values() {
    let (_output, result) = run_bas_may_fail("OPTION BASE 2\n");
    assert!(result.is_err());
}

#[test]
fn test_goto_rejects_fractional_line_number() {
    let (_output, result) = run_bas_may_fail("GOTO 10.5\n");
    assert!(result.is_err());
}

#[test]
fn test_print_tab_spc_reject_negative_values() {
    let (_output, result) = run_bas_may_fail("PRINT TAB(-1)\n");
    assert!(result.is_err());
    let (_output, result) = run_bas_may_fail("PRINT SPC(-1)\n");
    assert!(result.is_err());
}

#[test]
fn test_string_comparison() {
    let output = run_bas(
        r#"
IF "abc" < "def" THEN
    PRINT "yes"
ELSE
    PRINT "no"
END IF
"#,
    );
    assert_eq!(output.trim(), "yes");
}

#[test]
fn test_nested_loops() {
    let output = run_bas(
        "
FOR i = 1 TO 3
    FOR j = 1 TO 3
        PRINT i * 10 + j;
    NEXT j
    PRINT
NEXT i
",
    );
    assert!(output.contains("11"));
    assert!(output.contains("33"));
}

#[test]
fn test_const() {
    let output = run_bas(
        "
CONST PI = 3.14159
PRINT PI
",
    );
    assert!(output.contains("3.14159"));
}

#[test]
fn test_single_line_if() {
    let output = run_bas("IF 5 > 3 THEN PRINT \"yes\" ELSE PRINT \"no\"\n");
    assert_eq!(output.trim(), "yes");
}

#[test]
fn test_exit_for() {
    let output = run_bas(
        "
FOR i = 1 TO 100
    IF i = 5 THEN EXIT FOR
    PRINT i;
NEXT i
PRINT
",
    );
    assert_eq!(output.trim(), "1234");
}

#[test]
fn test_date_time() {
    let output = run_bas("PRINT DATE$()\nPRINT TIME$()");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    // Date format should be MM-DD-YYYY
    let date = lines[0].trim();
    assert_eq!(date.len(), 10, "DATE$ should be 10 chars: {date}");
    assert_eq!(&date[2..3], "-");
    assert_eq!(&date[5..6], "-");
    // Time format should be HH:MM:SS
    let time = lines[1].trim();
    assert_eq!(time.len(), 8, "TIME$ should be 8 chars: {time}");
    assert_eq!(&time[2..3], ":");
    assert_eq!(&time[5..6], ":");
}

#[test]
fn test_file_text_io() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
OPEN #1: NAME "{DIR}/test.txt", ACCESS OUTPUT
PRINT #1, "Hello, File!"
PRINT #1, "Second line"
CLOSE #1

OPEN #1: NAME "{DIR}/test.txt", ACCESS INPUT
LINE INPUT #1, a
PRINT a
LINE INPUT #1, b
PRINT b
PRINT EOF(1)
CLOSE #1
"#,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "Hello, File!");
    assert_eq!(lines[1], "Second line");
    assert_eq!(lines[2].trim(), "1"); // EOF should be true
}

#[test]
fn test_file_write_read() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
OPEN #1: NAME "{DIR}/test.txt", ACCESS OUTPUT
WRITE #1, "Alice", 30
WRITE #1, "Bob", 25
CLOSE #1

OPEN #1: NAME "{DIR}/test.txt", ACCESS INPUT
INPUT #1, name1, age1
PRINT name1; age1
INPUT #1, name2, age2
PRINT name2; age2
CLOSE #1
"#,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines[0].contains("Alice"));
    assert!(lines[0].contains("30"));
    assert!(lines[1].contains("Bob"));
    assert!(lines[1].contains("25"));
}

#[test]
fn test_file_append() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
OPEN #1: NAME "{DIR}/test.txt", ACCESS OUTPUT
PRINT #1, "Line 1"
CLOSE #1

OPEN #1: NAME "{DIR}/test.txt", ACCESS OUTIN
SET #1: POINTER LOF(1) + 1
PRINT #1, "Line 2"
CLOSE #1

OPEN #1: NAME "{DIR}/test.txt", ACCESS INPUT
LINE INPUT #1, a
PRINT a
LINE INPUT #1, b
PRINT b
CLOSE #1
"#,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "Line 1");
    assert_eq!(lines[1], "Line 2");
}

#[test]
fn test_file_binary() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
msg = "HELLO"
OPEN #1: NAME "{DIR}/test.bin", ORGANIZATION STREAM, ACCESS OUTIN
PUT #1, 1, msg
CLOSE #1

OPEN #1: NAME "{DIR}/test.bin", ORGANIZATION STREAM, ACCESS OUTIN
GET #1, 1, result
PRINT result
CLOSE #1
"#,
    );
    assert_eq!(output.trim(), "HELLO");
}

#[test]
fn test_file_freefile() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
PRINT FREEFILE
OPEN #1: NAME "{DIR}/a.tmp", ACCESS OUTPUT
PRINT FREEFILE
OPEN #2: NAME "{DIR}/b.tmp", ACCESS OUTPUT
PRINT FREEFILE
CLOSE #1
PRINT FREEFILE
CLOSE #2
"#,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "1");
    assert_eq!(lines[1].trim(), "2");
    assert_eq!(lines[2].trim(), "3");
    assert_eq!(lines[3].trim(), "1"); // #1 freed, so FREEFILE returns 1
}

#[test]
fn test_file_lof() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
OPEN #1: NAME "{DIR}/test.txt", ACCESS OUTPUT
PRINT #1, "Hello"
CLOSE #1

OPEN #1: NAME "{DIR}/test.txt", ACCESS INPUT
PRINT LOF(1)
CLOSE #1
"#,
    );
    let lof: i64 = output.trim().parse().unwrap();
    assert!(lof > 0);
}

#[test]
fn test_file_eof_loop() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
OPEN #1: NAME "{DIR}/test.txt", ACCESS OUTPUT
PRINT #1, "alpha"
PRINT #1, "beta"
PRINT #1, "gamma"
CLOSE #1

OPEN #1: NAME "{DIR}/test.txt", ACCESS INPUT
DO WHILE NOT EOF(1)
    LINE INPUT #1, x
    PRINT x
LOOP
CLOSE #1
"#,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "alpha");
    assert_eq!(lines[1], "beta");
    assert_eq!(lines[2], "gamma");
    assert_eq!(lines.len(), 3);
}

// ==================== PRINT USING tests ====================

#[test]
fn test_print_using_digits() {
    // Note: r####""## raw strings needed because Rust 2024 reserves ## in string literals
    let output = run_bas(
        r####"
PRINT USING "###.##"; 1.5
PRINT USING "###.##"; 123.456
PRINT USING "###.##"; -1.5
"####,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "  1.50");
    assert_eq!(lines[1], "123.46");
    // Negative: sign replaces a space
    assert_eq!(lines[2], " -1.50");
}

#[test]
fn test_print_using_dollar() {
    let output = run_bas(
        r####"
PRINT USING "$$###.##"; 1.5
PRINT USING "$$###.##"; 123.45
"####,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "  $1.50");
    assert_eq!(lines[1], "$123.45");
}

#[test]
fn test_print_using_sign() {
    let output = run_bas(
        r####"
PRINT USING "+###"; 5
PRINT USING "+###"; -5
PRINT USING "###-"; 5
PRINT USING "###-"; -5
"####,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "+  5");
    assert_eq!(lines[1], "-  5");
    assert_eq!(lines[2], "  5 ");
    assert_eq!(lines[3], "  5-");
}

#[test]
fn test_print_using_asterisk() {
    let output = run_bas(
        r####"
PRINT USING "**###.##"; 1.5
PRINT USING "**$###.##"; 1.5
"####,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "****1.50");
    assert_eq!(lines[1], "***$1.50");
}

#[test]
fn test_print_using_comma() {
    let output = run_bas(
        r####"
PRINT USING "#,###.##"; 1234.56
"####,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "1,234.56");
}

#[test]
fn test_print_using_scientific() {
    let output = run_bas(
        r####"
PRINT USING "##.##^^^^"; 1234.5
"####,
    );
    let lines: Vec<&str> = output.lines().collect();
    // digits_before=2, so: 12.35E+02
    assert_eq!(lines[0], "12.35E+02");
}

#[test]
fn test_print_using_string() {
    let output = run_bas(
        r####"
PRINT USING "!"; "Hello"
PRINT USING "\   \"; "Hello"
PRINT USING "&"; "Hello"
"####,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "H");
    assert_eq!(lines[1], "Hello");
    assert_eq!(lines[2], "Hello");
}

#[test]
fn test_print_using_escape() {
    let output = run_bas(
        r####"
PRINT USING "_###.##"; 1.5
"####,
    );
    let lines: Vec<&str> = output.lines().collect();
    // _ escapes #, so first # is literal, then ##.## is a 2-digit format
    assert_eq!(lines[0], "# 1.50");
}

#[test]
fn test_print_using_overflow() {
    let output = run_bas(
        r####"
PRINT USING "##.##"; 12345.67
"####,
    );
    let lines: Vec<&str> = output.lines().collect();
    // Number too wide for field — % prefix
    assert!(
        lines[0].starts_with('%'),
        "expected overflow prefix %, got: {}",
        lines[0]
    );
}

#[test]
fn test_print_using_repeat() {
    let output = run_bas(
        r####"
PRINT USING "###"; 1; 2; 3
"####,
    );
    let lines: Vec<&str> = output.lines().collect();
    // Format repeats for each value
    assert_eq!(lines[0], "  1  2  3");
}

#[test]
fn test_print_using_file() {
    let (output, _dir) = run_bas_with_tmpdir(
        r####"
OPEN #1: NAME "{DIR}/test.txt", ACCESS OUTPUT
PRINT #1, USING "###.##"; 3.14
CLOSE #1

OPEN #1: NAME "{DIR}/test.txt", ACCESS INPUT
LINE INPUT #1, x
PRINT x
CLOSE #1
"####,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "  3.14");
}

#[test]
fn test_randomize() {
    let output = run_file("tests/programs/randomize.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "deterministic");
    assert_eq!(lines[1], "rnd0 ok");
    assert_eq!(lines[2], "range ok");
}

// ==================== Phase 1-4 new feature tests ====================

#[test]
fn test_write_stmt() {
    let output = run_file("tests/programs/write_stmt.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "1,2,3");
    assert_eq!(lines[1], "\"hello\",42,\"world\"");
    assert_eq!(lines[2], "");
}

#[test]
fn test_clear() {
    let output = run_file("tests/programs/clear_test.bas");
    let lines: Vec<&str> = output.lines().collect();
    // After CLEAR, variables should auto-init to 0
    assert_eq!(lines[0].trim(), "0");
    assert_eq!(lines[1].trim(), "0");
}

#[test]
fn test_mid_assign() {
    let output = run_file("tests/programs/mid_assign.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "Hello BASIC");
    assert_eq!(lines[1], "12ABCD7890");
    assert_eq!(lines[2], "HiXXXXXXX");
}

#[test]
fn test_shared() {
    let output = run_file("tests/programs/shared_test.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "20");
}

#[test]
fn test_static_var() {
    let output = run_file("tests/programs/static_test.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "1");
    assert_eq!(lines[1].trim(), "2");
    assert_eq!(lines[2].trim(), "3");
}

#[test]
fn test_static_sub() {
    let output = run_file("tests/programs/static_sub.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "5");
    assert_eq!(lines[1].trim(), "10");
    assert_eq!(lines[2].trim(), "15");
}

#[test]
fn test_environ() {
    // Test ENVIRON function returns a non-empty value for a known env var
    let output = run_bas(
        r#"
        x = ENVIRON("PATH")
        IF LEN(x) > 0 THEN
            PRINT "has path"
        ELSE
            PRINT "no path"
        END IF
        y = ENVIRON("NONEXISTENT_VAR_12345")
        PRINT LEN(y)
    "#,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "has path");
    assert_eq!(lines[1].trim(), "0");
}

#[test]
fn test_file_ops() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
        MKDIR "{DIR}/testsubdir"
        OPEN #1: NAME "{DIR}/testsubdir/test.txt", ACCESS OUTPUT
        PRINT #1, "hello"
        CLOSE #1
        NAME "{DIR}/testsubdir/test.txt" AS "{DIR}/testsubdir/renamed.txt"
        OPEN #1: NAME "{DIR}/testsubdir/renamed.txt", ACCESS INPUT
        LINE INPUT #1, x
        CLOSE #1
        PRINT x
        KILL "{DIR}/testsubdir/renamed.txt"
        RMDIR "{DIR}/testsubdir"
        PRINT "done"
    "#,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "hello");
    assert_eq!(lines[1], "done");
}

#[test]
fn test_sleep() {
    // Just make sure SLEEP 0 parses and runs without error
    let output = run_bas("SLEEP 0\nPRINT \"ok\"");
    assert_eq!(output.trim(), "ok");
}

#[test]
fn test_type_basic() {
    let output = run_file("tests/programs/type_basic.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "John");
    assert_eq!(lines[1].trim(), "Doe");
    assert_eq!(lines[2].trim(), "30");
}

#[test]
fn test_type_array() {
    let output = run_file("tests/programs/type_array.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "1020");
    assert_eq!(lines[1].trim(), "3040");
    assert_eq!(lines[2].trim(), "5060");
}

#[test]
fn test_type_sub() {
    let output = run_file("tests/programs/type_sub.bas");
    assert_eq!(output.trim(), "16.5");
}

// ── Phase 1+2: Console features ──────────────────────────────────────

#[test]
fn test_cls() {
    let output = run_bas("CLS\n");
    assert!(
        output.contains("\x1b[2J\x1b[H"),
        "CLS should emit ANSI clear + home"
    );
}

#[test]
fn test_beep() {
    let output = run_bas("BEEP\n");
    assert!(output.contains("\x07"), "BEEP should emit BEL character");
}

#[test]
fn test_locate() {
    let output = run_bas("LOCATE 5, 10\nPRINT \"X\"\n");
    assert!(
        output.contains("\x1b[5;10H"),
        "LOCATE should emit ANSI cursor move"
    );
    assert!(output.contains("X"));
}

#[test]
fn test_locate_row_only() {
    let output = run_bas("LOCATE 3\nPRINT \"Y\"\n");
    assert!(
        output.contains("\x1b[3;1H"),
        "LOCATE with row only should keep col=1"
    );
}

#[test]
fn test_color() {
    let output = run_bas("COLOR 4, 1\nPRINT \"red on blue\"\n");
    // Color 4 = red -> ANSI 31, color 1 = blue -> ANSI 44
    assert!(
        output.contains("\x1b[31;44m"),
        "COLOR 4,1 should emit combined ANSI 31;44"
    );
}

#[test]
fn test_color_fg_only() {
    let output = run_bas("COLOR 2\nPRINT \"green\"\n");
    // Color 2 = green -> ANSI 32
    assert!(output.contains("\x1b[32m"));
}

#[test]
fn test_color_error_out_of_range() {
    let (_output, result) = run_bas_may_fail("COLOR 16\n");
    assert!(result.is_err(), "COLOR 16 should error (out of range 0-15)");
}

#[test]
fn test_locate_error_row_zero() {
    let (_output, result) = run_bas_may_fail("LOCATE 0, 1\n");
    assert!(result.is_err(), "LOCATE 0 should error (rows are 1-based)");
}

#[test]
fn test_csrlin() {
    let output = run_bas("LOCATE 7, 1\nPRINT CSRLIN\n");
    assert!(
        output.contains("7"),
        "CSRLIN should return 7 after LOCATE 7"
    );
}

#[test]
fn test_pos() {
    let output = run_bas("LOCATE 1, 12\nPRINT POS(0)\n");
    assert!(
        output.contains("12"),
        "POS(0) should return 12 after LOCATE ,12"
    );
}

#[test]
fn test_width() {
    let output = run_bas("WIDTH 40\nPRINT \"ok\"\n");
    assert!(output.contains("ok"), "WIDTH should not crash");
}

#[test]
fn test_view_print() {
    let output = run_bas("VIEW PRINT 5 TO 20\n");
    assert!(
        output.contains("\x1b[5;20r"),
        "VIEW PRINT should emit ANSI scroll region"
    );
}

#[test]
fn test_view_print_reset() {
    let output = run_bas("VIEW PRINT\n");
    // Reset emits ANSI scroll region reset (no args)
    assert!(
        output.contains("\x1b[r"),
        "VIEW PRINT (no args) should reset scroll region"
    );
}

// ── Phase 3: INKEY$ and INPUT$ ───────────────────────────────────────

#[test]
fn test_inkey_returns_empty_in_test_mode() {
    let output = run_bas("PRINT INKEY$()\n");
    // In non-interactive mode, INKEY$ returns ""
    assert_eq!(output.trim(), "");
}

#[test]
fn test_inkey_in_expression() {
    let output = run_bas(
        r#"
DIM k AS STRING
k = INKEY$()
IF k = "" THEN PRINT "empty" ELSE PRINT "key"
"#,
    );
    assert_eq!(output.trim(), "empty");
}

// INPUT$ function tests removed — INPUT$ is not part of ANSI BASIC.
// Use LINE INPUT #n for reading strings from files.

// ── Phase 4: SCREEN() function ───────────────────────────────────────

#[test]
fn test_screen_function() {
    let output = run_bas(
        r#"
LOCATE 1, 1
PRINT "A";
PRINT SCREEN(1, 1)
"#,
    );
    // SCREEN(1,1) should return ASCII code of 'A' = 65
    assert!(
        output.contains("65"),
        "SCREEN(1,1) should return 65 for 'A'"
    );
}

#[test]
fn test_screen_function_empty() {
    let output = run_bas(
        r#"
CLS
PRINT SCREEN(5, 5)
"#,
    );
    // Empty screen position should return 32 (space)
    assert!(
        output.contains("32"),
        "Empty position should return 32 (space)"
    );
}

#[test]
fn test_screen_function_after_print() {
    let output = run_bas(
        r#"
CLS
LOCATE 2, 3
PRINT "XY";
PRINT SCREEN(2, 3); SCREEN(2, 4)
"#,
    );
    // X=88, Y=89
    assert!(
        output.contains("88"),
        "SCREEN(2,3) should return 88 for 'X'"
    );
    assert!(
        output.contains("89"),
        "SCREEN(2,4) should return 89 for 'Y'"
    );
}

// ==================== SET/ASK POINTER ====================

#[test]
fn test_set_ask_pointer() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
OPEN #1: NAME "{DIR}/seek.dat", ORGANIZATION STREAM, ACCESS OUTIN
DIM s AS STRING
s = "ABCDEFGHIJ"
PUT #1, 1, s
ASK #1: POINTER p
PRINT p
SET #1: POINTER 1
ASK #1: POINTER p
PRINT p
CLOSE #1
"#,
    );
    // After PUT of 10 bytes, position should be 11 (1-based)
    // After SET POINTER to 1, position should be 1
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "11");
    assert_eq!(lines[1].trim(), "1");
}

// ==================== BYVAL ====================

#[test]
fn test_byref_sub() {
    // ANSI BASIC: parameters are BYVAL by default, so x is NOT modified
    let output = run_bas(
        r#"
DIM x AS NUMERIC
x = 10
CALL AddFive(x)
PRINT x

SUB AddFive(n AS NUMERIC)
    n = n + 5
END SUB
"#,
    );
    assert_eq!(output, "10\n");
}

#[test]
fn test_byval_sub() {
    let output = run_bas(
        r#"
DIM x AS NUMERIC
x = 10
CALL AddFive(x)
PRINT x

SUB AddFive(BYVAL n AS NUMERIC)
    n = n + 5
END SUB
"#,
    );
    assert_eq!(output, "10\n");
}

#[test]
fn test_byval_paren_forces_byval() {
    // ANSI BASIC: BYVAL by default, parenthesized arg also forces BYVAL
    let output = run_bas(
        r#"
DIM x AS NUMERIC
x = 10
CALL AddFive((x))
PRINT x

SUB AddFive(n AS NUMERIC)
    n = n + 5
END SUB
"#,
    );
    assert_eq!(output, "10\n");
}

#[test]
fn test_byval_expression_arg() {
    let output = run_bas(
        r#"
DIM x AS NUMERIC
x = 10
CALL AddFive(x + 0)
PRINT x

SUB AddFive(n AS NUMERIC)
    n = n + 5
END SUB
"#,
    );
    assert_eq!(output, "10\n");
}

#[test]
fn test_byref_function() {
    // ANSI BASIC: BYVAL by default, so x is NOT modified by the function
    let output = run_bas(
        r#"
DIM x AS NUMERIC
x = 10
DIM r AS NUMERIC
r = Dbl(x)
PRINT x
PRINT r

FUNCTION Dbl(n AS NUMERIC)
    n = n * 2
    Dbl = n
END FUNCTION
"#,
    );
    assert_eq!(output, "10\n20\n");
}

#[test]
fn test_byval_function() {
    let output = run_bas(
        r#"
DIM x AS NUMERIC
x = 10
DIM r AS NUMERIC
r = Dbl(x)
PRINT x
PRINT r

FUNCTION Dbl(BYVAL n AS NUMERIC)
    n = n * 2
    Dbl = n
END FUNCTION
"#,
    );
    assert_eq!(output, "10\n20\n");
}

// ── MAT (matrix) operations ──────────────────────────────────────────

#[test]
fn test_mat_zer_con_idn() {
    let output = run_bas(
        "DIM A(1 TO 2, 1 TO 2)\nMAT A = ZER\nMAT PRINT A\nMAT A = CON\nMAT PRINT A\nMAT A = IDN\nMAT PRINT A\n",
    );
    assert_eq!(output, "0 0\n0 0\n1 1\n1 1\n1 0\n0 1\n");
}

#[test]
fn test_mat_explicit_lower_bounds() {
    let output = run_bas("DIM A(0 TO 1, 0 TO 1)\nMAT A = CON\nPRINT A(0,0)\nPRINT A(1,1)\n");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "1");
    assert_eq!(lines[1], "1");
}

#[test]
fn test_redim_updates_mat_dimensions() {
    let output =
        run_bas("DIM A(1 TO 2, 1 TO 2)\nREDIM A(1 TO 3, 1 TO 3)\nMAT A = CON\nMAT PRINT A\n");
    assert_eq!(output.lines().count(), 3);
}

#[test]
fn test_mat_add_sub() {
    let output = run_bas(
        "DIM A(1 TO 2, 1 TO 2)\nDIM B(1 TO 2, 1 TO 2)\nDIM C(1 TO 2, 1 TO 2)\nA(1,1) = 1\nA(1,2) = 2\nA(2,1) = 3\nA(2,2) = 4\nB(1,1) = 5\nB(1,2) = 6\nB(2,1) = 7\nB(2,2) = 8\nMAT C = A + B\nMAT PRINT C\nMAT C = A - B\nMAT PRINT C\n",
    );
    assert_eq!(output, "6 8\n10 12\n-4 -4\n-4 -4\n");
}

#[test]
fn test_mat_mul() {
    let output = run_bas(
        "DIM A(1 TO 2, 1 TO 2)\nDIM B(1 TO 2, 1 TO 2)\nDIM C(1 TO 2, 1 TO 2)\nA(1,1) = 1\nA(1,2) = 2\nA(2,1) = 3\nA(2,2) = 4\nB(1,1) = 5\nB(1,2) = 6\nB(2,1) = 7\nB(2,2) = 8\nMAT C = A * B\nMAT PRINT C\n",
    );
    assert_eq!(output, "19 22\n43 50\n");
}

#[test]
fn test_mat_scalar_mul() {
    let output = run_bas(
        "DIM A(1 TO 2, 1 TO 2)\nDIM C(1 TO 2, 1 TO 2)\nA(1,1) = 1\nA(1,2) = 2\nA(2,1) = 3\nA(2,2) = 4\nMAT C = (3) * A\nMAT PRINT C\n",
    );
    assert_eq!(output, "3 6\n9 12\n");
}

#[test]
fn test_mat_trn() {
    let output = run_bas(
        "DIM A(1 TO 2, 1 TO 3)\nDIM B(1 TO 3, 1 TO 2)\nA(1,1) = 1\nA(1,2) = 2\nA(1,3) = 3\nA(2,1) = 4\nA(2,2) = 5\nA(2,3) = 6\nMAT B = TRN(A)\nMAT PRINT B\n",
    );
    assert_eq!(output, "1 4\n2 5\n3 6\n");
}

#[test]
fn test_mat_inv_det() {
    let output = run_bas(
        "DIM A(1 TO 2, 1 TO 2)\nDIM B(1 TO 2, 1 TO 2)\nA(1,1) = 4\nA(1,2) = 7\nA(2,1) = 2\nA(2,2) = 6\nMAT B = INV(A)\nPRINT DET\n",
    );
    // det(A) = 4*6 - 7*2 = 10
    assert_eq!(output.trim(), "10");
}

#[test]
fn test_mat_read() {
    let output = run_bas("DIM A(1 TO 2, 1 TO 2)\nDATA 10, 20, 30, 40\nMAT READ A\nMAT PRINT A\n");
    assert_eq!(output, "10 20\n30 40\n");
}

#[test]
fn test_mat_copy() {
    let output = run_bas(
        "DIM A(1 TO 2, 1 TO 2)\nDIM B(1 TO 2, 1 TO 2)\nA(1,1) = 1\nA(1,2) = 2\nA(2,1) = 3\nA(2,2) = 4\nMAT B = A\nMAT PRINT B\n",
    );
    assert_eq!(output, "1 2\n3 4\n");
}

#[test]
fn test_qb_compat() {
    let output = run_file("tests/programs/qb_compat.bas");
    let lines: Vec<&str> = output.lines().map(|l| l.trim()).collect();
    assert_eq!(lines[0], "10");
    assert_eq!(lines[1], "200000");
    // Float values formatting: print strips trailing zeros or handles formats.
    // Let's assert shape/start of strings if they might have floating representation variations
    assert!(lines[2].starts_with("3.14"));
    assert!(lines[3].starts_with("2.71828182"));
    assert_eq!(lines[4], "hello");
    
    // Comparison
    assert_eq!(lines[5], "-1");
    assert_eq!(lines[6], "-1");
    assert_eq!(lines[7], "0");
    
    // Bitwise
    assert_eq!(lines[8], "1");
    assert_eq!(lines[9], "7");
    assert_eq!(lines[10], "0");
    assert_eq!(lines[11], "6");
    
    // String concat +
    assert_eq!(lines[12], "hello world");
    
    // Hex/Octal
    assert_eq!(lines[13], "255");
    assert_eq!(lines[14], "63");
    
    // GOSUB
    assert_eq!(lines[15], "inside GOSUB");
    assert_eq!(lines[16], "after GOSUB");
    assert_eq!(lines[17], "done GOSUB");
    
    // ON GOTO
    assert_eq!(lines[18], "400");
    assert_eq!(lines[19], "done ON GOTO");
    
    // Parameter passing: BYREF vs BYVAL
    assert_eq!(lines[20], "42");
    assert_eq!(lines[21], "10");
}
