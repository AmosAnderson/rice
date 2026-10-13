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
fn test_byref_array_elements_in_subs_functions_and_records() {
    let source = std::fs::read_to_string("tests/programs/byref_elements.bas").unwrap();
    for dialect in ["QB", "ANSI"] {
        assert_eq!(
            run_bas(&format!("OPTION DIALECT \"{dialect}\"\n{source}")),
            "11\n130|13\nupdated\n42|0\n"
        );
    }
}

#[test]
fn test_byref_array_indices_are_evaluated_once_and_frozen_left_to_right() {
    for dialect in ["QB", "ANSI"] {
        let source = format!(
            r#"
OPTION DIALECT "{dialect}"
DIM a(2)
a(1) = 10: a(2) = 20
i = 1: calls = 0
CALL Change(a(Index()), MoveIndex())
PRINT calls; "|"; i; "|"; a(1); "|"; a(2)
i = 1
PRINT ChangeFunction(a(Index())); "|"; calls; "|"; i; "|"; a(1); "|"; a(2)
SUB Change(BYREF target, BYVAL ignored)
    target = 30
END SUB
FUNCTION ChangeFunction(BYREF target)
    SHARED i
    i = 2
    target = 40
    ChangeFunction = target
END FUNCTION
FUNCTION Index()
    SHARED calls, i
    calls = calls + 1
    Index = i
END FUNCTION
FUNCTION MoveIndex()
    SHARED i
    i = 2
    MoveIndex = 0
END FUNCTION
"#
        );
        assert_eq!(run_bas(&source), "1|2|30|20\n40|2|2|40|20\n");
    }
}

#[test]
fn test_byref_array_copyback_aliases_defaults_and_expression_suppression() {
    for dialect in ["QB", "ANSI"] {
        let source = format!(
            r#"
OPTION DIALECT "{dialect}"
DIM a(1), s$(1)
a(1) = 5
CALL Pair(a(1), a(1))
PRINT a(1)
CALL Change((a(1)))
CALL Change(+a(1))
CALL Change(a(1) + 0)
CALL ByValue(a(1))
CALL WholeArray(a())
PRINT a(1)
Change (a(1))
PRINT a(1)
CALL ObserveDefault(s$(1))
PRINT s$(1)
CALL Implicit(a(1))
PRINT a(1)
SUB Pair(BYREF first, BYREF second)
    first = 10
    second = second + 2
END SUB
SUB Change(BYREF target)
    target = 99
END SUB
SUB ByValue(BYVAL target)
    target = 99
END SUB
SUB WholeArray(BYREF target())
    target = 99
END SUB
SUB ObserveDefault(BYREF text$)
    PRINT LEN(text$)
    text$ = "ok"
END SUB
SUB Implicit(target)
    target = 88
END SUB
"#
        );
        let (parenthesized_call, result) = if dialect == "QB" { (7, 88) } else { (99, 99) };
        assert_eq!(
            run_bas(&source),
            format!("7\n7\n{parenthesized_call}\n0\nok\n{result}\n")
        );
    }
}

#[test]
fn test_byref_array_copyback_on_errors_and_nested_calls() {
    for kind in ["SUB", "FUNCTION"] {
        for dialect in ["QB", "ANSI"] {
            let call = if kind == "SUB" {
                "CALL Change(a(1))"
            } else {
                "result = Change(a(1))"
            };
            let (mut interpreter, output) = input_session("");
            let error = interpreter
                .run_source(&format!(
                    r#"
OPTION DIALECT "{dialect}"
DIM a(1)
a(1) = 3
{call}
{kind} Change(BYREF target)
    CALL Nested(target)
    failure = 1 / 0
END {kind}
SUB Nested(BYREF target)
    target = 17
END SUB
"#
                ))
                .unwrap_err();
            assert!(matches!(
                error.downcast_ref::<rice::error::RuntimeError>(),
                Some(rice::error::RuntimeError::DivisionByZero)
            ));
            interpreter.run_source("PRINT a(1)\n").unwrap();
            assert_eq!(output.into_string(), "17\n");
        }
    }
}

#[test]
fn test_byref_arrays_preserve_function_resolution_and_constants() {
    let source = r#"
DIM values(1), boxed(1)
CONST values_1 = 12
boxed(1) = 14
CONST boxed = 13
"#;
    for dialect in ["QB", "ANSI"] {
        let source = format!(
            r#"
OPTION DIALECT "{dialect}"
{source}
CALL Change(values(1))
CALL Change(boxed(1))
PRINT values(1); "|"; boxed(1)
DIM ABS(1), Produce(1)
ABS(1) = 5: Produce(1) = 6
CALL Change(ABS(-1))
CALL Change(Produce(1))
CALL Change(Answer)
PRINT ABS_1; "|"; PRODUCE_1; "|"; Answer
CALL Change(LEFT$("abc", 1))
PRINT LEFT$("abc", 1)
SUB Change(BYREF target)
    target = 99
END SUB
FUNCTION Produce(BYVAL ignored)
    Produce = 7
END FUNCTION
FUNCTION Answer()
    Answer = 42
END FUNCTION
"#
        );
        assert_eq!(run_bas(&source), "12|14\n5|6|42\na\n");
    }
}

#[test]
fn test_byref_array_invalid_indices_and_explicit_declarations() {
    for index in ["1 / 0", "VAL(\"NaN\")", "\"bad\""] {
        let (output, result) = run_bas_may_fail(&format!(
            "DIM a(1)\nCALL Change(a({index}))\nSUB Change(BYREF target)\nPRINT \"unreachable\"\nEND SUB\n"
        ));
        assert!(result.is_err(), "{index}");
        assert!(output.is_empty(), "{index}");
    }
    assert_eq!(
        run_bas(
            "OPTION EXPLICIT\nDIM a(1)\nCALL Change(a(1.9))\nPRINT a(1)\nSUB Change(BYREF target)\ntarget = 7\nEND SUB\n"
        ),
        "7\n"
    );
    let (output, result) = run_bas_may_fail(
        "OPTION EXPLICIT\nCALL Change(missing(1))\nSUB Change(BYREF target)\nPRINT \"unreachable\"\nEND SUB\n",
    );
    assert!(result.unwrap_err().to_string().contains("not declared"));
    assert!(output.is_empty());
}

fn input_session(input: &str) -> (rice::interpreter::Interpreter, SharedOutput) {
    let output = SharedOutput::new();
    let interpreter = rice::interpreter::Interpreter::with_io(
        Box::new(output.clone()),
        Box::new(Cursor::new(input.as_bytes().to_vec())),
    );
    (interpreter, output)
}

fn run_bas_with_input(source: &str, input: &str) -> String {
    let (mut interpreter, output) = input_session(input);
    interpreter.run_source(source).unwrap();
    output.into_string()
}

#[test]
fn test_hello() {
    assert_eq!(run_file("tests/programs/hello.bas"), "Hello, World!\n");
}

#[test]
fn test_indexed_swap_in_both_dialects() {
    let source = std::fs::read_to_string("tests/programs/indexed_swap.bas").unwrap();
    let expected = "20;10\n20;1\n10\n[hello][]\n[second][]\n10;0\n";
    assert_eq!(run_file("tests/programs/indexed_swap.bas"), expected);
    assert_eq!(
        run_bas(&format!("OPTION DIALECT \"ANSI\"\n{source}")),
        expected
    );
}

#[test]
fn test_indexed_swap_resolves_once_before_reading_values() {
    for dialect in ["QB", "ANSI"] {
        let source = format!(
            r#"
OPTION DIALECT "{dialect}"
DIM values(2)
values(1) = 10
values(2) = 20
calls = 0
trace$ = ""
FUNCTION Pick(BYVAL n)
  SHARED calls, trace$
  calls = calls + 1
  trace$ = trace$ & STR$(n)
  Pick = n
END FUNCTION
SWAP values(Pick(1)), values(Pick(2))
PRINT values(1); ";"; values(2); ";"; calls; ";"; trace$
left = 30
FUNCTION ChangeIndex()
  SHARED left
  left = 40
  ChangeIndex = 1
END FUNCTION
SWAP left, values(ChangeIndex())
PRINT left; ";"; values(1)
"#
        );
        assert_eq!(run_bas(&source), "20;10;2;12\n20;40\n");
    }
}

#[test]
fn test_indexed_swap_failure_preserves_both_values() {
    for (setup, target, message) in [
        ("", "values(1 / 0)", "Division by zero"),
        ("", "values(\"text\")", "Type mismatch"),
        ("CONST fixed = 9\n", "fixed", "constant"),
        ("CONST values_2 = 9\n", "values(2)", "constant"),
        ("OPTION EXPLICIT\n", "unknown(1)", "not declared"),
    ] {
        let (mut interpreter, output) = input_session("");
        interpreter
            .run_source("DIM values(2)\nvalues(1) = 10\nvalues(2) = 20\n")
            .unwrap();
        let error = interpreter
            .run_source(&format!("{setup}SWAP values(1), {target}\n"))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .to_lowercase()
                .contains(&message.to_lowercase()),
            "{target}: {error}"
        );
        interpreter
            .run_source("PRINT values(1); \";\"; values(2)\n")
            .unwrap();
        let expected = if target == "values(2)" {
            "10;9\n"
        } else {
            "10;20\n"
        };
        assert_eq!(output.into_string(), expected, "{target}");
    }
}

