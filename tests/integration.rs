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
fn test_default_dialect_is_qbasic() {
    let output = run_bas(
        r#"
PRINT 5 > 3
PRINT "hello " + "world"
GOSUB 100
END
100 PRINT "gosub"
RETURN
"#,
    );
    assert_eq!(output, "-1\nhello world\ngosub\n");
}

#[test]
fn test_option_dialect_ansi_overrides_default() {
    let output = run_bas(
        r#"
OPTION DIALECT "ANSI"
PRINT 5 > 3
"#,
    );
    assert_eq!(output, "1\n");
}

#[test]
fn test_option_dialect_qbasic_11_alias() {
    let output = run_bas(
        r#"
OPTION DIALECT "QBasic 1.1"
PRINT 5 > 3
"#,
    );
    assert_eq!(output, "-1\n");
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
LINE INPUT #1, a$
PRINT a$
LINE INPUT #1, b$
PRINT b$
PRINT EOF(1)
CLOSE #1
"#,
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "Hello, File!");
    assert_eq!(lines[1], "Second line");
    assert_eq!(lines[2].trim(), "-1"); // EOF should be true in QBasic mode
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
INPUT #1, name1$, age1
PRINT name1$; age1
INPUT #1, name2$, age2
PRINT name2$; age2
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
LINE INPUT #1, a$
PRINT a$
LINE INPUT #1, b$
PRINT b$
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
msg$ = "HELLO"
OPEN #1: NAME "{DIR}/test.bin", ORGANIZATION STREAM, ACCESS OUTIN
PUT #1, 1, msg$
CLOSE #1

OPEN #1: NAME "{DIR}/test.bin", ORGANIZATION STREAM, ACCESS OUTIN
GET #1, 1, result$
PRINT result$
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
    LINE INPUT #1, x$
    PRINT x$
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
LINE INPUT #1, x$
PRINT x$
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
        LINE INPUT #1, x$
        CLOSE #1
        PRINT x$
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
s$ = "ABCDEFGHIJ"
PUT #1, 1, s$
ASK #1: POINTER p
PRINT p
SET #1: POINTER 1
ASK #1: POINTER p
PRINT p
CLOSE #1
"#,
    );
    // QBasic variable-length string records include a 2-byte length prefix.
    // After PUT of 12 bytes, position should be 13 (1-based).
    // After SET POINTER to 1, position should be 1
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "13");
    assert_eq!(lines[1].trim(), "1");
}

// ==================== BYVAL ====================

#[test]
fn test_byref_sub() {
    // QBasic mode is the default, so parameters are BYREF unless BYVAL is explicit.
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
    assert_eq!(output, "15\n");
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
    // In QBasic mode, a parenthesized call argument forces BYVAL.
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
    // QBasic mode is the default, so function parameters are BYREF unless BYVAL is explicit.
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
    assert_eq!(output, "20\n20\n");
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

// ==================== Tier 1 QBasic features ====================

#[test]
fn test_while_wend() {
    let output = run_bas("x = 0\nWHILE x < 3\nPRINT x\nx = x + 1\nWEND\n");
    assert_eq!(output, "0\n1\n2\n");
}

#[test]
fn test_lbound_ubound() {
    let output = run_bas("DIM A(5 TO 12)\nPRINT LBOUND(A)\nPRINT UBOUND(A)\n");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "5");
    assert_eq!(lines[1], "12");
}

#[test]
fn test_lbound_ubound_dim() {
    let output = run_bas("DIM A(1 TO 3, 4 TO 9)\nPRINT LBOUND(A, 2)\nPRINT UBOUND(A, 2)\n");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "4");
    assert_eq!(lines[1], "9");
}

#[test]
fn test_numeric_conversions() {
    let output = run_bas("PRINT CINT(2.5)\nPRINT CINT(3.5)\nPRINT CLNG(-2.5)\nPRINT CDBL(3)\n");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "2"); // round half to even
    assert_eq!(lines[1], "4"); // round half to even
    assert_eq!(lines[2], "-2");
    assert_eq!(lines[3], "3");
}

#[test]
fn test_val_hex_octal() {
    let output = run_bas("PRINT VAL(\"&HFF\")\nPRINT VAL(\"&O77\")\nPRINT VAL(\"42\")\n");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "255");
    assert_eq!(lines[1], "63");
    assert_eq!(lines[2], "42");
}

