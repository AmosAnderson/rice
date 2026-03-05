use std::io::Cursor;
use rice::interpreter::SharedOutput;

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
    assert_eq!(lines[0].trim(), "02-28-2026");
    // Time format should be HH:MM:SS
    let time = lines[1].trim();
    assert_eq!(time.len(), 8);
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