#[test]
fn test_indexed_swap_defaults_and_option_explicit() {
    assert_eq!(
        run_bas(
            r#"
DEFSTR A
text$ = "text"
SWAP absent, text$
PRINT "["; absent; "]["; text$; "]"
TYPE Entry
  number AS INTEGER
END TYPE
DIM entries(2) AS Entry, existing AS Entry
existing.number = 7
SWAP entries(1), existing
PRINT entries(1).number; ";"; existing.number
OPTION EXPLICIT
DIM values(2), scalar
scalar = 8
SWAP values(2), scalar
PRINT values(2); ";"; scalar
"#
        ),
        "[text][]\n7;0\n8;0\n"
    );
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
fn test_logical_eqv_imp() {
    assert_eq!(
        run_file("tests/programs/logical_eqv_imp.bas"),
        "-1;0;0;-1\n-1;0;-1;-1\n-7;-5;-7;6\n-1;7;5\n-1;-1;-1\n-1;-1\n-1\n01\n-1\n-1-1\n"
    );
}

#[test]
fn test_logical_eqv_imp_errors_and_dialect() {
    for operator in ["EQV", "IMP"] {
        let (_, result) = run_bas_may_fail(&format!("PRINT 0 {operator} (1 / 0)\n"));
        assert!(matches!(
            result
                .unwrap_err()
                .downcast_ref::<rice::error::RuntimeError>(),
            Some(rice::error::RuntimeError::DivisionByZero)
        ));
        for (left, right) in [("\"text\"", "1"), ("1", "\"text\"")] {
            let (_, result) = run_bas_may_fail(&format!("PRINT {left} {operator} {right}\n"));
            assert!(matches!(
                result
                    .unwrap_err()
                    .downcast_ref::<rice::error::RuntimeError>(),
                Some(rice::error::RuntimeError::TypeMismatch { .. })
            ));
        }
        let (_, result) =
            run_bas_may_fail(&format!("OPTION DIALECT \"ANSI\"\nPRINT 1 {operator} 0\n"));
        assert!(result.unwrap_err().to_string().contains("ANSI"));
    }
}

#[test]
fn test_integer_division() {
    assert_eq!(
        run_file("tests/programs/integer_division.bas"),
        "3;-3;-3;3\n4;3;-4;-3\n4;2;-4;0\n3;10;0\n4;6;-2\n1.5;2.5\n3002399751580330\n4\nliteral backslash: \\\n"
    );
}

#[test]
fn test_integer_division_errors() {
    for expr in ["1 \\ 0", "1 \\ 0.5", "1 \\ -0.5", "1 \\ 0.1"] {
        let (_, result) = run_bas_may_fail(&format!("PRINT {expr}\n"));
        assert!(
            matches!(
                result
                    .unwrap_err()
                    .downcast_ref::<rice::error::RuntimeError>(),
                Some(rice::error::RuntimeError::DivisionByZero)
            ),
            "{expr}"
        );
    }
    for expr in [
        "(2 ^ 63) \\ 1",
        "1 \\ (2 ^ 63)",
        "(-2 ^ 63) \\ -1",
        "(-2 ^ 64) \\ 1",
        "EXP(1000) \\ 1",
        "1 \\ EXP(1000)",
        "((-1) ^ 0.5) \\ 1",
    ] {
        let (_, result) = run_bas_may_fail(&format!("PRINT {expr}\n"));
        assert!(
            matches!(
                result
                    .unwrap_err()
                    .downcast_ref::<rice::error::RuntimeError>(),
                Some(rice::error::RuntimeError::Overflow { .. })
            ),
            "{expr}"
        );
    }
    for expr in ["\"7\" \\ 2", "7 \\ \"2\""] {
        let (_, result) = run_bas_may_fail(&format!("PRINT {expr}\n"));
        assert!(matches!(
            result
                .unwrap_err()
                .downcast_ref::<rice::error::RuntimeError>(),
            Some(rice::error::RuntimeError::TypeMismatch { .. })
        ));
    }
}

#[test]
fn test_integer_division_error_trapping() {
    let source = r#"
ON ERROR GOTO handler
10 PRINT 1 \ 0.5
20 PRINT (-2 ^ 63) \ -1
30 PRINT "recovered"
END
handler:
PRINT ERR; ";"; ERL
RESUME NEXT
"#;
    assert_eq!(run_bas(source), "11;10\n6;20\nrecovered\n");
}

#[test]
fn test_integer_division_dialect_selection() {
    let (_, result) = run_bas_may_fail("OPTION DIALECT \"ANSI\"\nPRINT 7 \\ 2\n");
    assert!(result.unwrap_err().to_string().contains("ANSI"));
    let output = SharedOutput::new();
    let mut interp = rice::interpreter::Interpreter::with_io(
        Box::new(output.clone()),
        Box::new(Cursor::new(Vec::<u8>::new())),
    );
    interp.run_source("OPTION DIALECT \"ANSI\"\n").unwrap();
    assert!(interp.run_source("PRINT 7 \\ 2\n").is_err());
    interp.run_source("OPTION DIALECT \"QB\"\n").unwrap();
    interp.run_source("PRINT 7 \\ 2\n").unwrap();
    assert_eq!(output.into_string(), "3\n");
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
fn test_recursive_procedures_reuse_local_labels() {
    assert_eq!(
        run_file("tests/programs/procedure_reuse.bas"),
        "12\n6\nmain\n12\n6\nmain\n"
    );
}

#[test]
fn test_procedure_redefinition_refreshes_bodies_and_labels() {
    let output = SharedOutput::new();
    let mut interp = rice::interpreter::Interpreter::with_io(
        Box::new(output.clone()),
        Box::new(Cursor::new(Vec::<u8>::new())),
    );
    interp
        .run_source(
            r#"
SUB Worker
    GOSUB helper
    EXIT SUB
helper:
    PRINT "old"
    RETURN
END SUB
FUNCTION Score()
    GOSUB helper
    EXIT FUNCTION
helper:
    Score = 1
    RETURN
END FUNCTION
"#,
        )
        .unwrap();
    // Definitions and cached labels outlive the source program, including bare function calls.
    for _ in 0..2 {
        interp
            .run_source("CALL Worker\nPRINT Score\nPRINT Score()\n")
            .unwrap();
    }
    interp
        .run_source(
            r#"
SUB Worker
    PRINT "new"
    GOSUB helper
    EXIT SUB
helper:
    PRINT "helper"
    RETURN
END SUB
FUNCTION Score()
    Score = 20
    GOSUB helper
    EXIT FUNCTION
helper:
    Score = Score + 2
    RETURN
END FUNCTION
"#,
        )
        .unwrap();
    interp
        .run_source("CALL Worker\nPRINT Score\nPRINT Score()\n")
        .unwrap();
    assert_eq!(
        output.into_string(),
        "old\n1\n1\nold\n1\n1\nnew\nhelper\n22\n22\n"
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

#[test]
fn test_if_line_number_shorthand() {
    assert_eq!(
        run_file("tests/programs/if_line_number.bas"),
        "then\nelse\nfallthrough\nmixed else\nsecond\nmixed then\ncolon jump\n"
    );
}

#[test]
fn test_if_line_number_shorthand_nested_else() {
    assert_eq!(
        run_bas(
            r#"
IF 1 THEN IF 0 THEN 100 ELSE PRINT "inner" ELSE PRINT "wrong outer"
IF 0 THEN IF 1 THEN PRINT "wrong inner" ELSE 100 ELSE PRINT "outer"
IF 1 THEN IF 1 THEN 100 ELSE PRINT "wrong inner" ELSE PRINT "wrong outer"
PRINT "unreachable"
END
100 PRINT "jumped"
"#,
        ),
        "inner\nouter\njumped\n"
    );
}

#[test]
fn test_if_line_number_shorthand_uses_procedure_labels() {
    assert_eq!(
        run_bas(
            r#"
SUB Worker(flag)
    IF flag THEN 100 ELSE 200
100 PRINT "sub then": EXIT SUB
200 PRINT "sub else"
END SUB
FUNCTION Pick(flag)
    IF flag THEN 100 ELSE 200
100 Pick = 10: EXIT FUNCTION
200 Pick = 20
END FUNCTION
CALL Worker(1)
CALL Worker(0)
PRINT Pick(1); ","; Pick(0)
END
100 PRINT "wrong global"
200 PRINT "wrong global"
"#,
        ),
        "sub then\nsub else\n10,20\n"
    );
}

#[test]
fn test_if_line_number_shorthand_requires_an_existing_target() {
    let (_, result) = run_bas_may_fail("IF 1 THEN 999\n");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("undefined label: 999")
    );
}

#[test]
fn test_if_line_number_shorthand_before_exception_boundaries() {
    for target in ["100", "GOTO 100"] {
        assert_eq!(
            run_bas(&format!(
                "WHEN EXCEPTION IN\nIF 1 THEN {target} USE PRINT \"handler\"\nEND WHEN\n100 PRINT \"jumped\"\n"
            )),
            "jumped\n"
        );
        assert_eq!(
            run_bas(&format!(
                "WHEN EXCEPTION IN\nPRINT 1 / 0\nUSE\nIF 0 THEN PRINT \"wrong\" ELSE {target} END WHEN\n100 PRINT \"recovered\"\n"
            )),
            "recovered\n"
        );
    }
}

#[test]
fn test_if_line_number_shorthand_rejects_ansi() {
    for statement in ["IF 1 THEN 100", "IF 1 THEN PRINT 1 ELSE 100"] {
        let (output, result) = run_bas_may_fail(&format!(
            "OPTION DIALECT \"ANSI\"\n{statement}\n100 PRINT 2\n"
        ));
        assert!(output.is_empty());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("IF line-number shorthand is not supported in ANSI BASIC; use GOTO")
        );
    }
}

#[test]
fn test_minmax_functions_in_both_dialects() {
    let expected = "7\n-2.5\n-8\n-3\n4\n5\n2\n1\n312\n3\n231\n";
    assert_eq!(run_file("tests/programs/minmax.bas"), expected);
    let source = std::fs::read_to_string("tests/programs/minmax.bas").unwrap();
    assert_eq!(
        run_bas(&format!("OPTION DIALECT \"ANSI\"\n{source}")),
        expected,
    );
}

#[test]
fn test_minmax_rejects_missing_or_nonnumeric_arguments() {
    for dialect in ["QB", "ANSI"] {
        for name in ["MIN", "MAX"] {
            for arguments in ["", "\"2\"", "1, \"2\"", "VAL(\"NaN\"), \"2\""] {
                let (_, result) = run_bas_may_fail(&format!(
                    "OPTION DIALECT \"{dialect}\"\nPRINT {name}({arguments})\n"
                ));
                let message = result.unwrap_err().to_string();
                assert!(
                    message.contains(if arguments.is_empty() {
                        "wrong number of arguments"
                    } else {
                        "type mismatch"
                    }),
                    "{dialect} {name}({arguments}): {message}",
                );
            }
        }
    }
}

#[test]
fn test_doubled_quote_literals_in_both_dialects() {
    let expected = concat!(
        "\n\"\n\"\"\nHe said \"hello\".\n",
        "a\"b|\"start|end\"\n",
        "1:34\n",
        "é\"😀:3\n",
        "quote \" ' REM : remain text\n",
        "one \"quoted\" value|\"|\n",
        "OPTION DIALECT \"ANSI\"\n",
    );
    assert_eq!(run_file("tests/programs/quoted_strings.bas"), expected);
    let source = std::fs::read_to_string("tests/programs/quoted_strings.bas").unwrap();
    for dialect in ["QB", "ANSI"] {
        assert_eq!(
            run_bas(&format!(
                "OPTION DIALECT \"{dialect}\"\r\n{}",
                source.replace('\n', "\r\n")
            )),
            expected,
        );
    }
}

#[test]
fn test_unterminated_doubled_quote_literal_reports_opening_span() {
    for dialect in ["QB", "ANSI"] {
        let (_, result) = run_bas_may_fail(&format!(
            "OPTION DIALECT \"{dialect}\"\nPRINT \"unfinished\"\"\n"
        ));
        assert!(matches!(
            result.unwrap_err().downcast_ref::<rice::error::LexError>(),
            Some(rice::error::LexError::UnterminatedString { line: 2, col: 7 })
        ));
    }
}

#[test]
fn test_indexed_read_in_both_dialects() {
    let expected = "2|11|22|rice|33|44\n2|11\nfresh|55\n";
    assert_eq!(run_file("tests/programs/indexed_input.bas"), expected);
    let source = std::fs::read_to_string("tests/programs/indexed_input.bas").unwrap();
    assert_eq!(
        run_bas(&format!("OPTION DIALECT \"ANSI\"\n{source}")),
        expected
    );
}

#[test]
fn test_indexed_console_input_in_both_dialects() {
    for dialect in ["QB", "ANSI"] {
        let source = format!(
            r#"
OPTION DIALECT "{dialect}"
DIM numbers(2, 3), words$(2)
INPUT "Values"; scalar, numbers(1, 2), words$(1)
LINE INPUT "Line: "; words$(2)
LINE INPUT numbers(2, 3)
PRINT scalar; "|"; numbers(1, 2); "|"; words$(1)
PRINT "["; words$(2); "]"
PRINT numbers(2, 3)
"#
        );
        assert_eq!(
            run_bas_with_input(&source, "7, 4.5, hello\n  with, commas  \r\nconverted\n"),
            "Values? Line: 7|4.5|hello\n[  with, commas  ]\nconverted\n",
            "{dialect}"
        );
    }
}

#[test]
fn test_indexed_input_resolves_indices_once_across_retries() {
    for dialect in ["QB", "ANSI"] {
        let source = format!(
            r#"
OPTION DIALECT "{dialect}"
DIM values(2)
calls = 0
INPUT values(Pick()), values(Pick())
PRINT calls; "|"; values(1); "|"; values(2)
index = 1
INPUT index, values(index)
PRINT index; "|"; values(1); "|"; values(2)
FUNCTION Pick()
    SHARED calls
    calls = calls + 1
    Pick = calls
END FUNCTION
"#
        );
        assert_eq!(
            run_bas_with_input(&source, "123,bad\n1\n7,8\n2,99\n"),
            "? ? Redo from start\n? ? Redo from start\n? 2|7|8\n? 2|99|8\n",
            "{dialect}"
        );
    }
}

#[test]
fn test_indexed_input_rejected_line_does_not_partially_assign() {
    for dialect in ["QB", "ANSI"] {
        let (mut interpreter, output) = input_session("123,bad\n");
        let error = interpreter
            .run_source(&format!(
                r#"
OPTION DIALECT "{dialect}"
DIM values(2)
values(1) = 10
values(2) = 20
calls = 0
INPUT values(Pick()), values(Pick())
FUNCTION Pick()
    SHARED calls
    calls = calls + 1
    Pick = calls
END FUNCTION
"#
            ))
            .unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<rice::error::RuntimeError>()
                .unwrap()
                .basic_err_code(),
            62
        );
        interpreter
            .run_source("PRINT calls; \"|\"; values(1); \"|\"; values(2)\n")
            .unwrap();
        assert_eq!(
            output.into_string(),
            "? ? Redo from start\n? 2|10|20\n",
            "{dialect}"
        );
    }
}

#[test]
fn test_indexed_file_input_in_both_dialects() {
    for dialect in ["QB", "ANSI"] {
        let source = r#"
OPTION DIALECT "{DIALECT}"
DIM values(2, 2), words$(2)
OPEN "{DIR}/indexed.txt" FOR OUTPUT AS #1
WRITE #1, 2, "hello, ""rice""", 12.5
PRINT #1, "  raw, ""text""  "
CLOSE #1
OPEN "{DIR}/indexed.txt" FOR INPUT AS #1
index = 1
calls = 0
INPUT #1, index, words$(Pick()), values(index, 1)
LINE INPUT #1, words$(Pick())
PRINT index; "|"; values(1, 1); "|"; values(2, 1); "|"; words$(1)
PRINT "["; words$(2); "]|"; calls
CLOSE #1
FUNCTION Pick()
    SHARED calls
    calls = calls + 1
    Pick = calls
END FUNCTION
"#
        .replace("{DIALECT}", dialect);
        let (output, _dir) = run_bas_with_tmpdir(&source);
        assert_eq!(
            output, "2|12.5|0|hello, \"rice\"\n[  raw, \"text\"  ]|2\n",
            "{dialect}"
        );
    }
}

#[test]
fn test_indexed_input_validates_targets_before_consuming_input() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unread.txt");
    std::fs::write(&path, "40,50\n").unwrap();
    let path = path.to_str().unwrap().replace('\\', "/");
    for dialect in ["QB", "ANSI"] {
        let mut cases = vec![
            ("CONST locked = 5\n", "locked(1)", 5),
            ("DIM values(2)\nCONST values_1 = 5\n", "values(1)", 5),
            ("DIM values(2)\n", "values(1 / 0)", 11),
            ("DIM values(2)\n", "values(VAL(\"NaN\"))", 6),
            ("DIM values(2)\n", "values(\"bad\")", 13),
        ];
        if dialect == "QB" {
            cases.push(("OPTION EXPLICIT\n", "missing(1)", 5));
        }
        for (setup, target, expected_code) in cases {
            for kind in ["READ", "INPUT", "LINE INPUT", "INPUT #", "LINE INPUT #"] {
                let statement = match kind {
                    "READ" => format!("READ {target}"),
                    "INPUT" => format!("INPUT good, {target}"),
                    "LINE INPUT" => format!("LINE INPUT {target}"),
                    "INPUT #" => format!("INPUT #1, good, {target}"),
                    _ => format!("LINE INPUT #1, {target}"),
                };
                let (mut interpreter, output) = input_session("40,50\n");
                let error = interpreter
                    .run_source(&format!(
                        "OPTION DIALECT \"{dialect}\"\n{setup}DIM good\ngood = 10\nDATA 40, 50\nOPEN \"{path}\" FOR INPUT AS #1\n{statement}\n"
                    ))
                    .unwrap_err();
                assert_eq!(
                    error
                        .downcast_ref::<rice::error::RuntimeError>()
                        .unwrap()
                        .basic_err_code(),
                    expected_code,
                    "{dialect} {statement}: {error}"
                );
                assert!(
                    output.clone().into_string().is_empty(),
                    "{statement} prompted before validation"
                );
                let recovery = if kind == "READ" {
                    "DIM recovered\nREAD recovered\nPRINT recovered\n"
                } else if kind.ends_with('#') {
                    "PRINT LOC(1)\nLINE INPUT #1, remaining$\nPRINT remaining$\n"
                } else {
                    "LINE INPUT remaining$\nPRINT remaining$\n"
                };
                interpreter
                    .run_source(&format!("{recovery}PRINT good\nCLOSE #1\n"))
                    .unwrap();
                let expected = if kind == "READ" {
                    "40\n10\n"
                } else if kind.ends_with('#') {
                    "0\n40,50\n10\n"
                } else {
                    "40,50\n10\n"
                };
                assert_eq!(output.into_string(), expected, "{dialect} {statement}");
            }
        }
    }
}