#[test]
fn test_seek_function_and_statement() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
OPEN #1: NAME "{DIR}/seek.dat", ORGANIZATION STREAM, ACCESS OUTIN
s$ = "ABCDEFGHIJ"
PUT #1, 1, s$
SEEK #1, 3
PRINT SEEK(1)
RESET
"#,
    );
    assert_eq!(output.trim(), "3");
}

// ==================== Tier 2 QBasic features ====================

#[test]
fn test_on_error_resume_next_err_erl() {
    let output = run_bas(
        r#"
10 ON ERROR GOTO 100
20 PRINT 1 / 0
30 PRINT "after"
40 END
100 PRINT ERR
110 PRINT ERL
120 RESUME NEXT
"#,
    );
    assert_eq!(output, "11\n20\nafter\n");
}

#[test]
fn test_error_statement_resume_label() {
    let output = run_bas(
        r#"
10 ON ERROR GOTO 100
20 ERROR 53
30 END
100 PRINT ERR
110 RESUME 200
200 PRINT "resumed"
"#,
    );
    assert_eq!(output, "53\nresumed\n");
}

#[test]
fn test_mk_cv_packed_conversions() {
    let output = run_bas(
        r#"
PRINT LEN(MKI$(258))
PRINT CVI(MKI$(-2))
PRINT CVL(MKL$(123456))
PRINT INT(CVS(MKS$(1.5)) * 10)
PRINT CVD(MKD$(2.25))
"#,
    );
    assert_eq!(output, "2\n-2\n123456\n15\n2.25\n");
}

#[test]
fn test_field_lset_rset_random_records() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
OPEN "{DIR}/records.dat" FOR RANDOM AS #1 LEN = 12
FIELD #1, 5 AS A$, 3 AS B$
LSET A$ = "ALPHA!"
RSET B$ = "7"
PUT #1, 1
LSET A$ = "BETA"
RSET B$ = "42"
PUT #1, 2
LSET A$ = ""
LSET B$ = ""
GET #1, 1
PRINT A$ + "|" + B$
GET #1, 2
PRINT A$ + "|" + B$
CLOSE #1
"#,
    );
    assert_eq!(output, "ALPHA|  7\nBETA | 42\n");
}

#[test]
fn test_ansi_rejects_qbasic_only_tier2_syntax() {
    let cases = [
        "OPTION DIALECT \"ANSI\"\nDEFSTR A-Z\n",
        "OPTION DIALECT \"ANSI\"\nDEF FNX(X) = X\nPRINT FNX(1)\n",
        "OPTION DIALECT \"ANSI\"\nA$ = \"ABC\"\nMID$(A$, 1, 1) = \"Z\"\n",
    ];
    for source in cases {
        let (_output, result) = run_bas_may_fail(source);
        assert!(result.is_err(), "source should fail in ANSI mode: {source}");
    }
}

#[test]
fn test_ansi_rejects_qbasic_only_business_features() {
    let cases = [
        "OPTION DIALECT \"ANSI\"\nOPTION EXPLICIT\n",
        "OPTION DIALECT \"ANSI\"\nENVIRON \"X=1\"\n",
        "OPTION DIALECT \"ANSI\"\nDATE$ = \"12-25-2024\"\n",
        "OPTION DIALECT \"ANSI\"\nTIME$ = \"14:30:00\"\n",
        "OPTION DIALECT \"ANSI\"\nCOMMON SHARED x\n",
        "OPTION DIALECT \"ANSI\"\nCOMMON x\n",
    ];
    for source in cases {
        let (_output, result) = run_bas_may_fail(source);
        assert!(result.is_err(), "source should fail in ANSI mode: {source}");
    }
}

#[test]
fn test_seek_function_prefers_writer_position() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
OPTION DIALECT "ANSI"
OPEN #1: NAME "{DIR}/seek-outin.dat", ORGANIZATION STREAM, ACCESS OUTIN
s$ = "ABC"
PUT #1, 1, s$
PRINT SEEK(1)
RESET
"#,
    );
    assert_eq!(output, "4\n");
}

#[test]
fn test_on_error_open_uses_io_err_code() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
10 ON ERROR GOTO 100
20 OPEN "{DIR}/missing.txt" FOR INPUT AS #1
30 END
100 PRINT ERR
110 RESUME 200
200 PRINT "handled"
"#,
    );
    assert_eq!(output, "53\nhandled\n");
}

#[test]
fn test_binary_string_serialization_preserves_packed_bytes() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
s$ = MKI$(32767)
OPEN "{DIR}/packed.dat" FOR BINARY AS #1
PUT #1, 1, s$
CLOSE #1
OPEN "{DIR}/packed.dat" FOR BINARY AS #1
PRINT LOF(1)
GET #1, 1, t$
PRINT LEN(t$)
PRINT CVI(t$)
CLOSE #1
"#,
    );
    assert_eq!(output, "4\n2\n32767\n");
}

