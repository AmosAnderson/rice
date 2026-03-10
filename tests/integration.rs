use std::io::{BufReader, Cursor};
use rice::interpreter::SharedOutput;

/// Run a multi-file CHAIN test. `main_source` is the entry program; `files` is
/// a list of (filename, source) pairs written to the same temp directory.
/// Placeholders `{DIR}` in all sources are replaced with the temp dir path.
fn run_chain_test(main_source: &str, files: &[(&str, &str)]) -> String {
    let (output, result) = run_chain_test_may_fail(main_source, files);
    result.unwrap();
    output
}

/// Like run_chain_test but returns the error result too.
fn run_chain_test_may_fail(
    main_source: &str,
    files: &[(&str, &str)],
) -> (String, Result<(), Box<dyn std::error::Error>>) {
    let dir = tempfile::tempdir().unwrap();
    let dir_path = dir.path().to_str().unwrap().replace('\\', "/");

    for (name, content) in files {
        let file_content = content.replace("{DIR}", &dir_path);
        std::fs::write(dir.path().join(name), file_content).unwrap();
    }

    let main_content = main_source.replace("{DIR}", &dir_path);
    let main_path = dir.path().join("main.bas");
    std::fs::write(&main_path, &main_content).unwrap();

    let output = SharedOutput::new();
    let input: Box<dyn std::io::BufRead> = Box::new(BufReader::new(Cursor::new(Vec::<u8>::new())));
    let mut interp = rice::interpreter::Interpreter::with_io(Box::new(output.clone()), input);
    let result = interp.run_file(main_path.to_str().unwrap());
    (output.into_string(), result)
}

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
    let mut interp = rice::interpreter::Interpreter::with_io(
        Box::new(output.clone()),
        Box::new(input),
    );
    interp.run_source(source).unwrap();
    output.into_string()
}