#[test]
fn test_indexed_read_validates_each_target_before_its_data_item() {
    for dialect in ["QB", "ANSI"] {
        let (mut interpreter, output) = input_session("");
        let error = interpreter
            .run_source(&format!(
                "OPTION DIALECT \"{dialect}\"\nDIM values(2)\nDATA 10, 20\nREAD values(1), values(1 / 0)\n"
            ))
            .unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<rice::error::RuntimeError>()
                .unwrap()
                .basic_err_code(),
            11
        );
        interpreter
            .run_source("READ values(2)\nPRINT values(1); \"|\"; values(2)\n")
            .unwrap();
        assert_eq!(output.into_string(), "10|20\n");
    }
}

#[test]
fn test_indexed_input_eof_preserves_existing_values() {
    for dialect in ["QB", "ANSI"] {
        for statement in ["INPUT values(1), values(2)", "LINE INPUT words$(1)"] {
            let (mut interpreter, output) = input_session("");
            let error = interpreter
                .run_source(&format!(
                    "OPTION DIALECT \"{dialect}\"\nDIM values(2), words$(1)\nvalues(1) = 10\nvalues(2) = 20\nwords$(1) = \"kept\"\n{statement}\n"
                ))
                .unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<rice::error::RuntimeError>()
                    .unwrap()
                    .basic_err_code(),
                62
            );
            interpreter
                .run_source("PRINT values(1); \"|\"; values(2); \"|\"; words$(1)\n")
                .unwrap();
            let prompt = if statement.starts_with("INPUT") {
                "? "
            } else {
                ""
            };
            assert_eq!(output.into_string(), format!("{prompt}10|20|kept\n"));
        }
        for (statement, contents) in [
            ("INPUT #1, values(1), values(2)", "99"),
            ("LINE INPUT #1, words$(1)", ""),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("short.txt");
            std::fs::write(&path, contents).unwrap();
            let path = path.to_str().unwrap().replace('\\', "/");
            let (mut interpreter, output) = input_session("");
            let error = interpreter
                .run_source(&format!(
                    "OPTION DIALECT \"{dialect}\"\nDIM values(2), words$(1)\nvalues(1) = 10\nvalues(2) = 20\nwords$(1) = \"kept\"\nOPEN \"{path}\" FOR INPUT AS #1\n{statement}\n"
                ))
                .unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<rice::error::RuntimeError>()
                    .unwrap()
                    .basic_err_code(),
                62
            );
            interpreter
                .run_source("PRINT values(1); \"|\"; values(2); \"|\"; words$(1)\nCLOSE #1\n")
                .unwrap();
            assert_eq!(output.into_string(), "10|20|kept\n");
        }
    }
}

