use std::io::Cursor;
use rice::interpreter::SharedOutput;

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