#[test]
fn test_option_explicit() {
    let output = run_file("tests/programs/option_explicit.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "5");
    assert_eq!(lines[1], "10");
    assert_eq!(lines[2], "10");
    assert_eq!(lines[3], "hello");
    assert_eq!(lines[4], "6");
}

#[test]
fn test_option_explicit_undeclared() {
    let (_output, result) = run_bas_may_fail(
        r#"
OPTION EXPLICIT
x = 5
PRINT x
"#,
    );
    assert!(result.is_err(), "undeclared variable should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Variable 'X' is not declared"),
        "error should name the variable: {}",
        err
    );
}

#[test]
fn test_option_explicit_allows_pseudo_values_and_bare_zero_arg_functions() {
    let output = run_bas(
        r#"
OPTION EXPLICIT
PRINT ERR
PRINT ERL

FUNCTION FortyTwo
    FortyTwo = 42
END FUNCTION

PRINT FortyTwo
"#,
    );
    assert_eq!(output, "0\n0\n42\n");
}

#[test]
fn test_option_explicit_rejects_undeclared_write_only_targets() {
    let cases = [
        "OPTION EXPLICIT\nASK #1: POINTER p\n",
        "OPTION EXPLICIT\nLINE INPUT #1, p\n",
        "OPTION EXPLICIT\nGET #1, , p\n",
    ];
    for source in cases {
        let (_output, result) = run_bas_may_fail(source);
        assert!(result.is_err(), "source should fail: {source}");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Variable 'P' is not declared"),
            "error should name the undeclared target: {}",
            err
        );
    }
}

#[test]
fn test_option_explicit_dim_shared_in_sub() {
    // A module-level DIM SHARED variable must be usable inside a procedure
    // without a per-procedure SHARED statement, even under OPTION EXPLICIT.
    let output = run_bas(
        r#"
OPTION EXPLICIT
DIM SHARED x AS INTEGER
x = 5
CALL Foo
PRINT x

SUB Foo
    x = x + 10
END SUB
"#,
    );
    assert_eq!(output, "15\n");
}

#[test]
fn test_option_explicit_common_shared_in_sub() {
    let output = run_bas(
        r#"
OPTION EXPLICIT
COMMON SHARED total
DIM total AS INTEGER
total = 1
CALL Bump
PRINT total

SUB Bump
    total = total + 41
END SUB
"#,
    );
    assert_eq!(output, "42\n");
}

#[test]
fn test_environ_statement() {
    // Use a unique name to avoid pre-existing environment interference.
    let key = "RICE_ENVIRON_TEST_VAR";
    let output = run_bas(&format!(
        r#"
ENVIRON "{}=hello world"
PRINT ENVIRON$("{}")
ENVIRON "{}=goodbye"
PRINT ENVIRON$("{}")
"#,
        key, key, key, key
    ));
    assert_eq!(output, "hello world\ngoodbye\n");
}