#[test]
fn test_indexed_input_preserves_string_default_rules() {
    for dialect in ["QB", "ANSI"] {
        let source = format!(
            r#"
OPTION DIALECT "{dialect}"
DIM words$(2), labels(2) AS STRING
labels(1) = ""
INPUT words$(1), labels(1)
INPUT labels(2)
PRINT words$(1); "|"; labels(1); "|"; labels(2) + 1
"#
        );
        assert_eq!(
            run_bas_with_input(&source, "alpha,beta\n7\n"),
            "? ? alpha|beta|8\n",
            "{dialect}"
        );
    }
    assert_eq!(
        run_bas_with_input(
            "DEFSTR A-A\nINPUT alphabet(1)\nPRINT alphabet(1)\n",
            "text\n"
        ),
        "? text\n"
    );
}

#[test]
fn test_indexed_input_accepts_declared_targets_with_option_explicit() {
    let source = r#"
OPTION EXPLICIT
DIM values(2), words$(2), index
index = 1
INPUT values(index), words$(index)
LINE INPUT words$(2)
DATA 9
READ values(2)
PRINT values(1); "|"; values(2); "|"; words$(1); "|"; words$(2)
"#;
    assert_eq!(
        run_bas_with_input(source, "4,hello\nwhole line\n"),
        "? 4|9|hello|whole line\n"
    );
}

#[test]
fn test_mat_file_channels_roundtrip_and_preserve_console_input() {
    let fixture = std::fs::read_to_string("tests/programs/mat_file_channels.bas").unwrap();
    for dialect in ["QB", "ANSI"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().replace('\\', "/");
        let source = format!(
            "OPTION DIALECT \"{dialect}\"\n{}",
            fixture.replace("{DIR}", &path)
        );
        let (mut interpreter, output) = input_session("console untouched\n");
        interpreter.run_source(&source).unwrap();
        interpreter
            .run_source("LINE INPUT text$\nPRINT text$\n")
            .unwrap();
        assert_eq!(
            output.into_string(),
            "11\neof\n2\n1.5 -2\n3 4\n4|5|-2|-1\nconsole untouched\n",
            "{dialect}"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("matrix.csv")).unwrap(),
            "1.5,-2\n3,4\n"
        );
    }
}

#[test]
fn test_mat_file_print_uses_current_position_and_resets_column() {
    for dialect in ["QB", "ANSI"] {
        let source = r#"
OPTION DIALECT "{DIALECT}"
DIM m(1, 2)
DATA 1, 2
MAT READ m
OPEN "{DIR}/matrix.txt" FOR OUTPUT AS #1
PRINT #1, "prefix:";
MAT PRINT #1, m
PRINT #1, TAB(5); "x"
CLOSE #1
OPEN "{DIR}/matrix.txt" FOR APPEND AS #1
MAT PRINT #1, m
CLOSE #1
"#
        .replace("{DIALECT}", dialect);
        let (output, dir) = run_bas_with_tmpdir(&source);
        assert!(output.is_empty());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("matrix.txt")).unwrap(),
            "prefix:1,2\n    x\n1,2\n"
        );
    }
}

#[test]
fn test_mat_file_input_uses_numeric_fields_independent_of_rows() {
    for dialect in ["QB", "ANSI"] {
        let source = r#"
OPTION DIALECT "{DIALECT}"
DIM m(2, 2)
OPEN "{DIR}/matrix.txt" FOR OUTPUT AS #1
WRITE #1, "1", 2, 3
WRITE #1, 4, 99
CLOSE #1
OPEN "{DIR}/matrix.txt" FOR INPUT AS #1
MAT INPUT #1, m
INPUT #1, remainder
PRINT remainder
MAT PRINT m
PRINT EOF(1)
CLOSE #1
"#
        .replace("{DIALECT}", dialect);
        let (output, _dir) = run_bas_with_tmpdir(&source);
        let eof = if dialect == "QB" { -1 } else { 1 };
        assert_eq!(output, format!("99\n1 2\n3 4\n{eof}\n"));
    }
}

#[test]
fn test_mat_file_input_errors_preserve_matrix_and_consumed_position() {
    for dialect in ["QB", "ANSI"] {
        for (contents, code, recovery) in [
            ("2,bad,4,5,99\n", 13, "INPUT #1, rest\nPRINT rest\n"),
            ("2,3,4", 62, ""),
            ("2,\"unfinished", 62, ""),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("matrix.txt");
            std::fs::write(&path, contents).unwrap();
            let path = path.to_str().unwrap().replace('\\', "/");
            let (mut interpreter, output) = input_session("");
            let error = interpreter.run_source(&format!(
                "OPTION DIALECT \"{dialect}\"\nDIM m(2, 2)\nMAT m = CON\nOPEN \"{path}\" FOR INPUT AS #1\nMAT INPUT #1, m\n"
            )).unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<rice::error::RuntimeError>()
                    .unwrap()
                    .basic_err_code(),
                code
            );
            assert!(output.clone().into_string().is_empty());
            interpreter
                .run_source(&format!("MAT PRINT m\n{recovery}CLOSE #1\n"))
                .unwrap();
            let expected = if code == 13 {
                "1 1\n1 1\n99\n"
            } else {
                "1 1\n1 1\n"
            };
            assert_eq!(output.into_string(), expected, "{dialect} {contents:?}");
        }
    }
}

#[test]
fn test_mat_file_channels_validate_handles_without_console_io() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("matrix.txt");
    std::fs::write(&path, "1,2\n").unwrap();
    let path = path.to_str().unwrap().replace('\\', "/");
    for dialect in ["QB", "ANSI"] {
        for (setup, statement, message) in [
            (String::new(), "MAT PRINT #99, m", "file #99 is not open"),
            (String::new(), "MAT INPUT #99, m", "file #99 is not open"),
            (
                format!("OPEN \"{path}\" FOR INPUT AS #1\n"),
                "MAT PRINT #1, m",
                "not open for writing",
            ),
            (
                format!("OPEN \"{path}\" FOR OUTPUT AS #1\n"),
                "MAT INPUT #1, m",
                "not open for reading",
            ),
        ] {
            let (mut interpreter, output) = input_session("console untouched\n");
            let error = interpreter
                .run_source(&format!(
                    "OPTION DIALECT \"{dialect}\"\nDIM m(1, 2)\n{setup}{statement}\n"
                ))
                .unwrap_err();
            assert!(error.to_string().contains(message), "{error}");
            assert!(output.clone().into_string().is_empty());
            interpreter
                .run_source("LINE INPUT rest$\nPRINT rest$\nCLOSE\n")
                .unwrap();
            assert_eq!(output.into_string(), "console untouched\n");
        }
    }
}

#[test]
fn test_mat_file_input_validates_destinations_before_reading() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("matrix.txt");
    std::fs::write(&path, "1,2\n").unwrap();
    let path = path.to_str().unwrap().replace('\\', "/");
    for dialect in ["QB", "ANSI"] {
        let mut setups = vec![
            "DIM m(2)\n",
            "DIM m(1, 2)\nCONST m = 5\n",
            "DIM m(1, 2)\nCONST m_1_2 = 5\n",
        ];
        if dialect == "QB" {
            setups.push("OPTION EXPLICIT\nDIM m_1_1\nm_1_1 = 1\n");
        }
        for setup in setups {
            let (mut interpreter, output) = input_session("");
            let error = interpreter.run_source(&format!(
                "OPTION DIALECT \"{dialect}\"\n{setup}OPEN \"{path}\" FOR INPUT AS #1\nMAT INPUT #1, m\n"
            )).unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<rice::error::RuntimeError>()
                    .unwrap()
                    .basic_err_code(),
                5
            );
            interpreter
                .run_source("PRINT LOC(1)\nLINE INPUT #1, rest$\nPRINT rest$\nCLOSE #1\n")
                .unwrap();
            assert_eq!(output.into_string(), "0\n1,2\n", "{dialect} {setup}");
        }
    }
}