fn run_bas_may_fail(source: &str) -> (String, Result<(), Box<dyn std::error::Error>>) {
    let output = SharedOutput::new();
    let input = Cursor::new(Vec::<u8>::new());
    let mut interp = rice::interpreter::Interpreter::with_io(
        Box::new(output.clone()),
        Box::new(input),
    );
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
    assert!(output.contains("while: 1"));
    assert!(output.contains("while: 3"));
    assert!(output.contains("until: 1"));
    assert!(output.contains("until: 3"));
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
fn test_gosub() {
    let output = run_file("tests/programs/gosub.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "before");
    assert_eq!(lines[1], "hello from gosub");
    assert_eq!(lines[2], "after");
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
    assert!(output.contains("HELLO"));
    assert!(output.contains("hello"));
    assert!(output.contains("A"));
    assert!(output.contains("65"));
    assert!(output.contains("42"));
    assert!(output.contains("3.14"));
    assert!(output.contains("*****"));
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
fn test_string_comparison() {
    let output = run_bas(r#"
IF "abc" < "def" THEN
    PRINT "yes"
ELSE
    PRINT "no"
END IF
"#);
    assert_eq!(output.trim(), "yes");
}

#[test]
fn test_nested_loops() {
    let output = run_bas("
FOR i = 1 TO 3
    FOR j = 1 TO 3
        PRINT i * 10 + j;
    NEXT j
    PRINT
NEXT i
");
    assert!(output.contains("11"));
    assert!(output.contains("33"));
}

#[test]
fn test_const() {
    let output = run_bas("
CONST PI = 3.14159
PRINT PI
");
    assert!(output.contains("3.14159"));
}

#[test]
fn test_single_line_if() {
    let output = run_bas("IF 5 > 3 THEN PRINT \"yes\" ELSE PRINT \"no\"\n");
    assert_eq!(output.trim(), "yes");
}

#[test]
fn test_exit_for() {
    let output = run_bas("
FOR i = 1 TO 100
    IF i = 5 THEN EXIT FOR
    PRINT i;
NEXT i
PRINT
");
    assert_eq!(output.trim(), "1  2  3  4");
}

#[test]
fn test_date_time() {
    let output = run_bas("PRINT DATE$\nPRINT TIME$");
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
    let (output, _dir) = run_bas_with_tmpdir(r#"
OPEN "{DIR}/test.txt" FOR OUTPUT AS #1
PRINT #1, "Hello, File!"
PRINT #1, "Second line"
CLOSE #1

OPEN "{DIR}/test.txt" FOR INPUT AS #1
LINE INPUT #1, a$
PRINT a$
LINE INPUT #1, b$
PRINT b$
PRINT EOF(1)
CLOSE #1
"#);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "Hello, File!");
    assert_eq!(lines[1], "Second line");
    assert_eq!(lines[2].trim(), "-1"); // EOF should be true
}

#[test]
fn test_file_write_read() {
    let (output, _dir) = run_bas_with_tmpdir(r#"
OPEN "{DIR}/test.txt" FOR OUTPUT AS #1
WRITE #1, "Alice", 30
WRITE #1, "Bob", 25
CLOSE #1

OPEN "{DIR}/test.txt" FOR INPUT AS #1
INPUT #1, name1$, age1%
PRINT name1$; age1%
INPUT #1, name2$, age2%
PRINT name2$; age2%
CLOSE #1
"#);
    let lines: Vec<&str> = output.lines().collect();
    assert!(lines[0].contains("Alice"));
    assert!(lines[0].contains("30"));
    assert!(lines[1].contains("Bob"));
    assert!(lines[1].contains("25"));
}

#[test]
fn test_file_append() {
    let (output, _dir) = run_bas_with_tmpdir(r#"
OPEN "{DIR}/test.txt" FOR OUTPUT AS #1
PRINT #1, "Line 1"
CLOSE #1

OPEN "{DIR}/test.txt" FOR APPEND AS #1
PRINT #1, "Line 2"
CLOSE #1

OPEN "{DIR}/test.txt" FOR INPUT AS #1
LINE INPUT #1, a$
PRINT a$
LINE INPUT #1, b$
PRINT b$
CLOSE #1
"#);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "Line 1");
    assert_eq!(lines[1], "Line 2");
}

#[test]
fn test_file_binary() {
    let (output, _dir) = run_bas_with_tmpdir(r#"
msg$ = "HELLO"
OPEN "{DIR}/test.bin" FOR BINARY AS #1
PUT #1, 1, msg$
CLOSE #1

OPEN "{DIR}/test.bin" FOR BINARY AS #1
GET #1, 1, result$
PRINT result$
CLOSE #1
"#);
    assert_eq!(output.trim(), "HELLO");
}

#[test]
fn test_file_freefile() {
    let (output, _dir) = run_bas_with_tmpdir(r#"
PRINT FREEFILE
OPEN "{DIR}/a.tmp" FOR OUTPUT AS #1
PRINT FREEFILE
OPEN "{DIR}/b.tmp" FOR OUTPUT AS #2
PRINT FREEFILE
CLOSE #1
PRINT FREEFILE
CLOSE #2
"#);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "1");
    assert_eq!(lines[1].trim(), "2");
    assert_eq!(lines[2].trim(), "3");
    assert_eq!(lines[3].trim(), "1"); // #1 freed, so FREEFILE returns 1
}

#[test]
fn test_file_lof() {
    let (output, _dir) = run_bas_with_tmpdir(r#"
OPEN "{DIR}/test.txt" FOR OUTPUT AS #1
PRINT #1, "Hello"
CLOSE #1

OPEN "{DIR}/test.txt" FOR INPUT AS #1
PRINT LOF(1)
CLOSE #1
"#);
    let lof: i64 = output.trim().parse().unwrap();
    assert!(lof > 0);
}

#[test]
fn test_file_eof_loop() {
    let (output, _dir) = run_bas_with_tmpdir(r#"
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
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "alpha");
    assert_eq!(lines[1], "beta");
    assert_eq!(lines[2], "gamma");
    assert_eq!(lines.len(), 3);
}

// ==================== ON ERROR GOTO / RESUME tests ====================

#[test]
fn test_on_error_resume_next() {
    let output = run_bas(r#"
ON ERROR GOTO handler
PRINT 1 / 0
PRINT "after"
END

handler:
RESUME NEXT
"#);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "after");
}

#[test]
fn test_on_error_goto_0() {
    // Enable handler, then disable it — error should propagate
    let (_output, result) = run_bas_may_fail(r#"
ON ERROR GOTO handler
ON ERROR GOTO 0
PRINT 1 / 0
END

handler:
RESUME NEXT
"#);
    assert!(result.is_err());
}

#[test]
fn test_on_error_resume_retry() {
    // RESUME (without NEXT) retries the failing statement.
    // We set up a variable so the second attempt succeeds.
    let output = run_bas(r#"
DIM x AS INTEGER
x = 0
ON ERROR GOTO handler
PRINT 1 / x
PRINT "done"
END

handler:
x = 1
RESUME
"#);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "1");
    assert_eq!(lines[1], "done");
}

#[test]
fn test_on_error_resume_label() {
    let output = run_bas(r#"
ON ERROR GOTO handler
PRINT 1 / 0
PRINT "should not print"
END

handler:
RESUME skip

skip:
PRINT "skipped to label"
"#);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "skipped to label");
}

#[test]
fn test_err_erl() {
    let output = run_bas(r#"
ON ERROR GOTO handler
PRINT 1 / 0
END

handler:
PRINT "ERR="; ERR
RESUME NEXT
"#);
    let lines: Vec<&str> = output.lines().collect();
    // ERR for division by zero = 11
    assert!(lines[0].contains("11"), "expected ERR=11, got: {}", lines[0]);
}

// ==================== PRINT USING tests ====================

#[test]
fn test_print_using_digits() {
    // Note: r####""## raw strings needed because Rust 2024 reserves ## in string literals
    let output = run_bas(r####"
PRINT USING "###.##"; 1.5
PRINT USING "###.##"; 123.456
PRINT USING "###.##"; -1.5
"####);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "  1.50");
    assert_eq!(lines[1], "123.46");
    // Negative: sign replaces a space
    assert_eq!(lines[2], " -1.50");
}

#[test]
fn test_print_using_dollar() {
    let output = run_bas(r####"
PRINT USING "$$###.##"; 1.5
PRINT USING "$$###.##"; 123.45
"####);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "  $1.50");
    assert_eq!(lines[1], "$123.45");
}

#[test]
fn test_print_using_sign() {
    let output = run_bas(r####"
PRINT USING "+###"; 5
PRINT USING "+###"; -5
PRINT USING "###-"; 5
PRINT USING "###-"; -5
"####);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "+  5");
    assert_eq!(lines[1], "-  5");
    assert_eq!(lines[2], "  5 ");
    assert_eq!(lines[3], "  5-");
}

#[test]
fn test_print_using_asterisk() {
    let output = run_bas(r####"
PRINT USING "**###.##"; 1.5
PRINT USING "**$###.##"; 1.5
"####);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "****1.50");
    assert_eq!(lines[1], "***$1.50");
}

#[test]
fn test_print_using_comma() {
    let output = run_bas(r####"
PRINT USING "#,###.##"; 1234.56
"####);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "1,234.56");
}

#[test]
fn test_print_using_scientific() {
    let output = run_bas(r####"
PRINT USING "##.##^^^^"; 1234.5
"####);
    let lines: Vec<&str> = output.lines().collect();
    // digits_before=2, so: 12.35E+02
    assert_eq!(lines[0], "12.35E+02");
}

#[test]
fn test_print_using_string() {
    let output = run_bas(r####"
PRINT USING "!"; "Hello"
PRINT USING "\   \"; "Hello"
PRINT USING "&"; "Hello"
"####);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "H");
    assert_eq!(lines[1], "Hello");
    assert_eq!(lines[2], "Hello");
}

#[test]
fn test_print_using_escape() {
    let output = run_bas(r####"
PRINT USING "_###.##"; 1.5
"####);
    let lines: Vec<&str> = output.lines().collect();
    // _ escapes #, so first # is literal, then ##.## is a 2-digit format
    assert_eq!(lines[0], "# 1.50");
}

#[test]
fn test_print_using_overflow() {
    let output = run_bas(r####"
PRINT USING "##.##"; 12345.67
"####);
    let lines: Vec<&str> = output.lines().collect();
    // Number too wide for field — % prefix
    assert!(lines[0].starts_with('%'), "expected overflow prefix %, got: {}", lines[0]);
}

#[test]
fn test_print_using_repeat() {
    let output = run_bas(r####"
PRINT USING "###"; 1; 2; 3
"####);
    let lines: Vec<&str> = output.lines().collect();
    // Format repeats for each value
    assert_eq!(lines[0], "  1  2  3");
}

#[test]
fn test_print_using_file() {
    let (output, _dir) = run_bas_with_tmpdir(r####"
OPEN "{DIR}/test.txt" FOR OUTPUT AS #1
PRINT #1, USING "###.##"; 3.14
CLOSE #1

OPEN "{DIR}/test.txt" FOR INPUT AS #1
LINE INPUT #1, x$
PRINT x$
CLOSE #1
"####);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "  3.14");
}

#[test]
fn test_on_goto() {
    let output = run_file("tests/programs/on_goto.bas");
    let lines: Vec<&str> = output.lines().collect();
    // ON 2 GOTO → second label
    assert_eq!(lines[0].trim(), "two");
    // ON 0 GOTO → fall through
    assert_eq!(lines[1].trim(), "zero-fallthrough");
    // ON 5 GOTO → fall through (out of range)
    assert_eq!(lines[2].trim(), "over-fallthrough");
    // ON n GOSUB tests
    assert_eq!(lines[3].trim(), "sub1");
    assert_eq!(lines[4].trim(), "sub2");
    assert_eq!(lines[5].trim(), "sub3");
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
    // After CLEAR, x should auto-init to 0 and y$ to ""
    assert_eq!(lines[0].trim(), "0");
    assert_eq!(lines[1].trim(), "");
}

#[test]
fn test_mid_assign() {
    let output = run_file("tests/programs/mid_assign.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "Hello BASIC");
    assert_eq!(lines[1], "12ABCD7890");
    assert_eq!(lines[2], "HiXXXXXXXX");
}

#[test]
fn test_lset_rset() {
    let output = run_file("tests/programs/lset_rset.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "|Hello     |");
    assert_eq!(lines[1], "|        Hi|");
    assert_eq!(lines[2], "|ABCDE|");
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
fn test_deftype() {
    let output = run_file("tests/programs/deftype.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "2"); // 7 \ 3 = 2 (integer div)
}

#[test]
fn test_def_fn() {
    let output = run_file("tests/programs/def_fn.bas");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "25");  // FNSquare(5)
    assert_eq!(lines[1].trim(), "27");  // FNCube(3)
    assert_eq!(lines[2].trim(), "24");  // FNSquare(4) + FNCube(2) = 16 + 8
}

#[test]
fn test_environ() {
    // Test ENVIRON$ function returns a non-empty value for a known env var
    let output = run_bas(r#"
        x$ = ENVIRON$("PATH")
        IF LEN(x$) > 0 THEN
            PRINT "has path"
        ELSE
            PRINT "no path"
        END IF
        y$ = ENVIRON$("NONEXISTENT_VAR_12345")
        PRINT LEN(y$)
    "#);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0], "has path");
    assert_eq!(lines[1].trim(), "0");
}

#[test]
fn test_file_ops() {
    let (output, _dir) = run_bas_with_tmpdir(r#"
        MKDIR "{DIR}/testsubdir"
        OPEN "{DIR}/testsubdir/test.txt" FOR OUTPUT AS #1
        PRINT #1, "hello"
        CLOSE #1
        NAME "{DIR}/testsubdir/test.txt" AS "{DIR}/testsubdir/renamed.txt"
        OPEN "{DIR}/testsubdir/renamed.txt" FOR INPUT AS #1
        LINE INPUT #1, x$
        CLOSE #1
        PRINT x$
        KILL "{DIR}/testsubdir/renamed.txt"
        RMDIR "{DIR}/testsubdir"
        PRINT "done"
    "#);
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
fn test_binary_conversion() {
    let output = run_bas(r#"
        a$ = MKI$(1000)
        PRINT LEN(a$)
        PRINT CVI(a$)
        b$ = MKL$(123456)
        PRINT LEN(b$)
        PRINT CVL(b$)
    "#);
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "2");
    assert_eq!(lines[1].trim(), "1000");
    assert_eq!(lines[2].trim(), "4");
    assert_eq!(lines[3].trim(), "123456");
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
    assert_eq!(lines[0].trim(), "10  20");
    assert_eq!(lines[1].trim(), "30  40");
    assert_eq!(lines[2].trim(), "50  60");
}

#[test]
fn test_type_sub() {
    let output = run_file("tests/programs/type_sub.bas");
    assert_eq!(output.trim(), "16.5");
}

// ─── CHAIN / COMMON tests ───────────────────────────────────────────────

#[test]
fn test_chain_basic() {
    // Basic CHAIN with COMMON: transfer integer and string by position
    let output = run_chain_test(
        r#"
COMMON X AS INTEGER, Y AS STRING
X = 42
Y = "Hello from A"
PRINT "A: X ="; X
CHAIN "{DIR}/chain_b.bas"
PRINT "This should not appear"
"#,
        &[(
            "chain_b.bas",
            r#"
COMMON A AS INTEGER, B AS STRING
PRINT "B: A ="; A
PRINT "B: B = "; B
"#,
        )],
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "A: X = 42");
    assert_eq!(lines[1].trim(), "B: A = 42");
    assert_eq!(lines[2].trim(), "B: B = Hello from A");
    assert_eq!(lines.len(), 3, "Code after CHAIN should not execute");
}

#[test]
fn test_chain_no_common() {
    // CHAIN without COMMON: variables should be cleared
    let output = run_chain_test(
        r#"
DIM X AS INTEGER
X = 99
PRINT "A: X ="; X
CHAIN "{DIR}/chain_b.bas"
"#,
        &[(
            "chain_b.bas",
            r#"
PRINT "B: X ="; X
"#,
        )],
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "A: X = 99");
    assert_eq!(lines[1].trim(), "B: X = 0");
}

#[test]
fn test_chain_file_not_found() {
    let (_, result) = run_chain_test_may_fail(
        r#"
CHAIN "{DIR}/nonexistent.bas"
"#,
        &[],
    );
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("CHAIN error"), "Error should mention CHAIN: {err_msg}");
}

#[test]
fn test_chain_common_larger_target() {
    // Source has 1 COMMON var, target has 3. Extra vars get defaults.
    let output = run_chain_test(
        r#"
COMMON X AS INTEGER
X = 10
CHAIN "{DIR}/chain_b.bas"
"#,
        &[(
            "chain_b.bas",
            r#"
COMMON A AS INTEGER, B AS INTEGER, C AS STRING
PRINT "A ="; A
PRINT "B ="; B
PRINT "C = ["; C; "]"
"#,
        )],
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "A = 10");
    assert_eq!(lines[1].trim(), "B = 0");
    assert_eq!(lines[2].trim(), "C = []");
}

#[test]
fn test_chain_common_smaller_target() {
    // Source has 3 COMMON vars, target has 1. Extra source vars are ignored.
    let output = run_chain_test(
        r#"
COMMON X AS INTEGER, Y AS INTEGER, Z AS INTEGER
X = 10
Y = 20
Z = 30
CHAIN "{DIR}/chain_b.bas"
"#,
        &[(
            "chain_b.bas",
            r#"
COMMON A AS INTEGER
PRINT "A ="; A
"#,
        )],
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "A = 10");
}

#[test]
fn test_chain_common_array() {
    // Transfer an array via COMMON
    let output = run_chain_test(
        r#"
COMMON A() AS INTEGER
DIM A(4)
A(0) = 10
A(1) = 20
A(2) = 30
A(3) = 40
A(4) = 50
CHAIN "{DIR}/chain_b.bas"
"#,
        &[(
            "chain_b.bas",
            r#"
COMMON B() AS INTEGER
PRINT B(0)
PRINT B(1)
PRINT B(2)
PRINT B(3)
PRINT B(4)
"#,
        )],
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "10");
    assert_eq!(lines[1].trim(), "20");
    assert_eq!(lines[2].trim(), "30");
    assert_eq!(lines[3].trim(), "40");
    assert_eq!(lines[4].trim(), "50");
}

#[test]
fn test_chain_common_shared() {
    // COMMON SHARED variables should be accessible from SUB
    let output = run_chain_test(
        r#"
COMMON SHARED X AS INTEGER
X = 42
CHAIN "{DIR}/chain_b.bas"
"#,
        &[(
            "chain_b.bas",
            r#"
COMMON SHARED A AS INTEGER

SUB ShowA
    PRINT "In SUB: A ="; A
END SUB

CALL ShowA
"#,
        )],
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "In SUB: A = 42");
}

#[test]
fn test_chain_multiple_hops() {
    // A → B → C: values flow through the chain
    let output = run_chain_test(
        r#"
COMMON X AS INTEGER
X = 1
PRINT "A: X ="; X
CHAIN "{DIR}/chain_b.bas"
"#,
        &[
            (
                "chain_b.bas",
                r#"
COMMON Y AS INTEGER
Y = Y + 10
PRINT "B: Y ="; Y
CHAIN "{DIR}/chain_c.bas"
"#,
            ),
            (
                "chain_c.bas",
                r#"
COMMON Z AS INTEGER
PRINT "C: Z ="; Z
"#,
            ),
        ],
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "A: X = 1");
    assert_eq!(lines[1].trim(), "B: Y = 11");
    assert_eq!(lines[2].trim(), "C: Z = 11");
}

#[test]
fn test_chain_file_handles_preserved() {
    // File handles should persist across CHAIN
    let output = run_chain_test(
        r#"
COMMON X AS INTEGER
X = 1
OPEN "{DIR}/test_data.txt" FOR OUTPUT AS #1
PRINT #1, "Hello from A"
CHAIN "{DIR}/chain_b.bas"
"#,
        &[(
            "chain_b.bas",
            r#"
COMMON A AS INTEGER
PRINT #1, "Hello from B"
CLOSE #1
OPEN "{DIR}/test_data.txt" FOR INPUT AS #1
DIM line1 AS STRING
DIM line2 AS STRING
LINE INPUT #1, line1
LINE INPUT #1, line2
CLOSE #1
PRINT line1
PRINT line2
"#,
        )],
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "Hello from A");
    assert_eq!(lines[1].trim(), "Hello from B");
}

#[test]
fn test_chain_type_coercion() {
    // Source has DOUBLE, target has INTEGER: value should be truncated
    let output = run_chain_test(
        r#"
COMMON X AS DOUBLE
X = 3.14
CHAIN "{DIR}/chain_b.bas"
"#,
        &[(
            "chain_b.bas",
            r#"
COMMON A AS INTEGER
PRINT "A ="; A
"#,
        )],
    );
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "A = 3");
}

// ── Phase 1+2: Console features ──────────────────────────────────────

#[test]
fn test_cls() {
    let output = run_bas("CLS\n");
    assert!(output.contains("\x1b[2J\x1b[H"), "CLS should emit ANSI clear + home");
}

#[test]
fn test_beep() {
    let output = run_bas("BEEP\n");
    assert!(output.contains("\x07"), "BEEP should emit BEL character");
}

#[test]
fn test_locate() {
    let output = run_bas("LOCATE 5, 10\nPRINT \"X\"\n");
    assert!(output.contains("\x1b[5;10H"), "LOCATE should emit ANSI cursor move");
    assert!(output.contains("X"));
}

#[test]
fn test_locate_row_only() {
    let output = run_bas("LOCATE 3\nPRINT \"Y\"\n");
    assert!(output.contains("\x1b[3;1H"), "LOCATE with row only should keep col=1");
}

#[test]
fn test_color() {
    let output = run_bas("COLOR 4, 1\nPRINT \"red on blue\"\n");
    // QBasic color 4 = red -> ANSI 31, color 1 = blue -> ANSI 44
    assert!(output.contains("\x1b[31;44m"), "COLOR 4,1 should emit combined ANSI 31;44");
}

#[test]
fn test_color_fg_only() {
    let output = run_bas("COLOR 2\nPRINT \"green\"\n");
    // QBasic color 2 = green -> ANSI 32
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
    assert!(output.contains(" 7"), "CSRLIN should return 7 after LOCATE 7");
}

#[test]
fn test_pos() {
    let output = run_bas("LOCATE 1, 12\nPRINT POS(0)\n");
    assert!(output.contains(" 12"), "POS(0) should return 12 after LOCATE ,12");
}

#[test]
fn test_width() {
    let output = run_bas("WIDTH 40\nPRINT \"ok\"\n");
    assert!(output.contains("ok"), "WIDTH should not crash");
}

#[test]
fn test_view_print() {
    let output = run_bas("VIEW PRINT 5 TO 20\n");
    assert!(output.contains("\x1b[5;20r"), "VIEW PRINT should emit ANSI scroll region");
}

#[test]
fn test_view_print_reset() {
    let output = run_bas("VIEW PRINT\n");
    // Reset emits ANSI scroll region reset (no args)
    assert!(output.contains("\x1b[r"), "VIEW PRINT (no args) should reset scroll region");
}

// ── Phase 3: INKEY$ and INPUT$ ───────────────────────────────────────

#[test]
fn test_inkey_returns_empty_in_test_mode() {
    let output = run_bas("PRINT INKEY$\n");
    // In non-interactive mode, INKEY$ returns ""
    assert_eq!(output.trim(), "");
}

#[test]
fn test_inkey_in_expression() {
    let output = run_bas(r#"
DIM k AS STRING
k = INKEY$
IF k = "" THEN PRINT "empty" ELSE PRINT "key"
"#);
    assert_eq!(output.trim(), "empty");
}

#[test]
fn test_input_dollar_from_file() {
    let (output, _dir) = run_bas_with_tmpdir(r#"
OPEN "{DIR}/test.dat" FOR OUTPUT AS #1
PRINT #1, "HELLO WORLD"
CLOSE #1
OPEN "{DIR}/test.dat" FOR INPUT AS #2
DIM s AS STRING
s = INPUT$(5, #2)
PRINT s
CLOSE #2
"#);
    assert_eq!(output.trim(), "HELLO");
}

#[test]
fn test_input_dollar_from_file_no_hash() {
    let (output, _dir) = run_bas_with_tmpdir(r#"
OPEN "{DIR}/test.dat" FOR OUTPUT AS #1
PRINT #1, "ABCDEFG"
CLOSE #1
OPEN "{DIR}/test.dat" FOR INPUT AS #2
DIM s AS STRING
s = INPUT$(3, 2)
PRINT s
CLOSE #2
"#);
    assert_eq!(output.trim(), "ABC");
}

// ── Phase 4: SCREEN() function ───────────────────────────────────────

#[test]
fn test_screen_function() {
    let output = run_bas(r#"
LOCATE 1, 1
PRINT "A";
PRINT SCREEN(1, 1)
"#);
    // SCREEN(1,1) should return ASCII code of 'A' = 65
    assert!(output.contains(" 65"), "SCREEN(1,1) should return 65 for 'A'");
}

#[test]
fn test_screen_function_empty() {
    let output = run_bas(r#"
CLS
PRINT SCREEN(5, 5)
"#);
    // Empty screen position should return 32 (space)
    assert!(output.contains(" 32"), "Empty position should return 32 (space)");
}

#[test]
fn test_screen_function_after_print() {
    let output = run_bas(r#"
CLS
LOCATE 2, 3
PRINT "XY";
PRINT SCREEN(2, 3); SCREEN(2, 4)
"#);
    // X=88, Y=89
    assert!(output.contains(" 88"), "SCREEN(2,3) should return 88 for 'X'");
    assert!(output.contains(" 89"), "SCREEN(2,4) should return 89 for 'Y'");
}