#[test]
fn test_environ_statement_invalid() {
    let (_output, result) = run_bas_may_fail(r#"ENVIRON "no equals sign""#);
    assert!(result.is_err(), "ENVIRON without = should fail");
}

#[test]
fn test_date_time_assign() {
    let output = run_bas(
        r#"
DATE$ = "12-25-2024"
TIME$ = "14:30:15"
PRINT DATE$
PRINT TIME$
PRINT DATE$()
PRINT TIME$()
DATE$ = "01/01/2000"
PRINT DATE$
"#,
    );
    assert_eq!(
        output,
        "12-25-2024\n14:30:15\n12-25-2024\n14:30:15\n01/01/2000\n"
    );
}

#[test]
fn test_date_time_assign_invalid() {
    let (_output, result) = run_bas_may_fail(r#"DATE$ = "not-a-date""#);
    assert!(result.is_err(), "invalid DATE$ should fail");
    let (_output, result) = run_bas_may_fail(r#"TIME$ = "25:00:00""#);
    assert!(result.is_err(), "invalid TIME$ should fail");
}

#[test]
fn test_common_shared() {
    let output = run_bas(
        r#"
COMMON SHARED x, y
DIM x AS INTEGER
DIM y AS INTEGER
x = 10
y = 20
CALL Modify
PRINT x
PRINT y

SUB Modify
    x = x + 20
    y = y + 20
END SUB
"#,
    );
    assert_eq!(output, "30\n40\n");
}

#[test]
fn test_common_without_shared() {
    let output = run_bas(
        r#"
COMMON z
DIM z AS INTEGER
z = 5
CALL IncZ
PRINT z

SUB IncZ
    z = z + 10
END SUB
"#,
    );
    assert_eq!(output, "15\n");
}

#[test]
fn test_unary_plus_passes_argument_by_value() {
    assert_eq!(run_file("tests/programs/unary_plus_byval.bas"), "7\n");
    assert!(run_bas_may_fail("PRINT +\"text\"\n").1.is_err());
}

#[test]
fn test_static_record_initialization() {
    assert_eq!(run_file("tests/programs/static_record.bas"), "1\n2\n");
}

#[test]
fn test_declared_string_defaults() {
    assert_eq!(run_file("tests/programs/string_defaults.bas"), "0\n0\n");
}

#[test]
fn test_print_column_positions() {
    assert_eq!(
        run_file("tests/programs/print_columns.bas"),
        format!("A{}B\n    C\n", " ".repeat(15))
    );
}

#[test]
fn test_input_eof_returns_error() {
    for source in ["INPUT a, b\n", "INPUT a\n", "LINE INPUT a$\n"] {
        let (_, result) = run_bas_may_fail(source);
        let err = result.unwrap_err();
        let runtime = err.downcast_ref::<rice::error::RuntimeError>().unwrap();
        assert_eq!(runtime.basic_err_code(), 62, "{source}");
    }
}

#[test]
fn test_input_retries_invalid_numbers_and_honors_string_declarations() {
    let output = SharedOutput::new();
    let input = Cursor::new(b"bad\n 42 \nhello\n".to_vec());
    let mut interp =
        rice::interpreter::Interpreter::with_io(Box::new(output.clone()), Box::new(input));
    interp
        .run_source("INPUT n\nDIM text AS STRING\nINPUT text\nPRINT n\nPRINT text\n")
        .unwrap();
    assert_eq!(output.into_string(), "? ? Redo from start\n? ? 42\nhello\n");
}

#[test]
fn test_recursive_types_report_error() {
    for source in [
        "TYPE Node\n child AS Node\nEND TYPE\nDIM root AS Node\n",
        "TYPE A\n child AS B\nEND TYPE\nTYPE B\n child AS A\nEND TYPE\nDIM root AS A\n",
    ] {
        let (_, result) = run_bas_may_fail(source);
        assert!(result.unwrap_err().to_string().contains("recursive TYPE"));
    }
    assert_eq!(
        run_bas(
            "TYPE Leaf\n n AS INTEGER\nEND TYPE\nTYPE Pair\n a AS Leaf\n b AS Leaf\nEND TYPE\nDIM item AS Pair\nPRINT item.a.n\nPRINT item.b.n\n"
        ),
        "0\n0\n"
    );
}

#[test]
fn test_invalid_runtime_dimensions_and_positions() {
    for source in [
        "WIDTH -1\n",
        "WIDTH 0\n",
        "WIDTH 80, -1\n",
        "PRINT SCREEN(-1, 1)\n",
        "PRINT SCREEN(1, -1)\n",
        "DIM a(10 TO 1, 1 TO 2)\nMAT a = ZER\n",
        "REDIM a(10 TO 1, 1 TO 2)\nMAT a = ZER\n",
        "SEEK #1, -9223372036854775808\n",
    ] {
        assert!(run_bas_may_fail(source).1.is_err(), "{source}");
    }
}

#[test]
fn test_constant_assignment_reports_error() {
    for source in [
        "CONST n = 42\nn = 3\n",
        "CONST text$ = \"hello\"\nMID$(text$, 1) = \"x\"\n",
        "CONST n = 42\nSWAP n, m\n",
    ] {
        assert!(
            run_bas_may_fail(source)
                .1
                .unwrap_err()
                .to_string()
                .contains("cannot assign to constant"),
            "{source}"
        );
    }
}

#[test]
fn test_console_write_errors_are_reported() {
    struct BrokenOutput;
    impl std::io::Write for BrokenOutput {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "closed output",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut interp = rice::interpreter::Interpreter::with_io(
        Box::new(BrokenOutput),
        Box::new(Cursor::new(Vec::new())),
    );
    assert!(
        interp
            .run_source("PRINT 42\n")
            .unwrap_err()
            .to_string()
            .contains("closed output")
    );
}

#[test]
fn test_environ_rejects_nul_without_panicking() {
    assert!(
        run_bas_may_fail("ENVIRON \"RICE_TEST=\" + CHR$(0)\n")
            .1
            .is_err()
    );
}

#[test]
fn test_environ_changes_are_local_to_interpreter() {
    let key = "RICE_ENVIRON_ISOLATION_TEST";
    let original = std::env::var(key).ok();
    assert_eq!(
        run_bas(&format!(
            "ENVIRON \"{key}=local\"\nPRINT ENVIRON$(\"{key}\")\n"
        )),
        "local\n"
    );
    assert_eq!(std::env::var(key).ok(), original);
}

#[cfg(unix)]
#[test]
fn test_shell_inherits_interpreter_environment() {
    let (_, dir) = run_bas_with_tmpdir(
        r#"ENVIRON "RICE_SHELL_ENVIRON_TEST=hello"
SHELL "printf '%s' $RICE_SHELL_ENVIRON_TEST > '{DIR}/env.txt'"
"#,
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("env.txt")).unwrap(),
        "hello"
    );
}

#[test]
fn test_file_io_regressions() {
    let (output, _dir) = run_bas_with_tmpdir(include_str!("programs/review_file_io.bas"));
    assert_eq!(
        output,
        "héllo \"世界\"\n42\n62\n    A  B\nx               y\n6\n123\nrice\n3\n"
    );
}

#[test]
fn test_gosub_preserves_nested_loop_continuations() {
    assert_eq!(
        run_file("tests/programs/gosub_nested.bas"),
        "first1\nback1\nloop1\nfirst2\nback2\nloop2\ndone\n"
    );
}

#[test]
fn test_gosub_uses_current_procedure_labels() {
    assert_eq!(
        run_file("tests/programs/gosub_procedure_scope.bas"),
        "local1\nsub1\nlocal2\nsub2\nmain\n3\nmain\n"
    );
}

#[test]
fn test_gosub_end_stops_the_caller() {
    assert_eq!(run_file("tests/programs/gosub_end.bas"), "finished\n");
}

#[test]
fn test_gosub_can_return_from_a_local_block() {
    assert_eq!(
        run_bas(
            "DO\nGOSUB helper\nPRINT \"body\"\nEXIT DO\nhelper:\nPRINT \"local\"\nRETURN\nLOOP\nPRINT \"done\"\n"
        ),
        "local\nbody\ndone\n"
    );
}

#[test]
fn test_gosub_cannot_target_another_procedure() {
    let (_, result) =
        run_bas_may_fail("CALL Worker\nEND\nhelper:\nRETURN\nSUB Worker\nGOSUB helper\nEND SUB\n");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("undefined label: HELPER")
    );
}

#[test]
fn test_return_without_gosub_is_an_error() {
    let (_, result) = run_bas_may_fail("RETURN\n");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("RETURN without GOSUB")
    );
}

#[test]
fn test_gosub_preserves_on_error_resume_location() {
    assert_eq!(
        run_file("tests/programs/gosub_error_resume.bas"),
        "handler\n5:210\nresumed\nmain\n"
    );
}

#[test]
fn test_file_write_resets_print_column() {
    let (output, _dir) = run_bas_with_tmpdir(
        "OPEN \"{DIR}/columns.txt\" FOR OUTPUT AS #1\nPRINT #1, \"prefix\";\nWRITE #1, 1\nPRINT #1, TAB(5); \"x\"\nCLOSE #1\nOPEN \"{DIR}/columns.txt\" FOR INPUT AS #1\nLINE INPUT #1, text$\nLINE INPUT #1, text$\nPRINT text$\nCLOSE #1\n",
    );
    assert_eq!(output, "    x\n");
}

#[test]
fn test_undefined_goto_is_reported() {
    let (_, result) = run_bas_may_fail("GOTO missing\n");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("undefined label: MISSING")
    );
}

#[test]
fn test_exit_for_propagates_through_other_loops() {
    assert_eq!(
        run_bas(
            "FOR i = 1 TO 3\nWHILE i = 1\nDO\nEXIT FOR\nLOOP\nWEND\nPRINT \"unexpected\"\nNEXT i\nPRINT \"done\"\n"
        ),
        "done\n"
    );
}

#[test]
fn test_sub_control_flow_cannot_escape_to_callers_labels() {
    for statement in ["RETURN", "GOTO helper"] {
        let (_, result) = run_bas_may_fail(&format!(
            "GOSUB helper\nPRINT \"unexpected\"\nEND\nhelper:\nCALL Worker\nRETURN\nSUB Worker\n{statement}\nEND SUB\n"
        ));
        assert!(result.is_err(), "{statement} must remain in its procedure");
    }
}