#[test]
fn test_mat_file_channels_preserve_numeric_matrix_storage_rules() {
    for dialect in ["QB", "ANSI"] {
        let source = r#"
OPTION DIALECT "{DIALECT}"
DIM text$(1, 2)
text$(1, 1) = "not numeric"
text$(1, 2) = 5
OPEN "{DIR}/matrix.txt" FOR OUTPUT AS #1
MAT PRINT #1, text$
CLOSE #1
OPEN "{DIR}/matrix.txt" FOR INPUT AS #1
MAT INPUT #1, text$
CLOSE #1
PRINT text$(1, 1) + 1; "|"; text$(1, 2) + 1
"#
        .replace("{DIALECT}", dialect);
        let (output, dir) = run_bas_with_tmpdir(&source);
        assert_eq!(output, "1|6\n");
        assert_eq!(
            std::fs::read_to_string(dir.path().join("matrix.txt")).unwrap(),
            "0,5\n"
        );
    }
}

#[test]
fn test_command_line_arguments_are_isolated_and_persist_across_execution() {
    for dialect in [rice::Dialect::QuickBasic, rice::Dialect::Ansi] {
        let (mut interpreter, output) = input_session("");
        interpreter.dialect = dialect;
        interpreter.set_command_line_args(["", "MiXeD Case", "é😀", ""]);
        interpreter
            .run_file("tests/programs/command_line.bas")
            .unwrap();
        interpreter
            .run_source("CLEAR\nPRINT \"[\"; COMMAND$(); \"]\"\n")
            .unwrap();

        let (mut other, other_output) = input_session("");
        other
            .run_source("PRINT \"[\"; COMMAND$(); \"]\"\nPRINT \"[\"; COMMAND(); \"]\"\n")
            .unwrap();
        assert_eq!(other_output.into_string(), "[]\n[]\n");

        interpreter.set_command_line_args(["replacement"]);
        interpreter
            .run_source("PRINT command$()\nPRINT command()\n")
            .unwrap();
        interpreter.set_command_line_args(std::iter::empty::<&str>());
        interpreter
            .run_source("PRINT \"[\"; COMMAND$(); \"]\"\n")
            .unwrap();
        let truth = if dialect == rice::Dialect::QuickBasic {
            -1
        } else {
            1
        };
        assert_eq!(
            output.into_string(),
            format!(
                "[ MiXeD Case é😀 ]\n[ MiXeD Case é😀 ]\n{truth}\n[ MiXeD Case é😀 ]\nreplacement\nreplacement\n[]\n"
            )
        );
    }
}

#[test]
fn test_command_line_calls_preserve_bare_variables_and_validate_arity() {
    for dialect in [rice::Dialect::QuickBasic, rice::Dialect::Ansi] {
        let (mut interpreter, output) = input_session("");
        interpreter.dialect = dialect;
        interpreter.set_command_line_args(["payload"]);
        interpreter
            .run_source("PRINT LEN(COMMAND$)\nPRINT COMMAND\nCOMMAND$ = \"shadow\"\nPRINT COMMAND$\nPRINT COMMAND$()\nPRINT COMMAND()\n")
            .unwrap();
        assert_eq!(output.into_string(), "0\n0\nshadow\npayload\npayload\n");
        for name in ["COMMAND$", "COMMAND"] {
            let error = interpreter
                .run_source(&format!("PRINT {name}(1)\n"))
                .unwrap_err();
            assert!(matches!(
                error.downcast_ref::<rice::error::RuntimeError>(),
                Some(rice::error::RuntimeError::ArityMismatch {
                    expected: 0,
                    got: 1
                })
            ));
        }
    }
}

#[test]
fn test_multi_next_in_both_dialects() {
    let expected = "111213212223\n927\n1121:done\nskipped\n22211211\n";
    assert_eq!(run_file("tests/programs/multi_next.bas"), expected);
    let source = std::fs::read_to_string("tests/programs/multi_next.bas").unwrap();
    assert_eq!(
        run_bas(&format!("OPTION DIALECT \"ANSI\"\n{source}")),
        expected
    );
}

#[test]
fn test_multi_next_three_loops_and_procedure_scope() {
    for dialect in ["QB", "ANSI"] {
        let source = format!(
            r#"
OPTION DIALECT "{dialect}"
IF 1 THEN
    FOR i = 1 TO 2
        FOR j = 1 TO 2
            FOR k = 1 TO 2
                PRINT i * 100 + j * 10 + k; ";";
    NEXT k, j, i
ELSE
    PRINT "unreachable"
END IF
PRINT
CALL Worker
SUB Worker
    FOR i = 1 TO 2
        FOR j = 1 TO 2
            PRINT i; j;
    NEXT j, i: PRINT ":sub"
END SUB
"#
        );
        assert_eq!(
            run_bas(&source),
            "111;112;121;122;211;212;221;222;\n11122122:sub\n"
        );
    }
    assert_eq!(
        run_bas("FOR i% = 1 TO 2\nFOR j% = 1 TO 2\nPRINT i%; j%;\nNEXT J%, I%: PRINT\n"),
        "11122122\n"
    );
}

#[test]
fn test_multi_next_rejects_malformed_lists_before_execution() {
    for ending in [
        "NEXT i, j",
        "NEXT j, i, k",
        "NEXT j,",
        "NEXT j, i,",
        "NEXT j,, i",
        "NEXT j, i extra",
    ] {
        let (output, result) = run_bas_may_fail(&format!(
            "PRINT \"unreachable\"\nFOR i = 1 TO 2\nFOR j = 1 TO 2\n{ending}\n"
        ));
        assert!(
            output.is_empty(),
            "{ending} executed before reporting its syntax error"
        );
        assert!(
            result
                .unwrap_err()
                .downcast_ref::<rice::error::ParseError>()
                .is_some()
        );
    }
}

#[test]
fn test_record_len_nested_arrays_and_function_results() {
    let fixture = std::fs::read_to_string("tests/programs/record_len.bas").unwrap();
    for dialect in ["QB", "ANSI"] {
        assert_eq!(
            run_bas(&format!("OPTION DIALECT \"{dialect}\"\n{fixture}")),
            "34|5|3\n31|0\n32\n34|1\n36\n",
            "{dialect}"
        );
    }
}

#[test]
fn test_record_len_matches_binary_put_bytes() {
    let fixture = std::fs::read_to_string("tests/programs/record_len.bas").unwrap();
    let source = format!(
        "{fixture}\nOPEN \"{{DIR}}/packet.bin\" FOR BINARY AS #1\nPUT #1, 1, packet\nPRINT LOF(1); \"|\"; LEN(packet)\nCLOSE #1\n"
    );
    let (output, dir) = run_bas_with_tmpdir(&source);
    assert!(output.ends_with("36|36\n"));
    let bytes = std::fs::read(dir.path().join("packet.bin")).unwrap();
    assert_eq!(bytes.len(), 36);
    assert_eq!(&bytes[2..5], b"lon");
    assert_eq!(&bytes[29..], b"\x05\0hello");
}

#[test]
fn test_record_len_validates_serialized_values_and_arity() {
    for dialect in ["QB", "ANSI"] {
        for (assignment, expression, code) in [
            ("item.number = \"bad\"", "LEN(item)", 13),
            ("item.text = 42", "LEN(item)", 13),
            ("item.fixed = 42", "LEN(item)", 13),
            ("item.child = 42", "LEN(item)", 13),
            ("item.text = STRING$(65536, \"x\")", "LEN(item)", 5),
            ("", "LEN(42)", 13),
            ("", "LEN()", 5),
            ("", "LEN(item, item)", 5),
        ] {
            let source = format!(
                "OPTION DIALECT \"{dialect}\"\nTYPE Child\n value AS NUMERIC\nEND TYPE\nTYPE Item\n number AS NUMERIC\n text AS STRING\n fixed AS STRING * 2\n child AS Child\nEND TYPE\nDIM item AS Item\n{assignment}\nPRINT {expression}\n"
            );
            let (output, result) = run_bas_may_fail(&source);
            assert!(output.is_empty());
            assert_eq!(
                result
                    .unwrap_err()
                    .downcast_ref::<rice::error::RuntimeError>()
                    .unwrap()
                    .basic_err_code(),
                code,
                "{dialect}: {assignment}; {expression}"
            );
        }
        assert_eq!(
            run_bas(&format!(
                "OPTION DIALECT \"{dialect}\"\nTYPE TextRecord\n text AS STRING\nEND TYPE\nDIM item AS TextRecord\nitem.text = STRING$(65535, \"🙂\")\nPRINT LEN(item)\nPRINT LEN(\"é🙂\")\n"
            )),
            "65537\n2\n"
        );
    }
}

#[test]
fn test_record_len_uses_current_schema_and_declared_nested_layout() {
    for dialect in ["QB", "ANSI"] {
        let (mut interpreter, output) = input_session("");
        interpreter.run_source(&format!("OPTION DIALECT \"{dialect}\"\nTYPE InnerSmall\n value AS INTEGER\nEND TYPE\nTYPE InnerLarge\n value AS DOUBLE\nEND TYPE\nTYPE Outer\n child AS InnerSmall\nEND TYPE\nDIM item AS Outer, large AS InnerLarge\nitem.child = large\nPRINT LEN(item); \"|\"; LEN(item.child)\n")).unwrap();
        interpreter
            .run_source("TYPE InnerSmall\n value AS SINGLE\nEND TYPE\nPRINT LEN(item)\n")
            .unwrap();
        interpreter.run_source("TYPE Duplicate\n value AS INTEGER\n value AS SINGLE\nEND TYPE\nDIM duplicate AS Duplicate\nPRINT LEN(duplicate)\n").unwrap();
        assert_eq!(output.into_string(), "2|8\n4\n6\n");
        let error = interpreter
            .run_source("TYPE InnerSmall\n missing AS NUMERIC\nEND TYPE\nPRINT LEN(item)\n")
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("missing field in record: MISSING")
        );
    }
    let registry = rice::builtins::BuiltinRegistry::new();
    let record = rice::value::Value::Record {
        type_name: "Unknown".into(),
        fields: Default::default(),
    };
    assert!(matches!(
        registry.call("LEN", &[record]),
        Err(rice::error::RuntimeError::TypeMismatch { .. })
    ));
}

#[test]
fn test_matrix_determinant_named_matrices_and_cache() {
    let fixture = std::fs::read_to_string("tests/programs/matrix_determinant.bas").unwrap();
    for dialect in ["QB", "ANSI"] {
        assert_eq!(
            run_bas(&format!("OPTION DIALECT \"{dialect}\"\n{fixture}")),
            "0|-6|-6|0|0\n-6|0|-6\n0 2\n3 4\n7\n0\n5\n99|-6|-6\n",
            "{dialect}"
        );
        assert_eq!(
            run_bas(&format!(
                "OPTION DIALECT \"{dialect}\"\nDIM m(1, 1)\nm(1, 1) = 7\nm = 42\nPRINT DET(m); \"|\"; DET(m())\nFUNCTION m()\n PRINT \"unexpected\"\n m = 99\nEND FUNCTION\n"
            )),
            "7|7\n"
        );
        assert_eq!(
            run_bas(&format!(
                "OPTION DIALECT \"{dialect}\"\nDIM text$(1, 1)\ntext$(1, 1) = \"text\"\nPRINT DET(text$)\ntext$(1, 1) = 4\nPRINT DET(text$)\n"
            )),
            "0\n4\n"
        );
    }
}

#[test]
fn test_matrix_determinant_rejects_invalid_references_without_evaluation() {
    for dialect in ["QB", "ANSI"] {
        for expression in [
            "DET(unknown)",
            "DET(42)",
            "DET(\"m\")",
            "DET((m))",
            "DET(m(1, 1))",
            "DET(m(Bump()))",
            "DET(m, Bump())",
        ] {
            let (mut interpreter, output) = input_session("");
            interpreter.run_source(&format!("OPTION DIALECT \"{dialect}\"\nDIM m(1, 1)\nm(1, 1) = 1\ncalls = 0\nFUNCTION Bump()\n SHARED calls\n calls = calls + 1\n Bump = 1\nEND FUNCTION\n")).unwrap();
            let error = interpreter
                .run_source(&format!("PRINT {expression}\n"))
                .unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<rice::error::RuntimeError>()
                    .unwrap()
                    .basic_err_code(),
                5,
                "{dialect}: {expression}"
            );
            interpreter
                .run_source("PRINT calls; \"|\"; DET()\n")
                .unwrap();
            assert_eq!(output.into_string(), "0|0\n");
        }
        for declaration in ["DIM m(2)", "DIM m(2, 3)", "DIM m(2, 2, 2)"] {
            let (_, result) = run_bas_may_fail(&format!(
                "OPTION DIALECT \"{dialect}\"\n{declaration}\nPRINT DET(m)\n"
            ));
            assert!(result.is_err(), "{dialect}: {declaration}");
        }
    }
    assert_eq!(
        run_bas("OPTION EXPLICIT\nDIM m(1, 1)\nm(1, 1) = 3\nPRINT DET(m)\n"),
        "3\n"
    );
    let (_, result) = run_bas_may_fail("OPTION EXPLICIT\nDIM m_1_1\nm_1_1 = 3\nPRINT DET(m)\n");
    assert!(result.unwrap_err().to_string().contains("OPTION EXPLICIT"));
}

#[test]
fn test_data_fields_in_both_dialects() {
    let source = std::fs::read_to_string("tests/programs/data_fields.bas").unwrap();
    let expected = "0|7|0|-25|0|RICE|0\n0\n1 0\n0 -4\n";
    assert_eq!(run_file("tests/programs/data_fields.bas"), expected);
    assert_eq!(
        run_bas(&format!("OPTION DIALECT \"ANSI\"\n{source}")),
        expected
    );
}

#[test]
fn test_data_fields_use_destination_current_or_default_type() {
    for dialect in ["QB", "ANSI"] {
        let source = format!(
            r#"
OPTION DIALECT "{dialect}"
DIM named AS STRING, strings$(1), grid(1)
named = "before"
plain = "before"
dollar$ = 7
strings$(1) = 5
READ named, plain, dollar$, strings$(1), new$, grid(1)
DATA ,,,,,
PRINT LEN(named); "|"; LEN(plain); "|"; dollar$ + 1; "|"; strings$(1) + 1; "|"; LEN(new$); "|"; grid(1) + 1
RESTORE
READ index, numbers(index)
PRINT index; "|"; numbers(0)
"#
        );
        assert_eq!(run_bas(&source), "0|0|1|1|0|1\n0|0\n");
    }
    assert_eq!(
        run_bas("DEFSTR A\nREAD apple, arr(1)\nPRINT LEN(apple); LEN(arr(1))\nDATA ,\n"),
        "00\n"
    );
}

#[test]
fn test_data_fields_leave_item_unconsumed_when_destination_fails() {
    for dialect in [rice::Dialect::QuickBasic, rice::Dialect::Ansi] {
        for target in ["entry", "entries(1)", "numbers(\"bad\")", "locked"] {
            let (mut interpreter, output) = input_session("");
            interpreter.dialect = dialect;
            let source = format!(
                r#"
TYPE Item
    value AS NUMERIC
END TYPE
DIM entry AS Item, entries(1) AS Item
CONST locked = 3
DATA ,99
READ {target}
"#
            );
            assert!(interpreter.run_source(&source).is_err(), "{target}");
            interpreter
                .run_source("READ first, second\nPRINT first; \"|\"; second\n")
                .unwrap();
            assert_eq!(output.into_string(), "0|99\n", "{target}");
        }
    }
}

#[test]
fn test_data_fields_keep_earlier_assignments_when_later_target_fails() {
    let (mut interpreter, output) = input_session("");
    let error = interpreter
        .run_source(
            r#"
TYPE Item
    value AS NUMERIC
END TYPE
DIM entry AS Item
DATA +12,,+34
READ first, entry
"#,
        )
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<rice::error::RuntimeError>(),
        Some(rice::error::RuntimeError::TypeMismatch { .. })
    ));
    interpreter
        .run_source("READ second, third\nPRINT first; \"|\"; second; \"|\"; third\n")
        .unwrap();
    assert_eq!(output.into_string(), "12|0|34\n");
}

#[test]
fn test_return_targets_unwind_one_gosub() {
    assert_eq!(
        run_file("tests/programs/return_targets.bas"),
        "start\nouter\ninner\nouter-return\nmain\n"
    );
    assert_eq!(
        run_bas("10 GOSUB 100\n20 PRINT \"skipped\"\n30 PRINT \"done\"\n40 END\n100 RETURN 30\n"),
        "done\n"
    );
}

#[test]
fn test_return_targets_preserve_or_leave_active_blocks() {
    assert_eq!(
        run_bas(
            r#"
FOR i = 1 TO 3
    GOSUB work
    PRINT "skipped"
continued: PRINT i;
NEXT i
PRINT
END
work: RETURN continued
"#
        ),
        "123\n"
    );
    assert_eq!(
        run_bas(
            r#"
FOR i = 1 TO 3
    GOSUB work
    PRINT "skipped"
NEXT i
PRINT "skipped"
outside: PRINT i
END
work: RETURN outside
"#
        ),
        "1\n"
    );
}

#[test]
fn test_return_targets_use_local_procedure_labels() {
    assert_eq!(
        run_bas(
            r#"
CALL Worker
PRINT Compute()
END
SUB Worker
    GOSUB work
    PRINT "skipped"
done: PRINT "sub"
    EXIT SUB
work: RETURN done
END SUB
FUNCTION Compute()
    GOSUB work
    Compute = -1
done: Compute = 42
    EXIT FUNCTION
work: RETURN done
END FUNCTION
"#
        ),
        "sub\n42\n"
    );
}

#[test]
fn test_return_targets_report_unmatched_and_unreachable_targets() {
    for source in [
        "RETURN done\ndone: END",
        "CALL Worker\nEND\nSUB Worker\nRETURN done\ndone: EXIT SUB\nEND SUB",
        "PRINT Worker()\nEND\nFUNCTION Worker()\nRETURN done\ndone: EXIT FUNCTION\nEND FUNCTION",
        "PRINT Worker()\nEND\nFUNCTION Worker()\nRETURN\nEND FUNCTION",
    ] {
        let (_, result) = run_bas_may_fail(source);
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("RETURN without GOSUB"),
            "{source}"
        );
    }
    for source in [
        "GOSUB work\nEND\nwork: RETURN missing",
        "GOSUB work\nEND\nIF 0 THEN\nhidden: PRINT 1\nEND IF\nwork: RETURN hidden",
        "CALL Worker\nEND\noutside: PRINT 1\nSUB Worker\nGOSUB work\nEXIT SUB\nwork: RETURN outside\nEND SUB",
        "PRINT Worker()\nEND\noutside: PRINT 1\nFUNCTION Worker()\nGOSUB work\nEXIT FUNCTION\nwork: RETURN outside\nEND FUNCTION",
    ] {
        let (output, result) = run_bas_may_fail(source);
        assert!(output.is_empty(), "{source}");
        assert!(
            matches!(
                result
                    .unwrap_err()
                    .downcast_ref::<rice::error::RuntimeError>(),
                Some(rice::error::RuntimeError::UndefinedLabel { .. })
            ),
            "{source}"
        );
    }
}

#[test]
fn test_return_targets_use_an_enclosing_gosub_label_scope() {
    assert_eq!(
        run_bas(
            r#"
GOSUB outer
PRINT "main"
END
outer:
GOSUB inner
PRINT "skipped"
RETURN
inner:
RETURN hidden
IF 0 THEN
hidden: PRINT "hidden"
RETURN
END IF
"#
        ),
        "hidden\nmain\n"
    );
}

#[test]
fn test_val_scientific_prefixes_in_both_modes() {
    let source = std::fs::read_to_string("tests/programs/val_scientific.bas").unwrap();
    for dialect in ["QB", "ANSI"] {
        assert_eq!(
            run_bas(&format!("OPTION DIALECT \"{dialect}\"\n{source}")),
            "100|-250|0.05\n1.2|1|2|0\n255|63|0\n125|1\n300|1\n",
            "{dialect}"
        );
        for expression in ["VAL(42)", "VAL()", "VAL(\"1\", \"2\")"] {
            let (_, result) = run_bas_may_fail(&format!(
                "OPTION DIALECT \"{dialect}\"\nPRINT {expression}\n"
            ));
            assert!(
                result
                    .unwrap_err()
                    .downcast_ref::<rice::error::RuntimeError>()
                    .is_some()
            );
        }
    }
}

fn run_bas_with_test_environment(source: &str) -> String {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("environment.bas");
    std::fs::write(&path, source).unwrap();
    // Keep platform startup variables (including Windows SystemRoot), but make
    // this test's namespace independent of any inherited similarly named keys.
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_rice"));
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .to_uppercase()
            .starts_with("RICE_ENV_INDEX_TEST_")
        {
            command.env_remove(name);
        }
    }
    let output = command
        .arg(&path)
        .env("RICE_ENV_INDEX_TEST_Z_LAST", "last")
        .env("RICE_ENV_INDEX_TEST_A_FIRST", "first")
        .env("RICE_ENV_INDEX_TEST_M_MIDDLE", "middle=extra")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn test_environment_index_orders_host_entries_in_both_modes() {
    let source = std::fs::read_to_string("tests/programs/environment_index.bas").unwrap();
    for dialect in ["QB", "ANSI"] {
        assert_eq!(
            run_bas_with_test_environment(&format!("OPTION DIALECT \"{dialect}\"\n{source}")),
            "RICE_ENV_INDEX_TEST_A_FIRST=first\nRICE_ENV_INDEX_TEST_M_MIDDLE=middle=extra\nRICE_ENV_INDEX_TEST_Z_LAST=last\n3|[]\nRICE_ENV_INDEX_TEST_A_FIRST=first\nRICE_ENV_INDEX_TEST_M_MIDDLE=middle=extra|1\n[]\n",
            "{dialect}"
        );
    }
}

#[test]
fn test_environment_index_merges_overrides_without_mutating_host() {
    let source = r#"
ENVIRON "RICE_ENV_INDEX_TEST_M_MIDDLE=replaced=é🙂"
ENVIRON "RICE_ENV_INDEX_TEST_B_ADDED="
ENVIRON "RICE_ENV_INDEX_TEST_A_FIRST=local"
prefix$ = "RICE_ENV_INDEX_TEST_"
i = 1
count = 0
DO
    entry$ = ENVIRON$(i)
    IF entry$ = "" THEN EXIT DO
    IF LEFT$(entry$, LEN(prefix$)) = prefix$ THEN
        PRINT entry$
        count = count + 1
        IF INSTR(entry$, prefix$ & "M_MIDDLE=") = 1 THEN middleIndex = i
    END IF
    i = i + 1
LOOP
PRINT count; "|["; ENVIRON$(i); "]"
PRINT ENVIRON$(prefix$ & "A_FIRST"); "|"; ENVIRON(prefix$ & "A_FIRST")
i = 1
count = 0
DO
    entry$ = ENVIRON(i)
    IF entry$ = "" THEN EXIT DO
    IF LEFT$(entry$, LEN(prefix$)) = prefix$ THEN
        PRINT entry$
        count = count + 1
    END IF
    i = i + 1
LOOP
PRINT count; "|["; ENVIRON(i); "]"
ENVIRON "RICE_ENV_INDEX_TEST_M_MIDDLE=changed"
PRINT ENVIRON$(middleIndex)
CLEAR
PRINT ENVIRON$(FindMiddle())
FUNCTION FindMiddle()
    index = 1
    DO
        value$ = ENVIRON$(index)
        IF value$ = "" THEN EXIT DO
        IF INSTR(value$, "RICE_ENV_INDEX_TEST_M_MIDDLE=") = 1 THEN
            FindMiddle = index
            EXIT FUNCTION
        END IF
        index = index + 1
    LOOP
END FUNCTION
"#;
    assert_eq!(
        run_bas_with_test_environment(source),
        "RICE_ENV_INDEX_TEST_A_FIRST=local\nRICE_ENV_INDEX_TEST_B_ADDED=\nRICE_ENV_INDEX_TEST_M_MIDDLE=replaced=é🙂\nRICE_ENV_INDEX_TEST_Z_LAST=last\n4|[]\nlocal|first\nRICE_ENV_INDEX_TEST_A_FIRST=first\nRICE_ENV_INDEX_TEST_M_MIDDLE=middle=extra\nRICE_ENV_INDEX_TEST_Z_LAST=last\n3|[]\nRICE_ENV_INDEX_TEST_M_MIDDLE=changed\nRICE_ENV_INDEX_TEST_M_MIDDLE=changed\n"
    );
}

#[test]
fn test_environment_index_validates_arguments_in_both_modes() {
    for dialect in ["QB", "ANSI"] {
        for (expression, code) in [
            ("ENVIRON$(0)", 5),
            ("ENVIRON$(-1)", 5),
            ("ENVIRON$(.5)", 5),
            ("ENVIRON$(VAL(\"NaN\"))", 6),
            ("ENVIRON$(EXP(1000))", 6),
            ("ENVIRON$(9223372036854775808)", 6),
            ("ENVIRON$()", 5),
            ("ENVIRON$(1, 2)", 5),
        ] {
            let (_, result) = run_bas_may_fail(&format!(
                "OPTION DIALECT \"{dialect}\"\nPRINT {expression}\n"
            ));
            assert_eq!(
                result
                    .unwrap_err()
                    .downcast_ref::<rice::error::RuntimeError>()
                    .unwrap()
                    .basic_err_code(),
                code,
                "{dialect}: {expression}"
            );
        }
        let (_, result) = run_bas_may_fail(&format!(
            "OPTION DIALECT \"{dialect}\"\nTYPE Record\n x AS NUMERIC\nEND TYPE\nDIM item AS Record\nPRINT ENVIRON$(item)\n"
        ));
        assert_eq!(
            result
                .unwrap_err()
                .downcast_ref::<rice::error::RuntimeError>()
                .unwrap()
                .basic_err_code(),
            13
        );
    }
}

#[test]
fn test_function_result_types_initialize_strings_numbers_and_nested_records() {
    let fixture = std::fs::read_to_string("tests/programs/function_result_types.bas").unwrap();
    for dialect in ["QB", "ANSI"] {
        assert_eq!(
            run_bas(&format!("OPTION DIALECT \"{dialect}\"\n{fixture}")),
            "[]\n[]\n[]\n5\n12\n17\nrecord\n1\n1\npermissive\n"
        );
    }
}

#[test]
fn test_function_result_type_defaults_are_fresh_in_recursive_calls() {
    for dialect in ["QB", "ANSI"] {
        let source = format!(
            r#"
OPTION DIALECT "{dialect}"
TYPE Counter
    amount AS NUMERIC
END TYPE
PRINT MakeCounter(3).amount
PRINT MakeCounter(1).amount
FUNCTION MakeCounter(BYVAL depth) AS Counter
    MakeCounter.amount = 10
    IF depth > 0 THEN
        MakeCounter.amount = MakeCounter.amount + MakeCounter(depth - 1).amount
    END IF
END FUNCTION
"#
        );
        assert_eq!(run_bas(&source), "40\n20\n");
    }
}

#[test]
fn test_function_result_type_errors_leave_caller_scope_available() {
    for definition in [
        "FUNCTION Broken() AS Missing\nEND FUNCTION\n",
        "TYPE Recursive\nchild AS Recursive\nEND TYPE\nFUNCTION Broken() AS Recursive\nEND FUNCTION\n",
    ] {
        for dialect in ["QB", "ANSI"] {
            let output = run_bas(&format!(
                "OPTION DIALECT \"{dialect}\"\ncaller = 7\nWHEN EXCEPTION IN\nPRINT Broken()\nUSE\nPRINT caller\nEND WHEN\nPRINT caller\n{definition}"
            ));
            assert_eq!(output, "7\n7\n");
        }
    }
}

#[test]
fn test_function_without_return_as_preserves_suffix_defaults() {
    assert_eq!(
        run_bas(
            "DEFSTR A-Z\nPRINT Plain() + 1\nPRINT LEN(Text$())\nFUNCTION Plain()\nEND FUNCTION\nFUNCTION Text$()\nEND FUNCTION\n"
        ),
        "1\n0\n"
    );
}

#[test]
fn test_exit_while_nested_blocks_and_condition_evaluation() {
    let source = std::fs::read_to_string("tests/programs/exit_while.bas").unwrap();
    for dialect in ["QB", "ANSI"] {
        assert_eq!(
            run_bas(&format!("OPTION DIALECT \"{dialect}\"\n{source}")),
            "1:1:1\n2:1:1\n1\n"
        );
    }
}

#[test]
fn test_exit_select_nested_blocks_and_case_else() {
    let source = std::fs::read_to_string("tests/programs/exit_select.bas").unwrap();
    for dialect in ["QB", "ANSI"] {
        assert_eq!(
            run_bas(&format!("OPTION DIALECT \"{dialect}\"\n{source}")),
            "inner done\n1:1\n2:1\ndone\n"
        );
    }
}

#[test]
fn test_exit_while_and_select_with_local_procedures_and_exception_handlers() {
    for dialect in ["QB", "ANSI"] {
        let source = format!(
            r#"
OPTION DIALECT "{dialect}"
CALL Worker
PRINT Compute()
WHILE 1
    WHEN EXCEPTION IN
        x = 1 / 0
    USE
        EXIT WHILE
    END WHEN
    PRINT "unexpected"
WEND
SELECT CASE 1
CASE 1
    WHEN EXCEPTION IN
        x = 1 / 0
    USE
        EXIT SELECT
    END WHEN
    PRINT "unexpected"
END SELECT
PRINT "done"
SUB Worker
    WHILE 1
        EXIT WHILE
    WEND
    SELECT CASE 1
    CASE ELSE
        EXIT SELECT
    END SELECT
    PRINT "sub"
END SUB
FUNCTION Compute()
    SELECT CASE 1
    CASE ELSE
        EXIT SELECT
    END SELECT
    WHILE 1
        EXIT WHILE
    WEND
    Compute = 42
END FUNCTION
"#
        );
        assert_eq!(run_bas(&source), "sub\n42\ndone\n");
    }
}

#[test]
fn test_exit_while_and_select_do_not_escape_procedure_boundaries() {
    for dialect in ["QB", "ANSI"] {
        for (exit, open, close) in [
            ("EXIT WHILE", "WHILE 1", "WEND"),
            ("EXIT SELECT", "SELECT CASE 1\nCASE 1", "END SELECT"),
        ] {
            let sources = [
                format!("{exit}\nPRINT \"unexpected\""),
                format!("IF 1 THEN {exit} ELSE PRINT \"unexpected\""),
                format!(
                    "{open}\nCALL Worker\n{close}\nPRINT \"unexpected\"\nSUB Worker\n{exit}\nEND SUB"
                ),
                format!(
                    "{open}\nx = Compute()\n{close}\nPRINT \"unexpected\"\nFUNCTION Compute()\n{exit}\nEND FUNCTION"
                ),
            ];
            for source in sources {
                let (output, result) =
                    run_bas_may_fail(&format!("OPTION DIALECT \"{dialect}\"\n{source}"));
                assert!(output.is_empty(), "{source}: {output}");
                let error = result.unwrap_err().to_string();
                assert!(
                    error.contains("control-flow statement outside its enclosing construct"),
                    "{source}: {error}"
                );
            }
        }
    }
}

#[test]
fn test_exit_while_and_select_from_gosub_use_active_constructs() {
    for (exit, open, close) in [
        ("EXIT WHILE", "WHILE 1", "WEND"),
        ("EXIT SELECT", "SELECT CASE 1\nCASE 1", "END SELECT"),
    ] {
        let source = format!(
            "{open}\nGOSUB work\nPRINT \"unexpected\"\n{close}\nPRINT \"done\"\nEND\nwork: {exit}\n"
        );
        assert_eq!(run_bas(&source), "done\n");
    }
}

#[test]
fn test_indexed_get_put_qbasic_layouts_and_record_arrays() {
    let (output, directory) = run_bas_with_tmpdir(include_str!("programs/indexed_get_put.bas"));
    assert_eq!(output, "32\n-123;2147483000;1.25;3.5\néAB;17;xyz;hi\n");
    let bytes = std::fs::read(directory.path().join("elements.bin")).unwrap();
    assert_eq!(&bytes[..2], &(-123_i16).to_le_bytes());
    assert_eq!(&bytes[2..6], &2147483000_i32.to_le_bytes());
    assert_eq!(&bytes[6..10], &1.25_f32.to_le_bytes());
    assert_eq!(&bytes[10..18], &3.5_f64.to_le_bytes());
    assert_eq!(&bytes[18..23], &[3, 0, 233, b'A', b'B']);
}

#[test]
fn test_indexed_get_put_ansi_retains_raw_transfer_rules() {
    let (output, directory) = run_bas_with_tmpdir(
        r#"
OPTION DIALECT "ANSI"
TYPE Item
    n AS INTEGER
END TYPE
DIM words$(2), numbers(2), entries(2) AS Item
words$(1) = "éZ"
numbers(1) = 42
OPEN "{DIR}/raw.bin" FOR BINARY AS #1
PUT #1, , words$(1)
PUT #1, , numbers(1)
PUT #1, , entries(1)
PRINT LOF(1)
words$(2) = SPACE$(3)
numbers(2) = "xx"
GET #1, 1, words$(2)
GET #1, , numbers(2)
GET #1, , entries(2)
PRINT words$(2); ";"; numbers(2); ";"; entries(2)
GET #1, , words$(2)
PRINT LEN(words$(2)); ";"; EOF(1)
CLOSE #1
"#,
    );
    assert_eq!(output, "11\néZ;42;[ITEM]\n0;1\n");
    assert_eq!(
        std::fs::read(directory.path().join("raw.bin")).unwrap(),
        "éZ42[ITEM]".as_bytes()
    );
}

#[test]
fn test_indexed_get_put_defaults_and_constant_sources() {
    let (output, directory) = run_bas_with_tmpdir(
        r#"
TYPE Item
    n AS INTEGER
END TYPE
DIM records(2) AS Item, values(2), words$(2)
CONST values_1 = 42
OPEN "{DIR}/defaults.bin" FOR BINARY AS #1
OPTION EXPLICIT
PUT #1, , records(1)
PUT #1, , words$(1)
PUT #1, , values(1)
PUT #1, , values(2)
PRINT LOF(1)
GET #1, 1, records(2)
GET #1, , words$(2)
GET #1, , values(2)
PRINT records(2).n; ";"; LEN(words$(2)); ";"; values(2)
CLOSE #1
"#,
    );
    assert_eq!(output, "20\n0;0;42\n");
    let bytes = std::fs::read(directory.path().join("defaults.bin")).unwrap();
    assert_eq!(&bytes[..4], &[0, 0, 0, 0]);
    assert_eq!(&bytes[4..12], &42_f64.to_le_bytes());
    assert_eq!(&bytes[12..], &0_f64.to_le_bytes());
}

#[test]
fn test_indexed_get_put_evaluation_order_and_channel_availability() {
    let (output, _) = run_bas_with_tmpdir(
        r#"
DIM values(2, 2)
values(2, 1) = 19
trace$ = ""
FUNCTION Mark(BYVAL n)
    SHARED trace$
    trace$ = trace$ & STR$(n)
    Mark = n + 0 * LOF(1)
END FUNCTION
OPEN "{DIR}/order.bin" FOR BINARY AS #1
PUT #Mark(1), Mark(1), values(Mark(2), Mark(1))
values(2, 1) = 0
GET #Mark(1), Mark(1), values(Mark(2), Mark(1))
PRINT trace$; ";"; values(2, 1)
CLOSE #1
"#,
    );
    assert_eq!(output, "11211121;19\n");
}

#[test]
fn test_indexed_get_put_bad_targets_leave_file_cursor_and_values() {
    for operation in ["GET", "PUT"] {
        for (setup, target, error_text) in [
            ("", "values(1 / 0)", "division by zero"),
            ("", "values(\"bad\")", "type mismatch"),
            ("OPTION EXPLICIT\n", "unknown(1)", "not declared"),
            ("CONST locked = 9\n", "locked(1)", "constant"),
            ("CONST values_2 = 9\n", "values(2)", "constant"),
        ] {
            if operation == "PUT" && error_text == "constant" {
                continue;
            }
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("unchanged.bin");
            std::fs::write(&path, 23_f64.to_le_bytes()).unwrap();
            let name = path.to_str().unwrap().replace('\\', "/");
            let (mut interpreter, output) = input_session("");
            interpreter
                .run_source(&format!(
                    "DIM values(2)\nvalues(1) = 7\nOPEN \"{name}\" FOR BINARY AS #1\n{setup}"
                ))
                .unwrap();
            let error = interpreter
                .run_source(&format!("{operation} #1, 3, {target}\n"))
                .unwrap_err();
            assert!(
                error.to_string().to_lowercase().contains(error_text),
                "{operation} {target}: {error}"
            );
            interpreter
                .run_source("PRINT SEEK(1); \";\"; values(1); \";\"; LOF(1)\nCLOSE #1\n")
                .unwrap();
            assert_eq!(output.into_string(), "1;7;8\n", "{operation} {target}");
            assert_eq!(std::fs::read(path).unwrap(), 23_f64.to_le_bytes());
        }
    }
}

#[test]
fn test_indexed_get_truncated_record_keeps_destination_and_handle() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("short.bin");
    std::fs::write(&path, [1, 0, 2]).unwrap();
    let name = path.to_str().unwrap().replace('\\', "/");
    let (mut interpreter, output) = input_session("");
    interpreter
        .run_source(&format!(
            r#"
TYPE Item
    first AS INTEGER
    second AS INTEGER
END TYPE
DIM records(1) AS Item
records(1).first = 77
records(1).second = 88
OPEN "{name}" FOR INPUT AS #1
"#
        ))
        .unwrap();
    assert!(interpreter.run_source("GET #1, , records(1)\n").is_err());
    interpreter
        .run_source("PRINT records(1).first; \";\"; records(1).second; \";\"; LOC(1)\nCLOSE #1\n")
        .unwrap();
    assert_eq!(output.into_string(), "77;88;3\n");
}

#[test]
fn test_indexed_get_put_rejects_non_element_operands() {
    for operation in ["GET", "PUT"] {
        for target in [
            "values()",
            "values(1:2)",
            "record.field",
            "values(1).field",
            "values(1) + 2",
        ] {
            let (_, result) = run_bas_may_fail(&format!("{operation} #1, , {target}\n"));
            assert!(
                result
                    .unwrap_err()
                    .downcast_ref::<rice::error::ParseError>()
                    .is_some(),
                "{operation} {target}"
            );
        }
    }
}
