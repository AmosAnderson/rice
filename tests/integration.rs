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
fn test_strcomp() {
    let source = std::fs::read_to_string("tests/programs/string_compare.bas").unwrap();

    assert_eq!(run_bas(&source), "-1\n0\n1\n-1\n0\n0\n-1\n1\n1\nmatched\n",);
}

#[test]
fn test_strcomp_evaluates_arguments_once_left_to_right() {
    let source = r#"
trace$ = ""
PRINT STRCOMP(First$(), Second$(), Mode())
PRINT trace$
FUNCTION First$()
    SHARED trace$
    trace$ = trace$ & "L"
    First$ = "É"
END FUNCTION
FUNCTION Second$()
    SHARED trace$
    trace$ = trace$ & "R"
    Second$ = "é"
END FUNCTION
FUNCTION Mode()
    SHARED trace$
    trace$ = trace$ & "M"
    Mode = 1
END FUNCTION
"#;

    assert_eq!(run_bas(&source), "0\nLRM\n");
}

#[test]
fn test_strcomp_reports_invalid_mode_as_catchable_basic_error() {
    assert_eq!(
        run_bas(
            "ON ERROR GOTO handler\nPRINT STRCOMP(\"x\", \"x\", 0.5)\nEND\nhandler:\nPRINT ERR\n"
        ),
        "5\n"
    );
    for call in [
        "STRCOMP(1, \"x\")",
        "STRCOMP(\"x\", 1)",
        "STRCOMP(\"x\", \"x\", \"1\")",
        "STRCOMP()",
        "STRCOMP(\"x\")",
        "STRCOMP(\"x\", \"x\", 0, 0)",
    ] {
        assert!(
            run_bas_may_fail(&format!("PRINT {call}\n")).1.is_err(),
            "{call}"
        );
    }
}

#[test]
fn test_like_patterns() {
    let source = std::fs::read_to_string("tests/programs/like_patterns.bas").unwrap();
    let expected =
        "-1|0\n-1|0\n-1|-1|0\n-1|-1|0\n-1|-1|0\n-1|0|-1\n-1|-1|-1\n0|-1\n-1|0\ntext file\n";

    assert_eq!(run_bas(&source), expected,);
}

#[test]
fn test_like_operands_evaluate_once_left_to_right() {
    let source = r#"
trace$ = ""
PRINT TextValue$() LIKE PatternValue$()
PRINT trace$
FUNCTION TextValue$()
    SHARED trace$
    trace$ = trace$ & "L"
    TextValue$ = "file7.bas"
END FUNCTION
FUNCTION PatternValue$()
    SHARED trace$
    trace$ = trace$ & "R"
    PatternValue$ = "file#.*"
END FUNCTION
"#;
    assert_eq!(run_bas(source), "-1\nLR\n");
}

#[test]
fn test_like_pattern_and_type_errors_are_catchable() {
    for expression in [
        "\"x\" LIKE \"[]\"",
        "\"x\" LIKE \"[!]\"",
        "\"x\" LIKE \"[\"",
        "\"x\" LIKE \"[z-a]\"",
        "\"\" LIKE \"no-match*[\"",
        "1 LIKE \"*\"",
        "\"1\" LIKE 1",
    ] {
        let source = format!(
            "WHEN EXCEPTION IN\nPRINT {expression}\nUSE\nPRINT \"caught\"\nEND WHEN\nPRINT \"done\"\n"
        );
        assert_eq!(run_bas(&source), "caught\ndone\n", "{expression}");

        let expected = if expression.starts_with("1 LIKE") || expression.ends_with("LIKE 1") {
            "13\n"
        } else {
            "5\n"
        };
        assert_eq!(
            run_bas(&format!(
                "ON ERROR GOTO handler\nPRINT {expression}\nEND\nhandler:\nPRINT ERR\n"
            )),
            expected,
            "{expression}"
        );
    }
}

#[test]
fn test_indexed_mid_assignment_preserves_length_and_unicode_positions() {
    assert_eq!(
        run_file("tests/programs/indexed_mid_assign.bas"),
        "é漢Zbcd|6\né漢Zbcd\nabcXYf\nabcXZf\n[]\n"
    );
    assert_eq!(
        run_bas("MID$(missing, 1) = \"x\"\nPRINT LEN(missing)\n"),
        "0\n"
    );
}

#[test]
fn test_indexed_mid_assignment_freezes_indices_and_evaluates_arguments_once() {
    let source = r#"
DIM a$(2)
a$(1) = "abcdef": a$(2) = "second"
i = 1: trace$ = ""
MID$(a$(IndexOnce()), StartOnce(), CountOnce()) = ReplacementOnce$()
PRINT trace$; "|"; i; "|"; a$(1); "|"; a$(2)
FUNCTION IndexOnce()
    SHARED trace$, i
    trace$ = trace$ + "I"
    IndexOnce = i
END FUNCTION
FUNCTION StartOnce()
    SHARED trace$, i
    trace$ = trace$ + "S"
    i = 2
    StartOnce = 2
END FUNCTION
FUNCTION CountOnce()
    SHARED trace$
    trace$ = trace$ + "L"
    CountOnce = 2
END FUNCTION
FUNCTION ReplacementOnce$()
    SHARED trace$
    trace$ = trace$ + "R"
    ReplacementOnce$ = "XYZ"
END FUNCTION
"#;
    assert_eq!(run_bas(source), "ISRL|2|aXYdef|second\n");
}

#[test]
fn test_indexed_mid_assignment_snapshots_after_replacement_before_count() {
    let source = r#"
DIM a$(1)
a$(1) = "abcdef"
MID$(a$(1), 2, CountAndChange(a$(1))) = ReplaceAndChange$(a$(1))
PRINT a$(1)
FUNCTION ReplaceAndChange$(BYREF text$)
    text$ = "UVWXYZ"
    ReplaceAndChange$ = "12"
END FUNCTION
FUNCTION CountAndChange(BYREF text$)
    PRINT text$
    text$ = "later"
    CountAndChange = 2
END FUNCTION
"#;
    assert_eq!(run_bas(source), "UVWXYZ\nU12XYZ\n");
}

#[test]
fn test_indexed_mid_assignment_rejects_invalid_destinations_before_side_effects() {
    for declaration in ["CONST a = \"locked\"", "CONST a_1 = \"locked\""] {
        let (mut interpreter, output) = input_session("");
        interpreter.run_source(&format!(
            "{declaration}\ncalls = 0\nFUNCTION Changed()\n SHARED calls\n calls = calls + 1\n Changed = 1\nEND FUNCTION\n"
        )).unwrap();
        let error = interpreter
            .run_source("MID$(a(1), Changed()) = \"x\"\n")
            .unwrap_err();
        assert!(error.to_string().contains("constant"));
        interpreter.run_source("PRINT calls\n").unwrap();
        assert_eq!(output.into_string(), "0\n");
    }
    let (_, result) = run_bas_may_fail("OPTION EXPLICIT\nMID$(undeclared(1), 1) = \"x\"\n");
    assert!(result.unwrap_err().to_string().contains("OPTION EXPLICIT"));
    for source in [
        "DIM a(1)\nMID$(a(1), 1) = \"x\"\n",
        "TYPE Entry\n value AS STRING\nEND TYPE\nDIM a(1) AS Entry\nMID$(a(1), 1) = \"x\"\n",
    ] {
        let (_, result) = run_bas_may_fail(source);
        assert!(matches!(
            result
                .unwrap_err()
                .downcast_ref::<rice::error::RuntimeError>(),
            Some(rice::error::RuntimeError::TypeMismatch { .. })
        ));
    }
}

#[test]
fn test_indexed_mid_assignment_errors_leave_target_unchanged() {
    for statement in [
        "MID$(a$(1), 0) = \"x\"",
        "MID$(a$(1), 1, -1) = \"x\"",
        "MID$(a$(1), 1) = 42",
        "MID$(a$(1), VAL(\"NaN\")) = \"x\"",
        "MID$(a$(1), 1, EXP(1000)) = \"x\"",
        "MID$(a$(\"bad\"), 1) = \"x\"",
    ] {
        let (mut interpreter, output) = input_session("");
        interpreter
            .run_source("DIM a$(1)\na$(1) = \"before\"\n")
            .unwrap();
        assert!(interpreter.run_source(statement).is_err(), "{statement}");
        interpreter.run_source("PRINT a$(1)\n").unwrap();
        assert_eq!(output.into_string(), "before\n", "{statement}");
    }
    for source in [
        "MID$(a$(), 1) = \"x\"",
        "MID$(a$(1:2), 1) = \"x\"",
        "MID$(a(1).text, 1) = \"x\"",
        "MID$(\"literal\", 1) = \"x\"",
    ] {
        let (_, result) = run_bas_may_fail(source);
        assert!(
            result
                .unwrap_err()
                .downcast_ref::<rice::error::ParseError>()
                .is_some(),
            "{source}"
        );
    }
}

#[test]
fn test_byref_array_elements_in_subs_functions_and_records() {
    let source = std::fs::read_to_string("tests/programs/byref_elements.bas").unwrap();

    assert_eq!(run_bas(&source), "11\n130|13\nupdated\n42|0\n");
}

#[test]
fn test_byref_array_indices_are_evaluated_once_and_frozen_left_to_right() {
    let source = r#"
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
    .to_owned();
    assert_eq!(run_bas(&source), "1|2|30|20\n40|2|2|40|20\n");
}

#[test]
fn test_byref_array_copyback_aliases_defaults_and_expression_suppression() {
    let source = r#"
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
    .to_owned();
    let (parenthesized_call, result) = (7, 88);
    assert_eq!(
        run_bas(&source),
        format!("7\n7\n{parenthesized_call}\n0\nok\n{result}\n")
    );
}

#[test]
fn test_byref_array_copyback_on_errors_and_nested_calls() {
    for kind in ["SUB", "FUNCTION"] {
        let call = if kind == "SUB" {
            "CALL Change(a(1))"
        } else {
            "result = Change(a(1))"
        };
        let (mut interpreter, output) = input_session("");
        let error = interpreter
            .run_source(&format!(
                r#"
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

#[test]
fn test_byref_arrays_preserve_function_resolution_and_constants() {
    let source = r#"
DIM values(1), boxed(1)
CONST values_1 = 12
boxed(1) = 14
CONST boxed = 13
"#;

    let source = format!(
        r#"
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
fn test_indexed_swap() {
    let expected = "20;10\n20;1\n10\n[hello][]\n[second][]\n10;0\n";
    assert_eq!(run_file("tests/programs/indexed_swap.bas"), expected);
}

#[test]
fn test_indexed_swap_resolves_once_before_reading_values() {
    let source = r#"
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
    .to_owned();
    assert_eq!(run_bas(&source), "20;10;2;12\n20;40\n");
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
fn test_logical_eqv_imp_errors() {
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
fn test_default_language_behavior() {
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
fn test_string_search_and_slicing_functions() {
    let output = run_file("tests/programs/string_functions.bas");
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
    assert_eq!(lines[2].trim(), "-1"); // EOF returns numeric true.
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
    // Variable-length string records include a 2-byte length prefix.
    // After PUT of 12 bytes, position should be 13 (1-based).
    // After SET POINTER to 1, position should be 1
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines[0].trim(), "13");
    assert_eq!(lines[1].trim(), "1");
}

// ==================== BYVAL ====================

#[test]
fn test_byref_sub() {
    // Parameters are BYREF unless BYVAL is explicit.
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
    // A parenthesized call argument forces BYVAL.
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
    // Function parameters are BYREF unless BYVAL is explicit.
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
fn test_mat_extreme_indices_do_not_overflow_or_replace_values_on_failure() {
    assert_eq!(
        run_file("tests/programs/broad_matrix_index_limits.bas"),
        "0\n1\n1\n1\n1\n7\nindex overflow\n8\nextent overflow\ninitializer extent overflow\n5 0\n1 1\n"
    );
}

#[test]
fn test_failed_inverse_destination_preserves_saved_determinant() {
    assert_eq!(
        run_file("tests/programs/broad_matrix_failed_inverse.bas"),
        "2\ntype rejected\n2|0\nconstant rejected\n2|7\n3\n"
    );
}

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
fn test_language_features() {
    let output = run_file("tests/programs/language_features.bas");
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

// ==================== Procedure and control-flow features ====================

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

// ==================== Declarations and file features ====================

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
fn test_seek_function_prefers_writer_position() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
OPEN #1: NAME "{DIR}/seek-outin.dat", ORGANIZATION STREAM, ACCESS OUTIN
DIM payload AS BYTES
payload = BYTES(65, 66, 67)
PUT #1, 1, payload
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
fn test_minmax_functions() {
    let expected = "7\n-2.5\n-8\n-3\n4\n5\n2\n1\n312\n3\n231\n";
    assert_eq!(run_file("tests/programs/minmax.bas"), expected);
}

#[test]
fn test_minmax_rejects_missing_or_nonnumeric_arguments() {
    for name in ["MIN", "MAX"] {
        for arguments in ["", "\"2\"", "1, \"2\"", "VAL(\"NaN\"), \"2\""] {
            let (_, result) = run_bas_may_fail(&format!("PRINT {name}({arguments})\n"));
            let message = result.unwrap_err().to_string();
            assert!(
                message.contains(if arguments.is_empty() {
                    "wrong number of arguments"
                } else {
                    "type mismatch"
                }),
                "{name}({arguments}): {message}",
            );
        }
    }
}

#[test]
fn test_doubled_quote_literals() {
    let expected = concat!(
        "\n\"\n\"\"\nHe said \"hello\".\n",
        "a\"b|\"start|end\"\n",
        "1:34\n",
        "é\"😀:3\n",
        "quote \" ' REM : remain text\n",
        "one \"quoted\" value|\"|\n",
        "PRINT \"quoted\"\n",
    );
    assert_eq!(run_file("tests/programs/quoted_strings.bas"), expected);
    let source = std::fs::read_to_string("tests/programs/quoted_strings.bas").unwrap();

    assert_eq!(run_bas(&source.replace('\n', "\r\n")), expected,);
}

#[test]
fn test_unterminated_doubled_quote_literal_reports_opening_span() {
    let (_, result) = run_bas_may_fail("PRINT \"unfinished\"\"\n");
    assert!(matches!(
        result.unwrap_err().downcast_ref::<rice::error::LexError>(),
        Some(rice::error::LexError::UnterminatedString { line: 1, col: 7 })
    ));
}

#[test]
fn test_indexed_read() {
    let expected = "2|11|22|rice|33|44\n2|11\nfresh|55\n";
    assert_eq!(run_file("tests/programs/indexed_input.bas"), expected);
}

#[test]
fn test_indexed_console_input() {
    let source = r#"
DIM numbers(2, 3), words$(2), lines(2, 3) AS STRING
INPUT "Values"; scalar, numbers(1, 2), words$(1)
LINE INPUT "Line: "; words$(2)
LINE INPUT lines(2, 3)
PRINT scalar; "|"; numbers(1, 2); "|"; words$(1)
PRINT "["; words$(2); "]"
PRINT lines(2, 3)
"#
    .to_owned();
    assert_eq!(
        run_bas_with_input(&source, "7, 4.5, hello\n  with, commas  \r\nconverted\n"),
        "Values? Line: 7|4.5|hello\n[  with, commas  ]\nconverted\n",
    );
}

#[test]
fn test_indexed_input_resolves_indices_once_across_retries() {
    let source = r#"
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
    .to_owned();
    assert_eq!(
        run_bas_with_input(&source, "123,bad\n1\n7,8\n2,99\n"),
        "? ? Redo from start\n? ? Redo from start\n? 2|7|8\n? 2|99|8\n",
    );
}

#[test]
fn test_indexed_input_rejected_line_does_not_partially_assign() {
    let (mut interpreter, output) = input_session("123,bad\n");
    let error = interpreter
        .run_source(
            r#"
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
"#,
        )
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
    assert_eq!(output.into_string(), "? ? Redo from start\n? 2|10|20\n",);
}

#[test]
fn test_indexed_file_input() {
    let source = r#"
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
"#;
    let (output, _dir) = run_bas_with_tmpdir(&source);
    assert_eq!(output, "2|12.5|0|hello, \"rice\"\n[  raw, \"text\"  ]|2\n",);
}

#[test]
fn test_indexed_input_validates_targets_before_consuming_input() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unread.txt");
    std::fs::write(&path, "40,50\n").unwrap();
    let path = path.to_str().unwrap().replace('\\', "/");

    let mut cases = vec![
        ("CONST locked = 5\n", "locked(1)", 5),
        ("DIM values(2)\nCONST values_1 = 5\n", "values(1)", 5),
        ("DIM values(2)\n", "values(1 / 0)", 11),
        ("DIM values(2)\n", "values(VAL(\"NaN\"))", 6),
        ("DIM values(2)\n", "values(\"bad\")", 13),
    ];

    cases.push(("OPTION EXPLICIT\n", "missing(1)", 5));

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
                        "{setup}DIM good\ngood = 10\nDATA 40, 50\nOPEN \"{path}\" FOR INPUT AS #1\n{statement}\n"
                    ))
                    .unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<rice::error::RuntimeError>()
                    .unwrap()
                    .basic_err_code(),
                expected_code,
                "{statement}: {error}"
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
            assert_eq!(output.into_string(), expected, "{statement}");
        }
    }
}

#[test]
fn test_indexed_read_validates_each_target_before_its_data_item() {
    let (mut interpreter, output) = input_session("");
    let error = interpreter
        .run_source("DIM values(2)\nDATA 10, 20\nREAD values(1), values(1 / 0)\n")
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

#[test]
fn test_indexed_input_eof_preserves_existing_values() {
    for statement in ["INPUT values(1), values(2)", "LINE INPUT words$(1)"] {
        let (mut interpreter, output) = input_session("");
        let error = interpreter
                .run_source(&format!(
                    "DIM values(2), words$(1)\nvalues(1) = 10\nvalues(2) = 20\nwords$(1) = \"kept\"\n{statement}\n"
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
                    "DIM values(2), words$(1)\nvalues(1) = 10\nvalues(2) = 20\nwords$(1) = \"kept\"\nOPEN \"{path}\" FOR INPUT AS #1\n{statement}\n"
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

#[test]
fn test_indexed_input_uses_declared_string_defaults() {
    let source = r#"
DIM words$(2), labels(2) AS STRING
labels(1) = ""
INPUT words$(1), labels(1)
INPUT labels(2)
PRINT words$(1); "|"; labels(1); "|"; labels(2)
"#
    .to_owned();
    assert_eq!(
        run_bas_with_input(&source, "alpha,beta\ngamma\n"),
        "? ? alpha|beta|gamma\n",
    );

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

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_str().unwrap().replace('\\', "/");
    let source = fixture.replace("{DIR}", &path);
    let (mut interpreter, output) = input_session("console untouched\n");
    interpreter.run_source(&source).unwrap();
    interpreter
        .run_source("LINE INPUT text$\nPRINT text$\n")
        .unwrap();
    assert_eq!(
        output.into_string(),
        "11\neof\n2\n1.5 -2\n3 4\n4|5|-2|-1\nconsole untouched\n",
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("matrix.csv")).unwrap(),
        "1.5,-2\n3,4\n"
    );
}

#[test]
fn test_mat_file_print_uses_current_position_and_resets_column() {
    let source = r#"
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
"#;
    let (output, dir) = run_bas_with_tmpdir(&source);
    assert!(output.is_empty());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("matrix.txt")).unwrap(),
        "prefix:1,2\n    x\n1,2\n"
    );
}

#[test]
fn test_mat_file_input_uses_numeric_fields_independent_of_rows() {
    let source = r#"
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
"#;
    let (output, _dir) = run_bas_with_tmpdir(&source);
    let eof = -1;
    assert_eq!(output, format!("99\n1 2\n3 4\n{eof}\n"));
}

#[test]
fn test_mat_file_input_errors_preserve_matrix_and_consumed_position() {
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
        let error = interpreter
            .run_source(&format!(
                "DIM m(2, 2)\nMAT m = CON\nOPEN \"{path}\" FOR INPUT AS #1\nMAT INPUT #1, m\n"
            ))
            .unwrap_err();
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
        assert_eq!(output.into_string(), expected, "{contents:?}");
    }
}

#[test]
fn test_mat_file_channels_validate_handles_without_console_io() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("matrix.txt");
    std::fs::write(&path, "1,2\n").unwrap();
    let path = path.to_str().unwrap().replace('\\', "/");

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
            .run_source(&format!("DIM m(1, 2)\n{setup}{statement}\n"))
            .unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
        assert!(output.clone().into_string().is_empty());
        interpreter
            .run_source("LINE INPUT rest$\nPRINT rest$\nCLOSE\n")
            .unwrap();
        assert_eq!(output.into_string(), "console untouched\n");
    }
}

#[test]
fn test_mat_file_input_validates_destinations_before_reading() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("matrix.txt");
    std::fs::write(&path, "1,2\n").unwrap();
    let path = path.to_str().unwrap().replace('\\', "/");

    let mut setups = vec![
        "DIM m(2)\n",
        "DIM m(1, 2)\nCONST m = 5\n",
        "DIM m(1, 2)\nCONST m_1_2 = 5\n",
    ];

    setups.push("OPTION EXPLICIT\nDIM m_1_1\nm_1_1 = 1\n");

    for setup in setups {
        let (mut interpreter, output) = input_session("");
        let error = interpreter
            .run_source(&format!(
                "{setup}OPEN \"{path}\" FOR INPUT AS #1\nMAT INPUT #1, m\n"
            ))
            .unwrap_err();
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
        assert_eq!(output.into_string(), "0\n1,2\n", "{setup}");
    }
}

#[test]
fn test_mat_file_channels_preserve_numeric_matrix_storage_rules() {
    let source = r#"
DIM values(1, 2) AS SINGLE, received(1, 2) AS INTEGER
values(1, 1) = 0
values(1, 2) = 5
OPEN "{DIR}/matrix.txt" FOR OUTPUT AS #1
MAT PRINT #1, values
CLOSE #1
OPEN "{DIR}/matrix.txt" FOR INPUT AS #1
MAT INPUT #1, received
CLOSE #1
PRINT received(1, 1) + 1; "|"; received(1, 2) + 1; "|"; TYPEOF$(received(1, 2))
"#;
    let (output, dir) = run_bas_with_tmpdir(source);
    assert_eq!(output, "1|6|INTEGER\n");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("matrix.txt")).unwrap(),
        "0,5\n"
    );
}

#[test]
fn test_command_line_arguments_are_isolated_and_persist_across_execution() {
    let (mut interpreter, output) = input_session("");
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
    let truth = -1;
    assert_eq!(
        output.into_string(),
        format!(
            "[ MiXeD Case é😀 ]\n[ MiXeD Case é😀 ]\n{truth}\n[ MiXeD Case é😀 ]\nreplacement\nreplacement\n[]\n"
        )
    );
}

#[test]
fn test_command_line_calls_preserve_bare_variables_and_validate_arity() {
    let (mut interpreter, output) = input_session("");
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

#[test]
fn test_multi_next() {
    let expected = "111213212223\n927\n1121:done\nskipped\n22211211\n";
    assert_eq!(run_file("tests/programs/multi_next.bas"), expected);
}

#[test]
fn test_multi_next_three_loops_and_procedure_scope() {
    let source = r#"
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
    .to_owned();
    assert_eq!(
        run_bas(&source),
        "111;112;121;122;211;212;221;222;\n11122122:sub\n"
    );

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

    assert_eq!(run_bas(&fixture), "34|5|3\n31|0\n32\n34|1\n36\n",);
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
            "TYPE Child\n value AS NUMERIC\nEND TYPE\nTYPE Item\n number AS NUMERIC\n text AS STRING\n fixed AS STRING * 2\n child AS Child\nEND TYPE\nDIM item AS Item\n{assignment}\nPRINT {expression}\n"
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
            "{assignment}; {expression}"
        );
    }
    assert_eq!(
        run_bas(
            "TYPE TextRecord\n text AS STRING\nEND TYPE\nDIM item AS TextRecord\nitem.text = STRING$(65535, \"🙂\")\nPRINT LEN(item)\nPRINT LEN(\"é🙂\")\n"
        ),
        "65537\n2\n"
    );
}

#[test]
fn test_record_len_uses_current_schema_and_declared_nested_layout() {
    let (mut interpreter, output) = input_session("");
    interpreter.run_source("TYPE InnerSmall\n value AS INTEGER\nEND TYPE\nTYPE InnerLarge\n value AS DOUBLE\nEND TYPE\nTYPE Outer\n child AS InnerSmall\nEND TYPE\nDIM item AS Outer, large AS InnerLarge\nWHEN EXCEPTION IN\nitem.child = large\nUSE\nPRINT \"rejected\"\nEND WHEN\nPRINT LEN(item); \"|\"; LEN(item.child)\n").unwrap();
    interpreter
        .run_source("TYPE InnerSmall\n value AS SINGLE\nEND TYPE\nPRINT LEN(item)\n")
        .unwrap();
    interpreter.run_source("TYPE Duplicate\n value AS INTEGER\n value AS SINGLE\nEND TYPE\nDIM duplicate AS Duplicate\nPRINT LEN(duplicate)\n").unwrap();
    assert_eq!(output.into_string(), "rejected\n2|2\n4\n6\n");
    let error = interpreter
        .run_source("TYPE InnerSmall\n missing AS NUMERIC\nEND TYPE\nPRINT LEN(item)\n")
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("missing field in record: MISSING")
    );

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

    assert_eq!(
        run_bas(&fixture),
        "0|-6|-6|0|0\n-6|0|-6\n0 2\n3 4\n7\n0\n5\n99|-6|-6\n",
    );
    assert_eq!(
        run_bas(
            "DIM m(1, 1)\nm(1, 1) = 7\nm = 42\nPRINT DET(m); \"|\"; DET(m())\nFUNCTION m()\n PRINT \"unexpected\"\n m = 99\nEND FUNCTION\n"
        ),
        "7|7\n"
    );
    assert_eq!(
        run_bas(
            "DIM text$(1, 1)\ntext$(1, 1) = \"text\"\nPRINT DET(text$)\nWHEN EXCEPTION IN\ntext$(1, 1) = 4\nUSE\nPRINT \"assignment rejected\"\nEND WHEN\nPRINT DET(text$)\n"
        ),
        "0\nassignment rejected\n0\n"
    );
}

#[test]
fn test_matrix_determinant_rejects_invalid_references_without_evaluation() {
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
        interpreter.run_source("DIM m(1, 1)\nm(1, 1) = 1\ncalls = 0\nFUNCTION Bump()\n SHARED calls\n calls = calls + 1\n Bump = 1\nEND FUNCTION\n").unwrap();
        let error = interpreter
            .run_source(&format!("PRINT {expression}\n"))
            .unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<rice::error::RuntimeError>()
                .unwrap()
                .basic_err_code(),
            5,
            "{expression}"
        );
        interpreter
            .run_source("PRINT calls; \"|\"; DET()\n")
            .unwrap();
        assert_eq!(output.into_string(), "0|0\n");
    }
    for declaration in ["DIM m(2)", "DIM m(2, 3)", "DIM m(2, 2, 2)"] {
        let (_, result) = run_bas_may_fail(&format!("{declaration}\nPRINT DET(m)\n"));
        assert!(result.is_err(), "{declaration}");
    }

    assert_eq!(
        run_bas("OPTION EXPLICIT\nDIM m(1, 1)\nm(1, 1) = 3\nPRINT DET(m)\n"),
        "3\n"
    );
    let (_, result) = run_bas_may_fail("OPTION EXPLICIT\nDIM m_1_1\nm_1_1 = 3\nPRINT DET(m)\n");
    assert!(result.unwrap_err().to_string().contains("OPTION EXPLICIT"));
}

#[test]
fn test_data_fields() {
    let expected = "0|7|0|-25|0|RICE|0\n0\n1 0\n0 -4\n";
    assert_eq!(run_file("tests/programs/data_fields.bas"), expected);
}

#[test]
fn test_data_fields_use_destination_current_or_default_type() {
    let source = r#"
DIM named AS STRING, strings$(1), grid(1)
named = "before"
plain = "before"
dollar$ = "before"
strings$(1) = "5"
READ named, plain, dollar$, strings$(1), new$, grid(1)
DATA ,,,,,
PRINT LEN(named); "|"; LEN(plain); "|"; LEN(dollar$); "|"; LEN(strings$(1)); "|"; LEN(new$); "|"; grid(1) + 1
RESTORE
READ index, numbers(index)
PRINT index; "|"; numbers(0)
"#.to_owned();
    assert_eq!(run_bas(&source), "0|0|0|0|0|1\n0|0\n");

    assert_eq!(
        run_bas("DEFSTR A\nREAD apple, arr(1)\nPRINT LEN(apple); LEN(arr(1))\nDATA ,\n"),
        "00\n"
    );
}

#[test]
fn test_data_fields_leave_item_unconsumed_when_destination_fails() {
    for target in ["entry", "entries(1)", "numbers(\"bad\")", "locked"] {
        let (mut interpreter, output) = input_session("");
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
fn test_val_scientific_prefixes() {
    let source = std::fs::read_to_string("tests/programs/val_scientific.bas").unwrap();

    assert_eq!(
        run_bas(&source),
        "100|-250|0.05\n1.2|1|2|0\n255|63|0\n125|1\n300|1\n",
    );
    for expression in ["VAL(42)", "VAL()", "VAL(\"1\", \"2\")"] {
        let (_, result) = run_bas_may_fail(&format!("PRINT {expression}\n"));
        assert!(
            result
                .unwrap_err()
                .downcast_ref::<rice::error::RuntimeError>()
                .is_some()
        );
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
fn test_environment_index_orders_host_entries() {
    let source = std::fs::read_to_string("tests/programs/environment_index.bas").unwrap();

    assert_eq!(
        run_bas_with_test_environment(&source),
        "RICE_ENV_INDEX_TEST_A_FIRST=first\nRICE_ENV_INDEX_TEST_M_MIDDLE=middle=extra\nRICE_ENV_INDEX_TEST_Z_LAST=last\n3|[]\nRICE_ENV_INDEX_TEST_A_FIRST=first\nRICE_ENV_INDEX_TEST_M_MIDDLE=middle=extra|1\n[]\n",
    );
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
fn test_environment_index_validates_arguments() {
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
        let (_, result) = run_bas_may_fail(&format!("PRINT {expression}\n"));
        assert_eq!(
            result
                .unwrap_err()
                .downcast_ref::<rice::error::RuntimeError>()
                .unwrap()
                .basic_err_code(),
            code,
            "{expression}"
        );
    }
    let (_, result) = run_bas_may_fail(
        "TYPE Record\n x AS NUMERIC\nEND TYPE\nDIM item AS Record\nPRINT ENVIRON$(item)\n",
    );
    assert_eq!(
        result
            .unwrap_err()
            .downcast_ref::<rice::error::RuntimeError>()
            .unwrap()
            .basic_err_code(),
        13
    );
}

#[test]
fn test_function_result_types_initialize_strings_numbers_and_nested_records() {
    let fixture = std::fs::read_to_string("tests/programs/function_result_types.bas").unwrap();

    assert_eq!(
        run_bas(&fixture),
        "[]\n[]\n[    ]\n5\n12\n17\nrecord\n1\n1\n42\n"
    );
}

#[test]
fn test_function_result_type_defaults_are_fresh_in_recursive_calls() {
    let source = r#"
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
    .to_owned();
    assert_eq!(run_bas(&source), "40\n20\n");
}

#[test]
fn test_function_result_type_errors_leave_caller_scope_available() {
    for definition in [
        "FUNCTION Broken() AS Missing\nEND FUNCTION\n",
        "TYPE Recursive\nchild AS Recursive\nEND TYPE\nFUNCTION Broken() AS Recursive\nEND FUNCTION\n",
    ] {
        let output = run_bas(&format!(
            "caller = 7\nWHEN EXCEPTION IN\nPRINT Broken()\nUSE\nPRINT caller\nEND WHEN\nPRINT caller\n{definition}"
        ));
        assert_eq!(output, "7\n7\n");
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

    assert_eq!(run_bas(&source), "1:1:1\n2:1:1\n1\n");
}

#[test]
fn test_exit_select_nested_blocks_and_case_else() {
    let source = std::fs::read_to_string("tests/programs/exit_select.bas").unwrap();

    assert_eq!(run_bas(&source), "inner done\n1:1\n2:1\ndone\n");
}

#[test]
fn test_exit_while_and_select_with_local_procedures_and_exception_handlers() {
    let source = r#"
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
    .to_owned();
    assert_eq!(run_bas(&source), "sub\n42\ndone\n");
}

#[test]
fn test_exit_while_and_select_do_not_escape_procedure_boundaries() {
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
            let (output, result) = run_bas_may_fail(&source);
            assert!(output.is_empty(), "{source}: {output}");
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains("control-flow statement outside its enclosing construct"),
                "{source}: {error}"
            );
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
fn test_indexed_get_put_layouts_and_record_arrays() {
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

#[test]
fn test_const_lists_and_procedure_scopes() {
    let fixture = std::fs::read_to_string("tests/programs/const_lists.bas").unwrap();

    let explicit = "OPTION EXPLICIT\n";
    assert_eq!(
        run_bas(&format!("{explicit}{fixture}")),
        "3:6:6:ok!\n7:8\n16:17\n"
    );
}

#[test]
fn test_const_lists_evaluate_left_to_right_once() {
    let source = r#"
calls = 0
CONST first = Bump(), second = Bump() + first, third = Bump() + second
PRINT first; ":"; second; ":"; third; ":"; calls
FUNCTION Bump()
    SHARED calls
    calls = calls + 1
    Bump = calls
END FUNCTION
"#
    .to_owned();
    assert_eq!(run_bas(&source), "1:3:6:3\n");
}

#[test]
fn test_const_lists_keep_prior_definitions_and_initializer_effects_on_failure() {
    let explicit = "OPTION EXPLICIT\n";
    for (declaration, expected_calls, duplicate) in [
        (
            "CONST first = Bump(), first = Bump(), later = Bump()",
            2,
            true,
        ),
        (
            "CONST first = Bump(), failed = Bump() / 0, later = Bump()",
            2,
            false,
        ),
    ] {
        let (mut interpreter, output) = input_session("");
        interpreter.run_source(&format!(
                "{explicit}DIM calls\nFUNCTION Bump()\nSHARED calls\ncalls = calls + 1\nBump = calls\nEND FUNCTION\n"
            )).unwrap();
        let error = interpreter.run_source(declaration).unwrap_err();
        let runtime_error = error.downcast_ref::<rice::error::RuntimeError>().unwrap();
        if duplicate {
            assert!(matches!(
                runtime_error,
                rice::error::RuntimeError::DuplicateDefinition { .. }
            ));
        } else {
            assert!(matches!(
                runtime_error,
                rice::error::RuntimeError::DivisionByZero
            ));
        }
        interpreter
            .run_source("CONST later = 99\nPRINT first; \":\"; calls; \":\"; later\n")
            .unwrap();
        assert_eq!(output.into_string(), format!("1:{expected_calls}:99\n"));
        let error = interpreter.run_source("first = 9").unwrap_err();
        assert!(error.to_string().contains("constant"));
    }
}

#[test]
fn test_const_lists_are_one_statement_for_handler_continue() {
    let source = r#"
WHEN EXCEPTION IN
    CONST first = 7, failed = 1 / 0, skipped = 99
    PRINT first
USE
    PRINT "caught"
    CONTINUE
END WHEN
CONST skipped = 42
PRINT skipped
"#
    .to_owned();
    assert_eq!(run_bas(&source), "caught\n7\n42\n");
}

#[test]
fn test_declared_array_defaults_fixture() {
    let source = std::fs::read_to_string("tests/programs/declared_array_defaults.bas").unwrap();

    let explicit = "OPTION EXPLICIT\n";
    assert_eq!(
        run_bas(&format!("{explicit}{source}")),
        "0|2|0\n0|0|0|0\nhello|2|0\nrejected\n0|42\n0|2|0\n"
    );
}

#[test]
fn test_declared_array_defaults_redeclaration_and_lifetime() {
    assert_eq!(
        run_bas(
            r#"
DIM changing(3) AS INTEGER
changing(1) = 7
WHEN EXCEPTION IN
    DIM changing(3) AS STRING
USE
    PRINT "dim rejected"
END WHEN
PRINT changing(1); "|"; TYPEOF$(changing(2))
WHEN EXCEPTION IN
    REDIM PRESERVE changing(4) AS LONG
USE
    PRINT "preserve rejected"
END WHEN
PRINT changing(1); "|"; UBOUND(changing)
REDIM changing(2) AS STRING * 2
changing(1) = "long"
PRINT changing(1); "|"; LEN(changing(2))
ERASE changing
PRINT "["; changing(1); "]"
CLEAR
PRINT LEN(changing(1)); "|"; UBOUND(changing)
"#
        ),
        "dim rejected\n7|INTEGER\npreserve rejected\n7|3\nlo|2\n[  ]\n2|2\n"
    );
}

#[test]
fn test_declared_array_types_are_independent_of_later_def_types() {
    assert_eq!(
        run_bas(
            r#"
DIM apple(2), apricot(2) AS NUMERIC
DEFSTR A
PRINT TYPEOF$(apple(1)); "|"; TYPEOF$(apricot(1))
newName = 7
another = "text"
PRINT TYPEOF$(another)
DIM apple(2) AS STRING
PRINT LEN(apple(1))
DEFINT A
PRINT TYPEOF$(apple(1)); "|"; TYPEOF$(another)
"#
        ),
        "NUMERIC|NUMERIC\nSTRING\n0\nSTRING|STRING\n"
    );
}

#[test]
fn test_declared_array_defaults_apply_to_file_input() {
    let source = format!(
        r#"
OPEN "{{DIR}}/input.txt" FOR OUTPUT AS #1
PRINT #1, "alpha,5"
PRINT #1, "longer than one"
CLOSE #1
DIM labels(1) AS STRING, numbers$(1) AS NUMERIC, fixed(1) AS STRING * 1
OPEN "{{DIR}}/input.txt" FOR INPUT AS #1
INPUT #1, labels(1), numbers$(1)
LINE INPUT #1, fixed(1)
CLOSE #1
PRINT labels(1); "|"; numbers$(1) + 1; "|"; fixed(1)
"#
    );
    let (output, _) = run_bas_with_tmpdir(&source);
    assert_eq!(output, "alpha|6|l\n");
}

#[test]
fn test_declared_array_types_select_binary_widths_before_suffixes() {
    let source = r#"
DIM overridden%(2) AS DOUBLE, unsuffixed(2) AS INTEGER
DIM text$(2) AS STRING * 2
overridden%(1) = 123
unsuffixed(1) = 4
text$(1) = "longer"
OPEN "{DIR}/array.bin" FOR BINARY AS #1
PUT #1, , overridden%(1)
PUT #1, , unsuffixed(1)
PUT #1, , text$(1)
PRINT LOF(1)
GET #1, 1, overridden%(2)
GET #1, 9, unsuffixed(2)
GET #1, 11, text$(2)
PRINT overridden%(2); "|"; unsuffixed(2); "|"; text$(2)
CLOSE #1
"#;
    let (output, dir) = run_bas_with_tmpdir(source);
    assert_eq!(output, "12\n123|4|lo\n");
    let mut expected = 123_f64.to_le_bytes().to_vec();
    expected.extend_from_slice(&4_i16.to_le_bytes());
    expected.extend_from_slice(b"lo");
    assert_eq!(
        std::fs::read(dir.path().join("array.bin")).unwrap(),
        expected
    );
}

#[test]
fn test_declared_array_defaults_persist_across_submissions_and_existing_scope_rules() {
    let (mut interpreter, output) = input_session("");
    interpreter.run_source("DIM values(2) AS STRING\n").unwrap();
    interpreter
        .run_source("PRINT LEN(values(1))\nCLEAR\n")
        .unwrap();
    interpreter.run_source("PRINT LEN(values(1))\n").unwrap();
    interpreter.run_source(
        "CALL SetDefaults\nPRINT values(2)\nSUB SetDefaults\n DIM values(2) AS NUMERIC\nEND SUB\n"
    ).unwrap();
    assert_eq!(output.into_string(), "0\n0\n\n");
}

#[test]
fn test_declared_array_defaults_survive_failed_dimension_changes() {
    for operation in ["DIM", "REDIM", "REDIM PRESERVE"] {
        let (mut interpreter, output) = input_session("");
        interpreter.run_source("DIM text(2) AS STRING\n").unwrap();
        assert!(
            interpreter
                .run_source(&format!("{operation} text(2 TO 1) AS NUMERIC\n"))
                .is_err()
        );
        interpreter
            .run_source("PRINT LEN(text(1)); \"|\"; UBOUND(text)\n")
            .unwrap();
        assert_eq!(output.into_string(), "0|2\n");
    }
}

#[test]
fn test_replace_strings() {
    let source = std::fs::read_to_string("tests/programs/replace_strings.bas").unwrap();

    assert_eq!(
        run_bas(&source),
        "baXYZXYZ\nbba\nX abc\nbc\nbc\néé\n🙂X🙂\naabaa\n[]\n[]\nuntouched|unchanged\n",
    );
}

#[test]
fn test_reverse_search() {
    let source = std::fs::read_to_string("tests/programs/reverse_search.bas").unwrap();

    assert_eq!(run_bas(&source), "4|2\n4|2\n1\n3|2|0\n0|0\n1|3\nfile.bas\n",);
}

#[test]
fn test_negative_decimal_places() {
    let source = std::fs::read_to_string("tests/programs/negative_decimal_places.bas").unwrap();

    assert_eq!(
        run_bas(&source),
        "150|200|-200\n1400|-1400\n150|150\n1.3|1.2\n1\n1000\n",
    );
}

#[test]
fn test_string_transform_and_rounding_arguments_evaluate_once_left_to_right() {
    let source = r#"
trace$ = ""
PRINT REPLACE$(Text$("1", "éaba"), Text$("2", "a"), Text$("3", "X"), Number("4", 2), Number("5", 1))
PRINT trace$
trace$ = ""
PRINT INSTRREV(Text$("1", "banana"), Text$("2", "ana"), Number("3", 5))
PRINT trace$
trace$ = ""
PRINT ROUND(Number("1", 125), Number("2", -1))
PRINT trace$
trace$ = ""
PRINT TRUNCATE(Number("1", -125), Number("2", -1))
PRINT trace$
FUNCTION Text$(BYVAL tag$, BYVAL result$)
    SHARED trace$
    trace$ = trace$ & tag$
    Text$ = result$
END FUNCTION
FUNCTION Number(BYVAL tag$, BYVAL result)
    SHARED trace$
    trace$ = trace$ & tag$
    Number = result
END FUNCTION
"#;

    assert_eq!(run_bas(&source), "Xba\n12345\n2\n123\n130\n12\n-120\n12\n",);
}

#[test]
fn test_new_string_operations_and_negative_rounding_report_runtime_errors() {
    for (expression, expected_code) in [
        ("REPLACE$(\"\", \"\", \"x\", 0)", 5),
        ("REPLACE$(\"\", \"\", \"x\", 1, -2)", 5),
        ("REPLACE$(\"\", \"\", 3, 1, 0)", 13),
        ("REPLACE$(\"\", \"\", \"x\", 1, VAL(\"NaN\"))", 6),
        ("REPLACE$()", 5),
        ("INSTRREV(\"\", \"\", 0)", 5),
        ("INSTRREV(\"\", 1)", 13),
        ("INSTRREV(\"\", \"\", VAL(\"NaN\"))", 6),
        ("INSTRREV(\"a\", \"a\", 1, 0)", 5),
        ("ROUND(1, -309)", 5),
        ("TRUNCATE(1, -309)", 5),
        ("ROUND(1, \"-1\")", 13),
        ("ROUND(MAXNUM(), -308)", 6),
    ] {
        let (_, result) = run_bas_may_fail(&format!("PRINT {expression}\n"));
        let error = result.expect_err(expression);
        let runtime = error.downcast_ref::<rice::error::RuntimeError>().unwrap();
        assert_eq!(runtime.basic_err_code(), expected_code, "{expression}");
    }
}

#[test]
fn test_native_types() {
    let source = std::fs::read_to_string("tests/programs/native_types.bas").unwrap();
    let expected = concat!(
        "INT64|BOOL|BYTE|BYTES\n0|FALSE|0|0\n",
        "9007199254740995\n9007199254740991\n9007199254740994|9007199254740992\n9000000003\n",
        "1\nTRUE|FALSE\nTRUE\n20000000000001|9007199254740993\n",
        "TRUE|FALSE|TRUE\nBOOL|1|0\n255|BYTE\n",
        "007F80FF|255|4\n007F80FF|002180FF\n80FF|0\nRice 🍚\nAB00FF\n00FF|TRUE\n",
    );

    assert_eq!(run_bas(&source), expected,);
}

#[test]
fn test_native_integer_division_and_overflow_are_exact() {
    assert_eq!(
        run_bas(
            "PRINT CINT64(\"9007199254740993\") \\ CINT64(2)\nPRINT CINT64(\"-9007199254740993\") \\ CINT64(2)\n"
        ),
        "4503599627370496\n-4503599627370496\n"
    );
    for (expression, code) in [
        ("CINT64(\"-9223372036854775808\") \\ CINT64(-1)", 6),
        ("CINT64(1) \\ CINT64(0)", 11),
        ("CINT64(\"9223372036854775807\") + 1", 6),
        ("CINT64(\"-9223372036854775808\") - 1", 6),
    ] {
        let (_, result) = run_bas_may_fail(&format!("PRINT {expression}\n"));
        let error = result.expect_err(expression);
        let runtime = error.downcast_ref::<rice::error::RuntimeError>().unwrap();
        assert_eq!(runtime.basic_err_code(), code, "{expression}");
    }
}

#[test]
fn test_native_integer_loops_and_control_flow_preserve_exact_values() {
    let source = r#"
DIM counter AS INT64
FOR counter = CINT64("9007199254740992") TO CINT64("9007199254740994")
    PRINT counter
NEXT counter
FOR counter = CINT64(0) TO CINT64("18014398509481986") STEP CINT64("9007199254740993")
    PRINT counter
NEXT counter
SELECT CASE CINT64("9007199254740993")
    CASE 9007199254740992
        PRINT "rounded"
    CASE CINT64("9007199254740993")
        PRINT "exact"
END SELECT
IF TRUE() AND CBOOL(1) THEN PRINT "enabled"
"#;

    assert_eq!(
        run_bas(&source),
        "9007199254740992\n9007199254740993\n9007199254740994\n0\n9007199254740993\n18014398509481986\nexact\nenabled\n",
    );
}

#[test]
fn test_native_type_errors_are_catchable_and_preserve_destinations() {
    for (setup, operation, observe, expected) in [
        ("DIM n AS INT64\nn = CINT64(42)", "n = 1.5", "n", "42"),
        (
            "DIM n AS INT64\nn = CINT64(42)",
            "n = CINT64(\"9223372036854775807\") + CINT64(1)",
            "n",
            "42",
        ),
        (
            "DIM n AS INT64\nn = CINT64(42)",
            "n = -CINT64(\"-9223372036854775808\")",
            "n",
            "42",
        ),
        ("DIM n AS BYTE\nn = 254", "n = 256", "n", "254"),
        ("DIM n AS BOOL\nn = TRUE()", "n = \"yes\"", "n", "TRUE"),
        (
            "DIM n AS BYTES\nn = BYTES(255)",
            "n = \"text\"",
            "HEXENCODE$(n)",
            "FF",
        ),
        ("DIM n(2) AS BYTE\nn(1) = 12", "n(1) = -1", "n(1)", "12"),
        (
            "TYPE Packet\n n AS BYTE\nEND TYPE\nDIM p AS Packet\np.n = 12",
            "p.n = 256",
            "p.n",
            "12",
        ),
    ] {
        let source = format!(
            "{setup}\nWHEN EXCEPTION IN\n{operation}\nUSE\nPRINT \"caught\"\nEND WHEN\nPRINT {observe}\n"
        );
        assert_eq!(
            run_bas(&source),
            format!("caught\n{expected}\n"),
            "{operation}"
        );
    }
    for expression in [
        "TRUE() + 1",
        "CINT64(\"9007199254740993\") + 0.5",
        "CINT64(\"9223372036854775807\") * CINT64(2)",
        "UTF8DECODE$(BYTES(255))",
        "BYTEAT(BYTES(1), 0)",
        "BYTEAT(BYTES(1), 1.5)",
    ] {
        assert!(
            run_bas_may_fail(&format!("PRINT {expression}\n"))
                .1
                .is_err(),
            "{expression}"
        );
    }
}

#[test]
fn test_native_values_retain_types_in_arrays_records_and_clear() {
    let source = r#"
TYPE Packet
    identifier AS INT64
    active AS BOOL
    kind AS BYTE
    payload AS BYTES
END TYPE
DIM ids(2) AS INT64, flags(2) AS BOOL, octets(2) AS BYTE, buffers(2) AS BYTES
DIM packet AS Packet
ids(1) = CINT64("9223372036854775807")
flags(1) = TRUE()
octets(1) = 255
buffers(1) = BYTES(0, 255)
packet.identifier = ids(1)
packet.active = flags(1)
packet.kind = octets(1)
packet.payload = buffers(1)
PRINT packet.identifier; "|"; packet.active; "|"; packet.kind; "|"; HEXENCODE$(packet.payload)
PRINT TYPEOF$(ids(2)); "|"; TYPEOF$(flags(2)); "|"; TYPEOF$(octets(2)); "|"; TYPEOF$(buffers(2))
REDIM PRESERVE ids(3) AS INT64
PRINT ids(1); "|"; TYPEOF$(ids(3)); "|"; ids(3)
CLEAR
PRINT TYPEOF$(ids(1)); "|"; ids(1); "|"; flags(1); "|"; octets(1); "|"; LEN(buffers(1))
PRINT packet.identifier; "|"; packet.active; "|"; packet.kind; "|"; LEN(packet.payload)
"#;

    assert_eq!(
        run_bas(&source),
        concat!(
            "9223372036854775807|TRUE|255|00FF\nINT64|BOOL|BYTE|BYTES\n",
            "9223372036854775807|INT64|0\nINT64|0|FALSE|0|0\n0|FALSE|0|0\n"
        ),
    );
}

#[test]
fn test_native_array_resize_preserves_declared_type_and_rejects_type_changes() {
    let source = r#"
DIM ids(2) AS INT64
ids(1) = CINT64("9007199254740993")
REDIM PRESERVE ids(3)
PRINT ids(1); "|"; TYPEOF$(ids(1)); "|"; TYPEOF$(ids(3))
WHEN EXCEPTION IN
    REDIM PRESERVE ids(4) AS BYTE
USE
    PRINT "caught"
END WHEN
PRINT ids(1); "|"; TYPEOF$(ids(1)); "|"; UBOUND(ids)
DIM buffers(2) AS BYTES
buffers(1) = BYTES(0, 255)
ERASE buffers
PRINT TYPEOF$(buffers(1)); "|"; LEN(buffers(1))
"#;

    assert_eq!(
        run_bas(&source),
        "9007199254740993|INT64|INT64\ncaught\n9007199254740993|INT64|3\nBYTES|0\n",
    );
}

#[test]
fn test_native_procedure_arguments_results_and_copy_semantics() {
    let source = r#"
DIM number AS INT64, payload AS BYTES
number = CINT64("9007199254740993")
payload = BYTES(1, 2)
CALL Increment(number)
PRINT number
CALL ExplicitIncrement(number)
PRINT number
CALL ChangeCopy(payload)
PRINT HEXENCODE$(payload)
PRINT Success(); "|"; TYPEOF$(Success()); "|"; NextNumber(number)
SUB Increment(value AS INT64)
    value = value + CINT64(1)
END SUB
SUB ExplicitIncrement(BYREF value AS INT64)
    value = value + CINT64(1)
END SUB
SUB ChangeCopy(BYVAL value AS BYTES)
    value = BYTESET(value, 1, 255)
END SUB
FUNCTION Success() AS BOOL
    Success = TRUE()
END FUNCTION
FUNCTION NextNumber(BYVAL value AS INT64) AS INT64
    NextNumber = value + CINT64(1)
END FUNCTION
"#;
    assert_eq!(
        run_bas(source),
        "9007199254740994\n9007199254740995\n0102\nTRUE|BOOL|9007199254740996\n"
    );
    for source in [
        "DIM b AS BYTE\nCALL Invalid(b)\nSUB Invalid(BYREF value AS BYTE)\nvalue = 256\nEND SUB\n",
        "PRINT Invalid()\nFUNCTION Invalid() AS BYTE\nInvalid = 256\nEND FUNCTION\n",
        "CALL Invalid(0.5)\nSUB Invalid(BYVAL value AS INT64)\nEND SUB\n",
    ] {
        assert!(run_bas_may_fail(source).1.is_err(), "{source}");
    }
}

#[test]
fn test_native_input_read_and_write_preserve_exact_integer_text() {
    let source = r#"
DIM number AS INT64, enabled AS BOOL, octet AS BYTE
DATA "9223372036854775807", "TRUE", 255
READ number, enabled, octet
PRINT number; "|"; enabled; "|"; octet
OPEN "{DIR}/native.csv" FOR OUTPUT AS #1
WRITE #1, number, enabled, octet
CLOSE #1
number = 0
enabled = FALSE()
octet = 0
OPEN "{DIR}/native.csv" FOR INPUT AS #1
INPUT #1, number, enabled, octet
CLOSE #1
PRINT number; "|"; enabled; "|"; octet
"#;

    let (output, dir) = run_bas_with_tmpdir(&source);
    assert_eq!(
        output,
        "9223372036854775807|TRUE|255\n9223372036854775807|TRUE|255\n",
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("native.csv")).unwrap(),
        "9223372036854775807,TRUE,255\n"
    );
}

#[test]
fn test_native_binary_buffer_file_io_preserves_all_bytes_and_short_reads() {
    let source = r#"
DIM payload AS BYTES, readback AS BYTES
payload = BYTEBUFFER(256)
FOR index = 1 TO 256
    payload = BYTESET(payload, index, index - 1)
NEXT index
OPEN "{DIR}/bytes.bin" FOR BINARY AS #1
PUT #1, , payload
PRINT LOF(1)
readback = BYTEBUFFER(256)
GET #1, 1, readback
PRINT readback = payload
readback = BYTEBUFFER(8)
GET #1, 255, readback
PRINT HEXENCODE$(readback); "|"; LEN(readback)
CLOSE #1
"#;

    let (output, dir) = run_bas_with_tmpdir(&source);
    assert_eq!(output, "256\nTRUE\nFEFF|2\n");
    assert_eq!(
        std::fs::read(dir.path().join("bytes.bin")).unwrap(),
        (0..=255).collect::<Vec<u8>>()
    );
}

#[test]
fn test_native_record_binary_io_has_stable_exact_encoding() {
    let source = r#"
TYPE Packet
    identifier AS INT64
    enabled AS BOOL
    kind AS BYTE
    payload AS BYTES
END TYPE
DIM sent AS Packet, received AS Packet
sent.identifier = CINT64("9223372036854775807")
sent.enabled = TRUE()
sent.kind = 255
sent.payload = BYTES(0, 128, 255)
OPEN "{DIR}/packet.bin" FOR BINARY AS #1
PUT #1, , sent
GET #1, 1, received
PRINT received.identifier; "|"; received.enabled; "|"; received.kind; "|"; HEXENCODE$(received.payload)
CLOSE #1
"#;
    let (output, dir) = run_bas_with_tmpdir(source);
    assert_eq!(output, "9223372036854775807|TRUE|255|0080FF\n");
    let mut expected = i64::MAX.to_le_bytes().to_vec();
    expected.extend([1, 255]);
    expected.extend(3_u32.to_le_bytes());
    expected.extend([0, 128, 255]);
    assert_eq!(
        std::fs::read(dir.path().join("packet.bin")).unwrap(),
        expected
    );
}

#[test]
fn test_native_scalar_binary_io_is_exact() {
    let source = r#"
DIM identifier AS INT64, enabled AS BOOL, octet AS BYTE
identifier = CINT64("-9223372036854775808")
enabled = TRUE()
octet = 255
OPEN "{DIR}/scalars.bin" FOR BINARY AS #1
PUT #1, , identifier
PUT #1, , enabled
PUT #1, , octet
PRINT LOF(1)
identifier = 0
enabled = FALSE()
octet = 0
GET #1, 1, identifier
GET #1, , enabled
GET #1, , octet
PRINT identifier; "|"; enabled; "|"; octet
CLOSE #1
"#;

    let (output, dir) = run_bas_with_tmpdir(&source);
    assert_eq!(output, "10\n-9223372036854775808|TRUE|255\n");
    let mut expected = i64::MIN.to_le_bytes().to_vec();
    expected.extend([1, 255]);
    assert_eq!(
        std::fs::read(dir.path().join("scalars.bin")).unwrap(),
        expected
    );
}

#[test]
fn test_native_swap_and_byref_type_errors_preserve_all_destinations() {
    for operation in ["SWAP wider, smaller", "CALL Change(wider, smaller)"] {
        let source = format!(
            r#"
DIM wider AS INT64, smaller AS BYTE
wider = 256
smaller = 7
WHEN EXCEPTION IN
    {operation}
USE
    PRINT "caught"
END WHEN
PRINT wider; "|"; smaller
SUB Change(BYREF first, BYREF second)
    first = 42
    second = 256
END SUB
"#
        );
        assert_eq!(run_bas(&source), "caught\n256|7\n", "{operation}");
    }
}

#[test]
fn test_native_matrix_type_failure_preserves_entire_destination() {
    let source = r#"
DIM target(1, 2) AS BYTE, source(1, 2) AS NUMERIC
target(1, 1) = 7
target(1, 2) = 8
source(1, 1) = 12
source(1, 2) = 256
WHEN EXCEPTION IN
    MAT target = source
USE
    PRINT "caught"
END WHEN
PRINT target(1, 1); "|"; target(1, 2); "|"; TYPEOF$(target(1, 2))
"#;
    assert_eq!(run_bas(source), "caught\n7|8|BYTE\n");
}

#[test]
fn test_native_matrix_never_replaces_unrepresentable_int64_with_zero() {
    let source = r#"
DIM source(1, 1) AS INT64, target(1, 1) AS NUMERIC
source(1, 1) = CINT64("9007199254740993")
target(1, 1) = 7
WHEN EXCEPTION IN
    MAT target = source
USE
    PRINT "caught"
END WHEN
PRINT target(1, 1)
"#;
    assert_eq!(run_bas(source), "caught\n7\n");
}

#[test]
fn test_native_failed_dimension_change_preserves_array_type() {
    let source = r#"
DIM values(2) AS BYTE
values(1) = 7
WHEN EXCEPTION IN
    DIM values(2 TO 1) AS BOOL
USE
    PRINT "caught"
END WHEN
values(1) = 255
PRINT TYPEOF$(values(1)); "|"; values(1); "|"; UBOUND(values)
"#;
    assert_eq!(run_bas(source), "caught\nBYTE|255|2\n");
}

#[test]
fn test_native_static_type_survives_a_skipped_declaration_on_later_calls() {
    let source = r#"
CALL Remember(TRUE())
WHEN EXCEPTION IN
    CALL Remember(FALSE())
USE
    PRINT "caught"
END WHEN
SUB Remember(BYVAL first AS BOOL)
    IF first THEN
        STATIC octet AS BYTE
        octet = 7
    ELSE
        octet = 256
    END IF
    PRINT TYPEOF$(octet); "|"; octet
END SUB
"#;
    assert_eq!(run_bas(source), "BYTE|7\ncaught\n");
}

#[test]
fn test_native_typed_parameters_shadow_shared_globals() {
    let source = r#"
DIM SHARED value AS BYTE
value = 7
CALL LocalValue(CINT64(256))
PRINT TYPEOF$(value); "|"; value
SUB LocalValue(BYVAL value AS INT64)
    value = value + 1
    PRINT TYPEOF$(value); "|"; value
END SUB
"#;
    assert_eq!(run_bas(source), "INT64|257\nBYTE|7\n");
}

#[test]
fn test_native_invalid_binary_values_leave_existing_values_untouched() {
    let source = r#"
DIM raw AS BYTES, flag AS BOOL
raw = BYTES(2)
flag = TRUE()
OPEN "{DIR}/invalid-bool.bin" FOR BINARY AS #1
PUT #1, , raw
WHEN EXCEPTION IN
    GET #1, 1, flag
USE
    PRINT "invalid bool"
END WHEN
PRINT flag
CLOSE #1
TYPE Payload
    content AS BYTES
END TYPE
DIM item AS Payload
item.content = BYTES(42)
raw = BYTES(255, 255, 255, 255)
OPEN "{DIR}/invalid-length.bin" FOR BINARY AS #1
PUT #1, , raw
WHEN EXCEPTION IN
    GET #1, 1, item
USE
    PRINT "invalid length"
END WHEN
PRINT HEXENCODE$(item.content)
CLOSE #1
"#;
    let (output, _dir) = run_bas_with_tmpdir(source);
    assert_eq!(output, "invalid bool\nTRUE\ninvalid length\n2A\n");
}

#[test]
fn test_native_binary_put_after_clear_uses_declared_defaults() {
    let source = r#"
DIM flag AS BOOL, octet AS BYTE, payload AS BYTES
flag = TRUE()
octet = 255
payload = BYTES(42)
CLEAR
OPEN "{DIR}/cleared.bin" FOR BINARY AS #1
PUT #1, , flag
PUT #1, , octet
PUT #1, , payload
PRINT LOF(1)
CLOSE #1
"#;

    let (output, dir) = run_bas_with_tmpdir(&source);
    assert_eq!(output, "2\n");
    assert_eq!(
        std::fs::read(dir.path().join("cleared.bin")).unwrap(),
        vec![0, 0]
    );
}

#[test]
fn test_native_procedure_clear_restores_typed_results_and_byref_defaults() {
    let source = r#"
DIM payload AS BYTES
payload = BYTES(42)
CALL ClearParameter(payload)
PRINT TYPEOF$(payload); "|"; LEN(payload)
PRINT TYPEOF$(ClearResult())
SUB ClearParameter(BYREF value AS BYTES)
    CLEAR
END SUB
FUNCTION ClearResult() AS BYTES
    ClearResult = BYTES(1)
    CLEAR
END FUNCTION
"#;
    assert_eq!(run_bas(source), "BYTES|0\nBYTES\n");
}

#[test]
fn test_native_integer_logical_operations_reject_fractional_operands() {
    for operator in ["AND", "OR", "XOR"] {
        for expression in [
            format!("CINT64(\"9007199254740992\") {operator} 1.5"),
            format!("1.5 {operator} CBYTE(1)"),
        ] {
            let (_, result) = run_bas_may_fail(&format!("PRINT {expression}\n"));
            let error = result.expect_err(&expression);
            let runtime = error.downcast_ref::<rice::error::RuntimeError>().unwrap();
            assert_eq!(runtime.basic_err_code(), 13, "{expression}");
        }
    }
}

#[test]
fn test_declared_types_have_distinct_storage_and_consistent_conversion() {
    assert_eq!(
        run_file("tests/programs/declared_types.bas"),
        concat!(
            "INTEGER|LONG|SINGLE|NUMERIC\n",
            "-32768|2147483647|16777216|1.5\n",
            "[    ]\n[é🙂  ]4\nabcd\n",
            "INTEGER|[x  ]\nINTEGER|INTEGER|0\nNUMERIC|1.5\n",
            "INTEGER|LONG|SINGLE|NUMERIC|STRING\nINTEGER|[q  ]\n5|LONG\n",
            "1|INTEGER\n2|INTEGER\n21|[bye ]\n"
        )
    );
}

#[test]
fn test_declared_assignment_errors_preserve_value_and_type() {
    for (declaration, initial, invalid, expected) in [
        ("INTEGER", "12", "1.5", "12|INTEGER"),
        ("INTEGER", "12", "32768", "12|INTEGER"),
        ("INTEGER", "12", "-32769", "12|INTEGER"),
        ("LONG", "42", "2147483648", "42|LONG"),
        ("LONG", "42", "-2147483649", "42|LONG"),
        ("SINGLE", "1.25", "1E100", "1.25|SINGLE"),
        ("NUMERIC", "3.5", "\"bad\"", "3.5|NUMERIC"),
        ("DOUBLE", "3.5", "\"bad\"", "3.5|NUMERIC"),
        ("STRING", "\"kept\"", "42", "kept|STRING"),
        ("STRING * 4", "\"ok\"", "42", "ok  |STRING"),
    ] {
        let source = format!(
            "DIM value AS {declaration}\nvalue = {initial}\nWHEN EXCEPTION IN\nvalue = {invalid}\nUSE\nPRINT \"caught\"\nEND WHEN\nPRINT value; \"|\"; TYPEOF$(value)\n"
        );
        assert_eq!(
            run_bas(&source),
            format!("caught\n{expected}\n"),
            "{declaration}: {invalid}"
        );
    }
    for name in ["value%", "value&", "value!", "value#", "value$"] {
        let initial = if name.ends_with('$') { "\"kept\"" } else { "7" };
        let invalid = if name.ends_with('$') { "7" } else { "\"bad\"" };
        let source = format!(
            "{name} = {initial}\nWHEN EXCEPTION IN\n{name} = {invalid}\nUSE\nPRINT \"caught\"\nEND WHEN\nPRINT {name}\n"
        );
        assert_eq!(
            run_bas(&source),
            if name.ends_with('$') {
                "caught\nkept\n"
            } else {
                "caught\n7\n"
            }
        );
    }
}

#[test]
fn test_declared_record_identity_and_field_types_are_enforced() {
    let setup = "TYPE First\nnumber AS INTEGER\nEND TYPE\nTYPE Second\nnumber AS INTEGER\nEND TYPE\nDIM first AS First, second AS Second, items(2) AS First\nfirst.number = 9\nitems(1).number = 7\n";
    for operation in [
        "first = second",
        "first.number = 2.5",
        "items(1) = second",
        "items(1).number = 32768",
    ] {
        assert_eq!(
            run_bas(&format!(
                "{setup}WHEN EXCEPTION IN\n{operation}\nUSE\nPRINT \"caught\"\nEND WHEN\nPRINT first.number; \"|\"; items(1).number\n"
            )),
            "caught\n9|7\n",
            "{operation}"
        );
    }
    for operation in [
        "CALL Wrong(second)\nSUB Wrong(BYVAL value AS First)\nPRINT \"unexpected\"\nEND SUB\n",
        "PRINT Wrong().number\nFUNCTION Wrong() AS First\nSHARED second\nWrong = second\nEND FUNCTION\n",
    ] {
        let (output, result) = run_bas_may_fail(&format!("{setup}{operation}"));
        assert!(result.is_err());
        assert!(output.is_empty());
    }
}

#[test]
fn test_declared_swap_and_byref_reject_narrowing_without_partial_write() {
    assert_eq!(
        run_bas(
            r#"
DIM small AS INTEGER, wide AS LONG
small = 7: wide = 40000
WHEN EXCEPTION IN
    SWAP small, wide
USE
    PRINT "swap rejected"
END WHEN
PRINT small; "|"; wide
DIM other AS INTEGER
other = 8
WHEN EXCEPTION IN
    CALL Change(small, other)
USE
    PRINT "copyback rejected"
END WHEN
PRINT small; "|"; other
SUB Change(BYREF first AS LONG, BYREF second AS LONG)
    first = 10
    second = 40000
END SUB
"#
        ),
        "swap rejected\n7|40000\ncopyback rejected\n7|8\n"
    );
}

#[test]
fn test_declared_types_survive_clear_and_input() {
    let source = r#"
DIM small AS INTEGER, wide AS LONG, approximate AS SINGLE, fixed AS STRING * 3
INPUT small, wide, approximate, fixed
PRINT TYPEOF$(small); "|"; TYPEOF$(wide); "|"; TYPEOF$(approximate); "|"; "["; fixed; "]"
READ small, wide, fixed
DATA 17, 2147483647, "x"
PRINT small; "|"; wide; "|"; "["; fixed; "]"
CLEAR
PRINT TYPEOF$(small); "|"; small; "|"; TYPEOF$(wide); "|"; TYPEOF$(approximate); "|"; "["; fixed; "]"
"#;
    assert_eq!(
        run_bas_with_input(source, "12,2147483647,1.25,abcdef\n"),
        "? INTEGER|LONG|SINGLE|[abc]\n17|2147483647|[x  ]\nINTEGER|0|LONG|SINGLE|[   ]\n"
    );
}

#[test]
fn test_def_types_bind_new_names_without_retyping_existing_values() {
    assert_eq!(
        run_bas(
            r#"
existing = "kept"
DEFINT A-Z
created = 7
PRINT TYPEOF$(created); "|"; existing
DEFLNG C
later = 8
created = 9
counter = 10
PRINT TYPEOF$(created); "|"; TYPEOF$(counter); "|"; TYPEOF$(later)
DIM declared
PRINT TYPEOF$(declared)
"#
        ),
        "INTEGER|kept\nINTEGER|LONG|INTEGER\nNUMERIC\n"
    );
}

#[test]
fn test_declare_signatures_match_types_modes_and_results() {
    assert_eq!(
        run_bas(
            r#"
DECLARE FUNCTION NextValue(BYVAL different AS DOUBLE) AS NUMERIC
PRINT NextValue(4)
FUNCTION NextValue(BYVAL value AS NUMERIC) AS DOUBLE
    NextValue = value + 1
END FUNCTION
"#
        ),
        "5\n"
    );
    for (declaration, definition) in [
        (
            "DECLARE SUB Work(value AS INTEGER)",
            "SUB Work(value AS LONG)\nEND SUB",
        ),
        (
            "DECLARE SUB Work(BYVAL value AS INTEGER)",
            "SUB Work(BYREF value AS INTEGER)\nEND SUB",
        ),
        (
            "DECLARE SUB Work(value() AS INTEGER)",
            "SUB Work(value AS INTEGER)\nEND SUB",
        ),
        ("DECLARE SUB Work(value AS INTEGER)", "SUB Work()\nEND SUB"),
        (
            "DECLARE FUNCTION Work() AS INTEGER",
            "FUNCTION Work() AS LONG\nEND FUNCTION",
        ),
        ("DECLARE FUNCTION Work() AS INTEGER", "SUB Work()\nEND SUB"),
    ] {
        let (output, result) = run_bas_may_fail(&format!(
            "{declaration}\nPRINT \"unexpected\"\n{definition}\n"
        ));
        assert!(result.is_err(), "{declaration}: {definition}");
        assert!(
            output.is_empty(),
            "signature validation must precede execution"
        );
    }
    let (mut interpreter, output) = input_session("");
    interpreter
        .run_source("DECLARE FUNCTION Later(BYVAL value AS INTEGER) AS LONG\n")
        .unwrap();
    interpreter.run_source("FUNCTION Later(BYVAL renamed AS INTEGER) AS LONG\nLater = renamed + 1\nEND FUNCTION\nPRINT Later(7)\n").unwrap();
    assert_eq!(output.into_string(), "8\n");
}

#[test]
fn test_typed_static_array_retains_values_and_constraints() {
    assert_eq!(
        run_bas(
            r#"
CALL Tick
CALL Tick
SUB Tick
    STATIC values(2) AS INTEGER
    values(1) = values(1) + 1
    WHEN EXCEPTION IN
        values(2) = 32768
    USE
        PRINT "rejected"
    END WHEN
    PRINT values(1); "|"; values(2); "|"; TYPEOF$(values(1))
END SUB
"#
        ),
        "rejected\n1|0|INTEGER\nrejected\n2|0|INTEGER\n"
    );
}

#[test]
fn test_typed_shared_and_common_arrays_convert_existing_values() {
    assert_eq!(
        run_bas(
            r#"
values(1) = 4: values(2) = 5
COMMON values() AS INTEGER
PRINT TYPEOF$(values(1)); "|"; values(2)
other(1) = 6: other(2) = 7
CALL ConvertShared
PRINT TYPEOF$(other(1)); "|"; other(1); "|"; other(2)
SUB ConvertShared
    SHARED other() AS LONG
    other(1) = other(1) + 3
END SUB
"#
        ),
        "INTEGER|5\nLONG|9|7\n"
    );
    for declaration in ["SHARED values() AS INTEGER", "COMMON values() AS INTEGER"] {
        assert_eq!(
            run_bas(&format!(
                "values(1) = 4\nvalues(2) = 1.5\nWHEN EXCEPTION IN\n{declaration}\nUSE\nPRINT \"rejected\"\nEND WHEN\nPRINT TYPEOF$(values(1)); \"|\"; values(1); \"|\"; values(2)\n"
            )),
            "rejected\nNUMERIC|4|1.5\n",
            "{declaration}"
        );
    }
}

#[test]
fn test_redim_preserve_accepts_numeric_double_alias_and_retains_types() {
    assert_eq!(
        run_bas(
            r#"
DIM values(2) AS NUMERIC
values(1) = 1.5
REDIM PRESERVE values(3) AS DOUBLE
PRINT values(1); "|"; TYPEOF$(values(3))
DIM fixed(2) AS STRING * 3
fixed(1) = "a"
WHEN EXCEPTION IN
    REDIM PRESERVE fixed(4) AS STRING * 4
USE
    PRINT "rejected"
END WHEN
PRINT "["; fixed(1); "]"; "|"; LEN(fixed(2)); "|"; UBOUND(fixed)
"#
        ),
        "1.5|NUMERIC\nrejected\n[a  ]|3|2\n"
    );
}

#[test]
fn test_single_mixed_integer_arithmetic_checks_integral_range() {
    for expression in [
        "CINT(1) + CSNG(1E20)",
        "CSNG(1E20) + CINT(1)",
        "CINT(1) + CDBL(1E20)",
    ] {
        let (_, result) = run_bas_may_fail(&format!("PRINT {expression}\n"));
        assert!(
            matches!(
                result
                    .unwrap_err()
                    .downcast_ref::<rice::error::RuntimeError>(),
                Some(rice::error::RuntimeError::Overflow { .. })
            ),
            "{expression}"
        );
    }
}

#[test]
fn test_single_console_input_retries_nonfinite_values() {
    assert_eq!(
        run_bas_with_input(
            "DIM value AS SINGLE\nINPUT value\nPRINT value; \"|\"; TYPEOF$(value)\n",
            "NaN\n1.25\n"
        ),
        "? ? Redo from start\n? 1.25|SINGLE\n"
    );
}

#[test]
fn test_fixed_string_clear_mid_assignment_and_procedure_conversion() {
    assert_eq!(
        run_bas(
            r#"
DIM text AS STRING * 3
CLEAR
MID$(text, 1, 1) = "X"
PRINT "["; text; "]"
DIM larger AS STRING * 5
larger = "hello"
CALL Change(larger)
PRINT "["; larger; "]"
PRINT "["; ShortText(); "]"
SUB Change(BYREF value AS STRING * 3)
    PRINT "["; value; "]"
    value = "é"
END SUB
FUNCTION ShortText() AS STRING * 2
    ShortText = "long"
END FUNCTION
"#
        ),
        "[X  ]\n[hel]\n[é    ]\n[lo]\n"
    );
}

#[test]
fn test_declared_strings_work_with_field_lset_and_rset() {
    let (output, dir) = run_bas_with_tmpdir(
        r#"
DIM first AS STRING, second AS STRING
OPEN "{DIR}/fields.bin" FOR RANDOM AS #1 LEN = 6
FIELD #1, 3 AS first, 3 AS second
LSET first = "A"
RSET second = "B"
PUT #1, 1
LSET first = ""
RSET second = ""
GET #1, 1
PRINT "["; first; "]|["; second; "]"
CLOSE #1
DIM fixed AS STRING * 4
LSET fixed = "x"
PRINT "["; fixed; "]"
RSET fixed = "y"
PRINT "["; fixed; "]"
"#,
    );
    assert_eq!(output, "[A  ]|[  B]\n[x   ]\n[   y]\n");
    assert_eq!(
        std::fs::read(dir.path().join("fields.bin")).unwrap(),
        b"A    B"
    );
}

#[test]
fn test_typed_constants_retain_binary_width_metadata() {
    let (output, dir) = run_bas_with_tmpdir(
        r#"
CONST text AS STRING * 3 = "x", number AS INTEGER = 7
OPEN "{DIR}/constants.bin" FOR BINARY AS #1
PUT #1, , text
PUT #1, , number
PRINT LOF(1)
CLOSE #1
"#,
    );
    assert_eq!(output, "5\n");
    assert_eq!(
        std::fs::read(dir.path().join("constants.bin")).unwrap(),
        b"x  \x07\0"
    );
}

#[test]
fn test_shared_then_dim_converts_root_array_and_rejects_partial_conversion() {
    for (second, expected) in [
        ("2", "INTEGER|1|INTEGER|2\n"),
        ("1.5", "rejected\nNUMERIC|1|NUMERIC|1.5\n"),
    ] {
        let source = format!(
            r#"
values(1) = 1: values(2) = {second}
WHEN EXCEPTION IN
    CALL DeclareShared
USE
    PRINT "rejected"
END WHEN
PRINT TYPEOF$(values(1)); "|"; values(1); "|"; TYPEOF$(values(2)); "|"; values(2)
SUB DeclareShared
    SHARED values()
    DIM values(2) AS INTEGER
END SUB
"#
        );
        assert_eq!(run_bas(&source), expected);
    }
}

#[test]
fn test_static_metadata_survives_clear_without_saved_values_and_skipped_declarations() {
    for (procedure_static, declaration) in [("", "STATIC"), (" STATIC", "DIM")] {
        let source = format!(
            r#"
CALL Remember(TRUE())
CALL Remember(FALSE())
CALL Remember(FALSE())
SUB Remember(BYVAL first AS BOOL){procedure_static}
    IF first THEN
        {declaration} counter AS INTEGER
        counter = 9
        CLEAR
        EXIT SUB
    END IF
    WHEN EXCEPTION IN
        counter = 1.5
    USE
        PRINT "rejected"
    END WHEN
    counter = counter + 1
    PRINT TYPEOF$(counter); "|"; counter
END SUB
"#
        );
        assert_eq!(
            run_bas(&source),
            "rejected\nINTEGER|1\nrejected\nINTEGER|2\n",
            "{procedure_static}: {declaration}"
        );
    }
    assert_eq!(
        run_bas(
            r#"
PRINT Remember(TRUE())
PRINT Remember(FALSE())
PRINT Remember(FALSE())
FUNCTION Remember(BYVAL first AS BOOL) AS INTEGER STATIC
    IF first THEN
        DIM counter AS INTEGER
        counter = 9
        CLEAR
        EXIT FUNCTION
    END IF
    WHEN EXCEPTION IN
        counter = 1.5
    USE
        PRINT "rejected"
    END WHEN
    counter = counter + 1
    PRINT TYPEOF$(counter)
    Remember = counter
END FUNCTION
"#
        ),
        "0\nrejected\nINTEGER\n1\nrejected\nINTEGER\n2\n"
    );
}

#[test]
fn test_def_type_constrains_new_local_write_despite_parent_binding() {
    assert_eq!(
        run_bas(
            r#"
DEFINT A
a = 1
WHEN EXCEPTION IN
    CALL Work
USE
    PRINT "rejected"
END WHEN
PRINT a; "|"; TYPEOF$(a)
SUB Work
    a = 1.5
END SUB
"#
        ),
        "rejected\n1|INTEGER\n"
    );
}

#[test]
fn test_declared_defaults_precede_bare_builtin_fallback_after_clear() {
    assert_eq!(
        run_bas(
            r#"
DIM TRUE AS STRING, FALSE AS INTEGER
TRUE = "old": FALSE = 7
CLEAR
PRINT TYPEOF$(TRUE); "|"; LEN(TRUE); "|"; TYPEOF$(FALSE); "|"; FALSE
PRINT TRUE(); "|"; FALSE()
"#
        ),
        "STRING|0|INTEGER|0\nTRUE|FALSE\n"
    );
}

#[test]
fn test_dim_constant_element_collision_preserves_array_metadata() {
    for declaration in ["DIM items(3) AS LONG", "DIM items(3) AS INTEGER"] {
        let (mut interpreter, output) = input_session("");
        interpreter
            .run_source("DIM items(2) AS INTEGER\nitems(1) = 3\nCONST items_1 = 7\n")
            .unwrap();
        let error = interpreter.run_source(declaration).unwrap_err();
        assert!(
            error.to_string().contains("constant"),
            "{declaration}: {error}"
        );
        interpreter
            .run_source("PRINT items_1; \"|\"; UBOUND(items); \"|\"; TYPEOF$(items(2))\n")
            .unwrap();
        assert_eq!(output.into_string(), "7|2|INTEGER\n", "{declaration}");
    }
}

#[test]
fn test_typed_array_reads_preserve_parent_values_and_local_shadowing() {
    assert_eq!(
        run_file("tests/programs/type_array_scope_regressions.bas"),
        "42|INTEGER|43\n42|42\n7|LONG|0\n42|INTEGER\n9|BYTE\n10|BYTE\n"
    );
}

#[test]
fn test_for_uses_inherited_shared_counter_type_and_storage() {
    assert_eq!(
        run_file("tests/programs/type_shared_for_regressions.bas"),
        "1|INTEGER\n2|INTEGER\n3|INTEGER\nrejected\n3|INTEGER\n"
    );
}

#[test]
fn test_rejected_declare_restores_prescanned_session_state() {
    let (mut interpreter, output) = input_session("");
    interpreter
        .run_source(
            r#"
DECLARE SUB Work()
SUB Work()
    PRINT "original"
END SUB
FUNCTION Answer() AS INTEGER
    Answer = 42
END FUNCTION
TYPE Kept
    number AS INTEGER
END TYPE
Original: DATA 10, 20
READ consumed
"#,
        )
        .unwrap();
    let rejected = interpreter.run_source(
        r#"
DECLARE SUB Work(value AS INTEGER)
FUNCTION Answer() AS INTEGER
    Answer = 99
END FUNCTION
SUB Leaked()
    PRINT "unexpected procedure"
END SUB
TYPE Kept
    number AS STRING
END TYPE
Original: DATA 99
DATA 100
PRINT "unexpected execution"
"#,
    );
    assert!(rejected.is_err());
    interpreter
        .run_source(
            r#"
PRINT 42
CALL Work
PRINT Answer()
DIM record AS Kept
record.number = 7
PRINT record.number; "|"; TYPEOF$(record.number)
READ remaining
PRINT remaining
RESTORE Original
READ first, second
PRINT first; "|"; second
"#,
        )
        .unwrap();
    assert!(
        interpreter
            .run_source("READ beyond\n")
            .unwrap_err()
            .to_string()
            .contains("past end of DATA")
    );
    assert!(interpreter.run_source("CALL Leaked\n").is_err());
    interpreter.run_source("PRINT \"recovered\"\n").unwrap();
    assert_eq!(
        output.into_string(),
        "42\noriginal\n42\n7|INTEGER\n20\n10|20\nrecovered\n"
    );
}

#[test]
fn test_rejected_definition_keeps_pending_declaration_usable() {
    let (mut interpreter, output) = input_session("");
    interpreter
        .run_source("DECLARE FUNCTION Pending(BYVAL value AS INTEGER) AS INTEGER\n")
        .unwrap();
    assert!(
        interpreter
            .run_source(
                "FUNCTION Pending(BYVAL value AS LONG) AS INTEGER\nPending = value + 99\nEND FUNCTION\n"
            )
            .is_err()
    );
    interpreter.run_source("PRINT 42\n").unwrap();
    interpreter
        .run_source(
            "FUNCTION Pending(BYVAL value AS INTEGER) AS INTEGER\nPending = value + 1\nEND FUNCTION\nPRINT Pending(7)\n"
        )
        .unwrap();
    assert_eq!(output.into_string(), "42\n8\n");
}

#[test]
fn test_read_dynamic_bindings_ignore_previous_native_value_type() {
    for initial in [
        "CINT(1)",
        "CLNG(1)",
        "CSNG(1)",
        "CINT64(1)",
        "TRUE()",
        "CBYTE(1)",
        "BYTES(1)",
    ] {
        let source = format!(
            "value = {initial}\nitems(1) = {initial}\nREAD value, items(1)\nDATA \"abc\", \"def\"\nPRINT value; \"|\"; TYPEOF$(value); \"|\"; items(1); \"|\"; TYPEOF$(items(1))\n"
        );
        assert_eq!(run_bas(&source), "abc|STRING|def|STRING\n", "{initial}");
    }
    assert_eq!(
        run_bas(
            "DIM value AS INTEGER, items(1) AS BOOL\nREAD value, items(1)\nDATA \"42\", \"TRUE\"\nPRINT value; \"|\"; TYPEOF$(value); \"|\"; items(1); \"|\"; TYPEOF$(items(1))\n"
        ),
        "42|INTEGER|TRUE|BOOL\n"
    );
}

#[test]
fn test_shared_and_common_preserve_existing_dynamic_bindings_after_def_type() {
    assert_eq!(
        run_file("tests/programs/type_shared_dynamic_regressions.bas"),
        "shared|common|common shared\none|two|three\nupdated shared|updated common|updated common shared\nupdated one|updated two|updated three\n"
    );
}

#[test]
fn test_def_type_only_constrains_new_implicit_array_bindings() {
    assert_eq!(
        run_file("tests/programs/type_implicit_array_regressions.bas"),
        "after|second|STRING\n7|INTEGER\nnew array rejected\nafter\nlocal array rejected\n0|NUMERIC\nuntouched local array rejected\nafter|second\n"
    );
}

#[test]
fn test_existing_dynamic_arrays_keep_defaults_after_defstr_without_local_read_bindings() {
    assert_eq!(
        run_file("tests/programs/type_array_defaults_after_defstr.bas"),
        "0|NUMERIC\n10|NUMERIC\nexisting read preserves local DEFSTR\n0|NUMERIC\nuntouched read preserves local DEFSTR\n10|0|NUMERIC|42\n"
    );
}

#[test]
fn test_untouched_typed_array_elements_enforce_atomic_byref_copyback() {
    assert_eq!(
        run_file("tests/programs/type_array_byref_copyback_regressions.bas"),
        "0|LONG\n12|INTEGER\ncopyback rejected\n0|INTEGER|0|BYTE\n"
    );
}

#[test]
fn test_byref_implicit_destinations_apply_def_type_without_partial_bindings() {
    let caller_output = "sub rejected\n10|NUMERIC|40|NUMERIC\nfunction rejected\n20|NUMERIC|30|NUMERIC\n7|INTEGER|8|INTEGER\n";
    assert_eq!(
        run_file("tests/programs/type_byref_implicit_destination_regressions.bas"),
        format!("{caller_output}{caller_output}TRUE\n10|20|30|40\n")
    );
}

#[test]
fn test_static_dynamic_arrays_preserve_initialization_until_clear() {
    assert_eq!(
        run_file("tests/programs/type_static_dynamic_array_regressions.bas"),
        "first|STRING\nfirst|STRING\n0|NUMERIC\nNUMERIC\nsecond|STRING\nsecond|STRING\nread-only sub|STRING\nread-only function|STRING\n"
    );
    for (definition, call, exit, end) in [
        (
            "SUB ResetState(BYVAL stage AS INTEGER) STATIC",
            "CALL ResetState",
            "EXIT SUB",
            "END SUB",
        ),
        (
            "FUNCTION ResetState(BYVAL stage AS INTEGER) AS INTEGER STATIC",
            "ignored = ResetState",
            "EXIT FUNCTION",
            "END FUNCTION",
        ),
    ] {
        let source = format!(
            r#"
{call}(1)
{call}(2)
DEFINT D, E, T
{call}(3)

{definition}
    IF stage = 1 THEN
        dynamicValues(1) = "old"
        PRINT TYPEOF$(emptyValues(1))
        DIM typedValues(1) AS BYTE
        {exit}
    END IF
    IF stage = 2 THEN
        CLEAR
        {exit}
    END IF
    WHEN EXCEPTION IN
        dynamicValues(1) = 1.5
    USE
        PRINT "dynamic reset"
    END WHEN
    WHEN EXCEPTION IN
        emptyValues(1) = 1.5
    USE
        PRINT "read-only reset"
    END WHEN
    WHEN EXCEPTION IN
        typedValues(1) = 256
    USE
        PRINT "declaration retained"
    END WHEN
    PRINT TYPEOF$(dynamicValues(1)); "|"; TYPEOF$(emptyValues(1)); "|"; TYPEOF$(typedValues(1))
{end}
"#
        );
        assert_eq!(
            run_bas(&source),
            "NUMERIC\ndynamic reset\nread-only reset\ndeclaration retained\nINTEGER|INTEGER|BYTE\n",
            "{definition}"
        );
    }
}

#[test]
fn test_lset_rset_distinguish_missing_empty_and_fixed_strings() {
    assert_eq!(
        run_file("tests/programs/type_lset_rset_regressions.bas"),
        "[left]|[right]|[default]\n[]|[]|[]\n[x   ]\n[   y]\n"
    );
}

#[test]
fn test_large_integer_exponents_only_overflow_for_unrepresentable_results() {
    assert_eq!(
        run_file("tests/programs/type_integer_power_regressions.bas"),
        "0|1|1\n0|1|-1\n1|-9223372036854775808\n"
    );
    for expression in [
        "CINT64(2) ^ CINT64(63)",
        "CINT64(2) ^ CINT64(\"4294967296\")",
        "CINT64(-2) ^ CINT64(\"4294967296\")",
        "CINT64(2) ^ CINT64(\"9223372036854775807\")",
    ] {
        let (output, result) = run_bas_may_fail(&format!("PRINT {expression}\n"));
        assert!(output.is_empty(), "{expression}");
        assert!(
            matches!(
                result
                    .unwrap_err()
                    .downcast_ref::<rice::error::RuntimeError>(),
                Some(rice::error::RuntimeError::Overflow { .. })
            ),
            "{expression}"
        );
    }
}

#[test]
fn test_structured_exceptions_resolve_local_and_outward_gotos() {
    assert_eq!(
        run_file("tests/programs/broad_exception_jumps.bas"),
        "body|2\nhandler|2|3001\ncleared|0\nnested body jump\noutward body jump\noutward handler jump|3001\n"
    );
}

#[test]
fn test_structured_exception_retry_after_continue_restarts_original_body() {
    assert_eq!(
        run_file("tests/programs/broad_exception_retry_continue.bas"),
        "completed|2|2\ncleared|0\nretained label|2\n"
    );
}

#[test]
fn test_structured_exception_continue_does_not_accumulate_stack_frames() {
    let mut source = String::from("handled = 0\nWHEN EXCEPTION IN\n");
    source.push_str(&"ERROR 1\n".repeat(4096));
    source.push_str(
        "PRINT \"completed\"\nUSE\nhandled = handled + 1\nCONTINUE\nEND WHEN\nPRINT handled; \"|\"; EXTYPE\n",
    );
    assert_eq!(run_bas(&source), "completed\n4096|0\n");
}

#[test]
fn test_structured_exception_preserves_outer_control_and_error_propagation() {
    assert_eq!(
        run_file("tests/programs/broad_exception_propagation.bas"),
        "body exit\nhandler exit\nloop exit|1\nouter caught handler error\nouter slot|3001\nshared slot|0\nclassic caught|7\nclassic resumed\n"
    );
    for (body, handler) in [
        ("PRINT \"body end\"\nEND", "PRINT \"wrong\""),
        ("ERROR 1", "PRINT \"handler end\"\nEND"),
    ] {
        assert_eq!(
            run_bas(&format!(
                "WHEN EXCEPTION IN\n{body}\nUSE\n{handler}\nEND WHEN\nPRINT \"wrong fallthrough\"\n"
            )),
            if body.starts_with("PRINT") {
                "body end\n"
            } else {
                "handler end\n"
            }
        );
    }
}

#[test]
fn test_seeking_flushes_pending_bytes_before_moving_shared_cursor() {
    let source = include_str!("programs/review_seek_buffered_output.bas");
    let (output, dir) = run_bas_with_tmpdir(source);
    assert_eq!(output, "1\n");
    for file in [
        "seek-binary.bin",
        "pointer-binary.bin",
        "seek-random.bin",
        "pointer-random.bin",
    ] {
        assert_eq!(
            std::fs::read(dir.path().join(file)).unwrap(),
            b"ABC",
            "{file}"
        );
    }
}

#[test]
fn test_field_records_cross_reader_buffers_and_pad_only_at_eof() {
    let source = include_str!("programs/review_field_buffer_boundaries.bas");
    let (output, _dir) = run_bas_with_tmpdir(source);
    assert_eq!(output, "85|0|8403\n");
}

#[test]
fn test_print_using_finite_extremes_match_console_and_file_output() {
    let (output, dir) = run_bas_with_tmpdir(include_str!("programs/broad_format_extremes.bas"));
    let expected = format!("1.00E-309\n4.94E-324\n1.80E+308\n%{:.2}\n", 1e308);
    assert_eq!(output, expected);
    assert_eq!(
        std::fs::read_to_string(dir.path().join("formatted.txt")).unwrap(),
        expected
    );
}

#[test]
fn test_sqlite_documented_example() {
    assert_eq!(
        run_file("tests/programs/sqlite.bas"),
        "FALSE\n5:1\n9007199254740993:café:TRUE\n00FF:TRUE\nINTEGER:INT64\nmissing:NULL\nFALSE\n1\n"
    );
}

#[test]
fn test_sqlite_triggers_repeated_bindings_and_materialized_results() {
    assert_eq!(
        run_bas(
            r#"
OPTION BASE 0
db = DBOPEN(":memory:")
CALL DBEXEC(db, "CREATE TABLE item(value)")
CALL DBEXEC(db, "CREATE TABLE audit(value)")
CALL DBEXEC(db, "CREATE TRIGGER added AFTER INSERT ON item BEGIN INSERT INTO audit VALUES(new.value); INSERT INTO audit VALUES(new.value); END;")
PRINT DBEXEC(db, "INSERT INTO item VALUES (?)", "first")
PRINT DBEXEC(db, "INSERT INTO item VALUES (?)", NULL())
PRINT DBEXEC(db, "CREATE INDEX audit_value ON audit(value)"); ":"; DBCHANGES(db)
snapshot = DBQUERY(db, "SELECT value FROM item ORDER BY rowid")
CALL DBEXEC(db, "DELETE FROM item")
PRINT DBROWCOUNT(snapshot)
WHILE DBNEXT(snapshot)
    PRINT DBVALUE(snapshot, 1)
WEND
FOR iteration = 1 TO 2
    value = "bound"
    IF iteration = 2 THEN value = NULL()
    result = DBQUERY(db, "SELECT :same AS a, :same AS b, ?3 AS c", value, "unused slot", BYTES(255))
    CALL DBNEXT(result)
    PRINT ISNULL(DBVALUE(result, 1)); ":"; DBVALUE(result, 1) = DBVALUE(result, 2); ":"; HEXENCODE$(DBVALUE(result, 3))
    CALL DBFREE(result)
NEXT iteration
result = DBQUERY(db, "SELECT COUNT(*) FROM audit")
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
CALL DBCLOSE(db)
"#
        ),
        "1\n1\n0:1\n2\nfirst\nNULL\nFALSE:-1:FF\nTRUE:TRUE:FF\n4\n"
    );
}

#[test]
fn test_sqlite_function_and_call_arguments_evaluate_once_in_order() {
    assert_eq!(
        run_bas(
            r#"
db = DBOPEN(":memory:")
trace$ = ""
CALL DBEXEC(db, "CREATE TABLE item(value)")
CALL DBEXEC(ConnectionArg(), SqlArg$(FALSE), ValueArg$())
PRINT trace$
trace$ = ""
result = DBQUERY(ConnectionArg(), SqlArg$(TRUE), ValueArg$())
PRINT trace$
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
CALL DBCLOSE(db)
FUNCTION ConnectionArg() AS INT64
    SHARED db, trace$
    trace$ = trace$ + "connection;"
    ConnectionArg = db
END FUNCTION
FUNCTION SqlArg$(BYVAL query AS BOOL)
    SHARED trace$
    trace$ = trace$ + "sql;"
    IF query THEN
        SqlArg$ = "SELECT value FROM item WHERE value = ?"
    ELSE
        SqlArg$ = "INSERT INTO item VALUES (?)"
    END IF
END FUNCTION
FUNCTION ValueArg$()
    SHARED trace$
    trace$ = trace$ + "value;"
    ValueArg$ = "saved"
END FUNCTION
"#
        ),
        "connection;sql;value;\nconnection;sql;value;\nsaved\n"
    );
}

#[test]
fn test_sqlite_crud_schema_and_bound_values_preserve_native_data() {
    let source = r#"
db = DBOPEN(":memory:")
PRINT DBEXEC(db, "CREATE TABLE item(id INTEGER PRIMARY KEY, name TEXT, data BLOB, enabled INTEGER, missing)")
payload$ = "café'; DROP TABLE item; --漢" + CHR$(0) + "tail"
large = CINT64("9007199254740993")
PRINT DBEXEC(db, "INSERT INTO item VALUES (?1, ?2, ?3, ?4, ?5)", large, payload$, BYTES(0, 128, 255), TRUE, NULL())
PRINT DBLASTID(db); ":"; TYPEOF$(DBLASTID(db)); ":"; DBCHANGES(db)
PRINT DBEXEC(db, "CREATE INDEX item_name ON item(name)")
CALL DBEXEC(db, "ALTER TABLE item ADD COLUMN score REAL DEFAULT 0")
CALL DBEXEC(db, "CREATE VIEW current_item AS SELECT * FROM item")
PRINT DBEXEC(db, "UPDATE item SET score = ?1 WHERE id = ?2 OR id = ?2", 1.25, large)
result = DBQUERY(db, "SELECT id, name, data, enabled, missing, score FROM current_item WHERE id = ?", large)
PRINT DBCOLUMNCOUNT(result); ":"; DBROWCOUNT(result); ":"; DBCOLUMNNAME$(result, 2)
PRINT DBNEXT(result)
PRINT DBVALUE(result, "ID"); ":"; TYPEOF$(DBVALUE(result, 1))
PRINT DBVALUE(result, "name") = payload$
PRINT HEXENCODE$(DBVALUE(result, 3)); ":"; CBOOL(DBVALUE(result, 4)); ":"; DBISNULL(result, 5)
PRINT DBVALUE(result, 6); ":"; DBCOLUMNTYPE$(result, 6); ":"; DBCOLUMNTYPE$(result, 5)
CALL DBFREE(result)
PRINT DBEXEC(db, "DELETE FROM item WHERE id = ?", large)
result = DBQUERY(db, "SELECT COUNT(*) FROM item")
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
CALL DBEXEC(db, "DROP VIEW current_item")
CALL DBEXEC(db, "DROP TABLE item")
CALL DBCLOSE(db)
"#;
    assert_eq!(
        run_bas(source),
        "0\n1\n9007199254740993:INT64:1\n0\n1\n6:1:name\nTRUE\n9007199254740993:INT64\n-1\n0080FF:TRUE:TRUE\n1.25:REAL:NULL\n1\n0\n"
    );
}

#[test]
fn test_sqlite_storage_classes_and_null_round_trip_are_distinct() {
    let source = r#"
db = DBOPEN(":memory:")
result = DBQUERY(db, "SELECT ?1 AS lo, ?2 AS hi, ?3 AS empty_text, ?4 AS empty_blob, ?5 AS missing, ?6 AS boolean, ?7 AS number", CINT64("-9223372036854775808"), CINT64("9223372036854775807"), "", BYTES(), NULL(), FALSE, 7)
CALL DBNEXT(result)
FOR column = 1 TO 7
    PRINT DBCOLUMNTYPE$(result, column); ":"; TYPEOF$(DBVALUE(result, column))
NEXT column
PRINT DBVALUE(result, 1); ":"; DBVALUE(result, 2)
PRINT LEN(DBVALUE(result, 3)); ":"; LEN(DBVALUE(result, 4)); ":"; DBISNULL(result, 3); ":"; DBISNULL(result, 4)
missing = DBVALUE(result, 5)
copy = DBQUERY(db, "SELECT ? IS NULL, typeof(?), typeof(?), typeof(?)", missing, CINT(1), CSNG(1), CBYTE(1))
CALL DBNEXT(copy)
PRINT DBVALUE(copy, 1); ":"; DBVALUE(copy, 2); ":"; DBVALUE(copy, 3); ":"; DBVALUE(copy, 4)
CALL DBCLOSE(db)
"#;
    assert_eq!(
        run_bas(source),
        "INTEGER:INT64\nINTEGER:INT64\nTEXT:STRING\nBLOB:BYTES\nNULL:NULL\nINTEGER:INT64\nREAL:NUMERIC\n-9223372036854775808:9223372036854775807\n0:0:FALSE:FALSE\n1:integer:real:integer\n"
    );
}

#[test]
fn test_sqlite_transactions_savepoints_and_returning() {
    let source = r#"
db = DBOPEN(":memory:")
CALL DBEXEC(db, "CREATE TABLE item(id INTEGER PRIMARY KEY, name TEXT)")
PRINT DBINTRANSACTION(db)
CALL DBBEGIN(db, "immediate")
PRINT DBINTRANSACTION(db)
CALL DBEXEC(db, "INSERT INTO item VALUES (1, 'kept')")
CALL DBEXEC(db, "SAVEPOINT nested")
CALL DBEXEC(db, "INSERT INTO item VALUES (2, 'discarded')")
CALL DBEXEC(db, "ROLLBACK TO nested")
CALL DBEXEC(db, "RELEASE nested")
CALL DBCOMMIT(db)
PRINT DBINTRANSACTION(db)
CALL DBBEGIN(db)
CALL DBEXEC(db, "DELETE FROM item")
CALL DBROLLBACK(db)
CALL DBBEGIN(db, "exclusive")
result = DBQUERY(db, "INSERT INTO item(name) VALUES (?) RETURNING id, name", "returned")
PRINT DBROWCOUNT(result); ":"; DBCHANGES(db)
CALL DBNEXT(result)
PRINT DBVALUE(result, 1); ":"; DBVALUE(result, 2)
CALL DBCOMMIT(db)
CALL DBFREE(result)
result = DBQUERY(db, "SELECT id, name FROM item ORDER BY id")
WHILE DBNEXT(result)
    PRINT DBVALUE(result, 1); ":"; DBVALUE(result, 2)
WEND
CALL DBCLOSE(db)
"#;
    assert_eq!(
        run_bas(source),
        "FALSE\nTRUE\nFALSE\n1:1\n2:returned\n1:kept\n2:returned\n"
    );
}

#[test]
fn test_sqlite_foreign_keys_constraint_errors_and_lock_timeout_are_catchable() {
    let (output, _dir) = run_bas_with_tmpdir(
        r#"
db = DBOPEN("{DIR}/shared.sqlite", "rwc", 0)
other = DBOPEN("{DIR}/shared.sqlite", "rw", 0)
CALL DBEXEC(db, "CREATE TABLE parent(id INTEGER PRIMARY KEY)")
CALL DBEXEC(db, "CREATE TABLE child(parent_id INTEGER REFERENCES parent(id))")
WHEN EXCEPTION IN
    CALL DBEXEC(db, "INSERT INTO child VALUES (99)")
USE
    PRINT "foreign key:"; EXTYPE
END WHEN
CALL DBEXEC(db, "INSERT INTO parent VALUES (1)")
CALL DBEXEC(db, "INSERT INTO child VALUES (1)")
CALL DBBEGIN(db, "IMMEDIATE")
WHEN EXCEPTION IN
    CALL DBEXEC(other, "INSERT INTO parent VALUES (2)")
USE
    PRINT "locked:"; EXTYPE; ":"; DBINTRANSACTION(db)
END WHEN
CALL DBROLLBACK(db)
CALL DBTIMEOUT(other, 1)
PRINT DBEXEC(other, "INSERT INTO parent VALUES (2)")
CALL DBCLOSE(db)
CALL DBCLOSE(other)
"#,
    );
    assert_eq!(output, "foreign key:9000\nlocked:9000:TRUE\n1\n");
    assert_eq!(
        run_bas(
            "db = DBOPEN(\":memory:\")\nON ERROR GOTO handler\nCALL DBEXEC(db, \"SELECT * FROM absent\")\nEND\nhandler:\nPRINT ERR\n"
        ),
        "90\n"
    );
}

#[test]
fn test_sqlite_file_modes_and_close_roll_back_uncommitted_changes() {
    let (output, dir) = run_bas_with_tmpdir(
        r#"
db = DBOPEN("{DIR}/persistent.sqlite")
CALL DBEXEC(db, "CREATE TABLE item(value)")
CALL DBEXEC(db, "INSERT INTO item VALUES (1)")
CALL DBBEGIN(db)
CALL DBEXEC(db, "INSERT INTO item VALUES (2)")
CALL DBCLOSE(db)
db = DBOPEN("{DIR}/persistent.sqlite", "ro")
result = DBQUERY(db, "SELECT SUM(value) FROM item")
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
WHEN EXCEPTION IN
    CALL DBEXEC(db, "INSERT INTO item VALUES (3)")
USE
    PRINT "read only:"; EXTYPE
END WHEN
CALL DBCLOSE(db)
WHEN EXCEPTION IN
    db = DBOPEN("{DIR}/missing.sqlite", "rw")
USE
    PRINT "missing rw:"; EXTYPE
END WHEN
WHEN EXCEPTION IN
    db = DBOPEN("{DIR}/missing.sqlite", "ro")
USE
    PRINT "missing ro:"; EXTYPE
END WHEN
"#,
    );
    assert_eq!(
        output,
        "1\nread only:9000\nmissing rw:9000\nmissing ro:9000\n"
    );
    assert!(!dir.path().join("missing.sqlite").exists());
}

#[test]
fn test_sqlite_connections_and_results_persist_between_runs_and_drop_rolls_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir
        .path()
        .join("drop.sqlite")
        .to_str()
        .unwrap()
        .replace('\\', "/");
    let output = SharedOutput::new();
    {
        let mut interpreter = rice::interpreter::Interpreter::with_io(
            Box::new(output.clone()),
            Box::new(Cursor::new(Vec::<u8>::new())),
        );
        interpreter
            .run_source(&format!(
                "db = DBOPEN(\"{path}\")\nCALL DBEXEC(db, \"CREATE TABLE item(value)\")\nCALL DBEXEC(db, \"INSERT INTO item VALUES (1), (2)\")\nresult = DBQUERY(db, \"SELECT value FROM item ORDER BY value\")\n"
            ))
            .unwrap();
        interpreter
            .run_source("PRINT DBNEXT(result); \":\"; DBVALUE(result, 1)\n")
            .unwrap();
        interpreter
            .run_source("PRINT DBNEXT(result); \":\"; DBVALUE(result, 1)\nCALL DBBEGIN(db)\nCALL DBEXEC(db, \"DELETE FROM item\")\n")
            .unwrap();
    }
    assert_eq!(output.into_string(), "TRUE:1\nTRUE:2\n");
    assert_eq!(
        run_bas(&format!(
            "db = DBOPEN(\"{path}\", \"ro\")\nresult = DBQUERY(db, \"SELECT COUNT(*) FROM item\")\nCALL DBNEXT(result)\nPRINT DBVALUE(result, 1)\n"
        )),
        "2\n"
    );
}

#[test]
fn test_sqlite_clear_closes_resources_without_reusing_old_handles() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir
        .path()
        .join("clear.sqlite")
        .to_str()
        .unwrap()
        .replace('\\', "/");
    let output = SharedOutput::new();
    let mut interpreter = rice::interpreter::Interpreter::with_io(
        Box::new(output.clone()),
        Box::new(Cursor::new(Vec::<u8>::new())),
    );
    interpreter
        .run_source(&format!(
            "db = DBOPEN(\"{path}\")\nCALL DBEXEC(db, \"CREATE TABLE item(value)\")\nCALL DBBEGIN(db)\nCALL DBEXEC(db, \"INSERT INTO item VALUES (1)\")\nresult = DBQUERY(db, \"SELECT value FROM item\")\nPRINT db\nPRINT result\n"
        ))
        .unwrap();
    let initial = output.clone().into_string();
    let handles: Vec<i64> = initial.lines().map(|line| line.parse().unwrap()).collect();
    interpreter
        .run_source(&format!(
            "CLEAR\ndb = DBOPEN(\"{path}\")\nPRINT db > CINT64({})\nWHEN EXCEPTION IN\nCALL DBCLOSE({})\nUSE\nPRINT \"old database\"\nEND WHEN\nWHEN EXCEPTION IN\nCALL DBNEXT({})\nUSE\nPRINT \"old result\"\nEND WHEN\nresult = DBQUERY(db, \"SELECT COUNT(*) FROM item\")\nCALL DBNEXT(result)\nPRINT DBVALUE(result, 1)\n",
            handles[1], handles[0], handles[1]
        ))
        .unwrap();
    assert_eq!(
        output.into_string(),
        format!("{initial}TRUE\nold database\nold result\n0\n")
    );
}

#[test]
fn test_sqlite_empty_results_metadata_and_cursor_boundaries() {
    let source = r#"
db = DBOPEN(":memory:")
result = DBQUERY(db, "SELECT 1 AS Answer, NULL AS Missing WHERE 0")
PRINT DBROWCOUNT(result); ":"; DBCOLUMNCOUNT(result); ":"; DBCOLUMNNAME$(result, 1)
PRINT DBNEXT(result); ":"; DBNEXT(result)
WHEN EXCEPTION IN
    PRINT DBVALUE(result, 1)
USE
    PRINT "empty row"
END WHEN
CALL DBFREE(result)
result = DBQUERY(db, "SELECT 7 AS Value, 8 AS VALUE")
WHEN EXCEPTION IN
    PRINT DBVALUE(result, 1)
USE
    PRINT "before first"
END WHEN
CALL DBNEXT(result)
PRINT DBVALUE(result, 1); ":"; DBVALUE(result, 2)
WHEN EXCEPTION IN
    PRINT DBVALUE(result, "value")
USE
    PRINT "ambiguous column"
END WHEN
PRINT DBNEXT(result); ":"; DBNEXT(result)
WHEN EXCEPTION IN
    PRINT DBCOLUMNTYPE$(result, 1)
USE
    PRINT "after last"
END WHEN
CALL DBCLOSE(db)
"#;
    assert_eq!(
        run_bas(source),
        "0:2:Answer\nFALSE:FALSE\nempty row\nbefore first\n7:8\nambiguous column\nFALSE:FALSE\nafter last\n"
    );
}

#[test]
fn test_sqlite_freed_closed_and_wrong_kind_handles_are_rejected() {
    for operation in [
        "CALL DBNEXT(db)",
        "CALL DBFREE(db)",
        "CALL DBCLOSE(result)",
        "CALL DBEXEC(result, \"CREATE TABLE wrong(value)\")",
        "CALL DBFREE(result)\nCALL DBNEXT(result)",
        "CALL DBCLOSE(db)\nCALL DBNEXT(result)",
        "CALL DBCLOSE(db)\nCALL DBCHANGES(db)",
        "CALL DBCLOSE(db)\nnewdb = DBOPEN(\":memory:\")\nCALL DBCLOSE(db)",
        "CALL DBFREE(result)\nnewresult = DBQUERY(db, \"SELECT 2\")\nCALL DBNEXT(result)",
    ] {
        let source =
            format!("db = DBOPEN(\":memory:\")\nresult = DBQUERY(db, \"SELECT 1\")\n{operation}\n");
        let error = run_bas_may_fail(&source).1.unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<rice::error::RuntimeError>(),
                Some(rice::error::RuntimeError::IllegalFunctionCall { .. })
            ),
            "{operation}: {error}"
        );
    }
}

#[test]
fn test_sqlite_statement_validation_prevents_unintended_writes() {
    for operation in [
        "CALL DBEXEC(db, \"INSERT INTO item VALUES (?)\")",
        "CALL DBEXEC(db, \"INSERT INTO item VALUES (?)\", 1, 2)",
        "CALL DBEXEC(db, \"INSERT INTO item VALUES (?)\", VAL(\"NaN\"))",
        "CALL DBEXEC(db, \"INSERT INTO item VALUES (1) RETURNING value\")",
        "CALL DBQUERY(db, \"INSERT INTO item VALUES (1)\")",
        "CALL DBEXEC(db, \"INSERT INTO item VALUES (1); INSERT INTO item VALUES (2)\")",
        "CALL DBQUERY(db, \"INSERT INTO item VALUES (1) RETURNING value; SELECT 2\")",
        "CALL DBEXEC(db, \"INSERT INTO item VALUES (1)\" + CHR$(0) + \"; DELETE FROM item\")",
        "CALL DBQUERY(db, \"INSERT INTO item VALUES (1) RETURNING value\", 7)",
    ] {
        let source = format!(
            "db = DBOPEN(\":memory:\")\nCALL DBEXEC(db, \"CREATE TABLE item(value)\")\nWHEN EXCEPTION IN\n{operation}\nUSE\nPRINT \"rejected\"\nEND WHEN\nresult = DBQUERY(db, \"SELECT COUNT(*) FROM item\")\nCALL DBNEXT(result)\nPRINT DBVALUE(result, 1)\n"
        );
        assert_eq!(run_bas(&source), "rejected\n0\n", "{operation}");
    }
}

#[test]
fn test_sqlite_statement_shape_changes_are_checked_before_writing() {
    assert_eq!(
        run_bas(
            r#"
db = DBOPEN(":memory:")
CALL DBEXEC(db, "CREATE TABLE item(value)")
CALL DBEXEC(db, "INSERT INTO item VALUES (1)")
CALL DBEXEC(db, "PRAGMA count_changes=ON")
WHEN EXCEPTION IN
    CALL DBEXEC(db, "INSERT INTO item VALUES (1)")
USE
    PRINT "exec rejected"
END WHEN
result = DBQUERY(db, "SELECT COUNT(*) FROM item")
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
result = DBQUERY(db, "INSERT INTO item VALUES (1)")
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
CALL DBEXEC(db, "PRAGMA count_changes=OFF")
WHEN EXCEPTION IN
    result = DBQUERY(db, "INSERT INTO item VALUES (1)")
USE
    PRINT "query rejected"
END WHEN
PRINT DBEXEC(db, "INSERT INTO item VALUES (1)")
result = DBQUERY(db, "SELECT COUNT(*) FROM item")
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
"#
        ),
        "exec rejected\n1\n1\nquery rejected\n1\n3\n"
    );
}

#[test]
fn test_sqlite_rejected_pragmas_do_not_poison_later_execution() {
    assert_eq!(
        run_bas(
            r#"
db = DBOPEN(":memory:")
WHEN EXCEPTION IN
    result = DBQUERY(db, "PRAGMA cache_size=100")
USE
    PRINT "shape rejected"
END WHEN
CALL DBEXEC(db, "PRAGMA cache_size=200")
CALL DBEXEC(db, "PRAGMA cache_size=100")
result = DBQUERY(db, "PRAGMA cache_size")
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
WHEN EXCEPTION IN
    CALL DBEXEC(db, "PRAGMA synchronous=OFF", 99)
USE
    PRINT "parameters rejected"
END WHEN
CALL DBEXEC(db, "PRAGMA synchronous=FULL")
CALL DBEXEC(db, "PRAGMA synchronous=OFF")
result = DBQUERY(db, "PRAGMA synchronous")
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
WHEN EXCEPTION IN
    CALL DBEXEC(db, "INSERT INTO missing_table VALUES (1)")
USE
    PRINT EXTYPE
END WHEN
CALL DBEXEC(db, "CREATE TABLE missing_table(value)")
PRINT DBEXEC(db, "INSERT INTO missing_table VALUES (1)")
"#
        ),
        "shape rejected\n100\nparameters rejected\n0\n9000\n1\n"
    );
}

#[test]
fn test_sqlite_exec_counts_direct_dml_and_returns_zero_for_schema_writes() {
    assert_eq!(
        run_bas(
            r#"
db = DBOPEN(":memory:")
PRINT DBEXEC(db, "CREATE VIRTUAL TABLE documents USING fts5(body)")
PRINT DBEXEC(db, "INSERT INTO documents VALUES ('first'), ('second')")
PRINT DBEXEC(db, "UPDATE documents SET body = 'changed' WHERE rowid = 1")
PRINT DBEXEC(db, "DELETE FROM documents WHERE rowid = 2")
CALL DBEXEC(db, "CREATE TABLE parent(id INTEGER PRIMARY KEY)")
CALL DBEXEC(db, "CREATE TABLE child(parent_id INTEGER REFERENCES parent(id) ON DELETE CASCADE)")
CALL DBEXEC(db, "INSERT INTO parent VALUES (1)")
CALL DBEXEC(db, "INSERT INTO child VALUES (1), (1)")
PRINT DBEXEC(db, "DROP TABLE parent")
result = DBQUERY(db, "SELECT COUNT(*) FROM child")
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
CALL DBEXEC(db, "CREATE TABLE item(id INTEGER PRIMARY KEY, value)")
CALL DBEXEC(db, "CREATE TABLE audit(value)")
CALL DBEXEC(db, "CREATE TRIGGER added AFTER INSERT ON item BEGIN INSERT INTO audit VALUES (new.value); END")
PRINT DBEXEC(db, "INSERT INTO item(value) VALUES (10), (20)")
PRINT DBEXEC(db, "WITH seed(value) AS (VALUES (30), (40)) INSERT INTO item(value) SELECT value FROM seed")
PRINT DBEXEC(db, "REPLACE INTO item(id, value) VALUES (1, 100)")
PRINT DBEXEC(db, "UPDATE item SET value = value + 1")
PRINT DBEXEC(db, "DELETE FROM item WHERE id >= 3")
PRINT DBEXEC(db, "CREATE INDEX item_value ON item(value)")
result = DBQUERY(db, "SELECT COUNT(*) FROM audit")
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
PRINT DBEXEC(db, "ANALYZE")
PRINT DBEXEC(db, "VACUUM")
"#
        ),
        "0\n2\n1\n1\n0\n0\n2\n2\n1\n4\n2\n0\n5\n0\n0\n"
    );
}

#[test]
fn test_sqlite_invalid_column_names_are_catchable_with_and_without_rows() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid-metadata.sqlite");
    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE item(rice_invalid_utf8_column INTEGER);
                 INSERT INTO item VALUES (7);
                 CREATE VIEW computed AS SELECT 7 + 1 AS rice_invalid_utf8_alias;
                 CREATE VIRTUAL TABLE documents USING fts5(body);
                 INSERT INTO documents VALUES ('SQLite metadata')",
            )
            .unwrap();
    }
    // SQLite permits identifiers containing non-UTF-8 bytes. Replace just one
    // byte of each unique schema name, preserving every page offset. A computed
    // view alias has no origin column, so origin metadata cannot validate it.
    let mut contents = std::fs::read(&path).unwrap();
    for marker in [
        b"rice_invalid_utf8_column".as_slice(),
        b"rice_invalid_utf8_alias".as_slice(),
    ] {
        let offsets: Vec<_> = contents
            .windows(marker.len())
            .enumerate()
            .filter_map(|(index, window)| (window == marker).then_some(index))
            .collect();
        assert_eq!(offsets.len(), 1);
        contents[offsets[0]] = 0xff;
    }
    std::fs::write(&path, contents).unwrap();

    let source = format!(
        r#"
db = DBOPEN("{}")
result = DBQUERY(db, "SELECT body FROM documents WHERE documents MATCH 'sqlite'")
PRINT DBCOLUMNNAME$(result, 1)
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
WHEN EXCEPTION IN
    result = DBQUERY(db, "SELECT * FROM item")
USE
    PRINT EXTYPE
END WHEN
WHEN EXCEPTION IN
    result = DBQUERY(db, "SELECT * FROM item WHERE 0")
USE
    PRINT EXTYPE
END WHEN
WHEN EXCEPTION IN
    result = DBQUERY(db, "SELECT * FROM computed")
USE
    PRINT EXTYPE
END WHEN
WHEN EXCEPTION IN
    result = DBQUERY(db, "SELECT * FROM computed WHERE 0")
USE
    PRINT EXTYPE
END WHEN
result = DBQUERY(db, "SELECT COUNT(*) FROM item")
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
result = DBQUERY(db, "SELECT COUNT(*) FROM documents WHERE documents MATCH 'sqlite'")
CALL DBNEXT(result)
PRINT DBVALUE(result, 1)
"#,
        path.to_str().unwrap().replace('\\', "/")
    );
    assert_eq!(
        run_bas(&source),
        "body\nSQLite metadata\n9000\n9000\n9000\n9000\n1\n1\n"
    );
}

#[test]
fn test_sqlite_invalid_arguments_and_column_references_are_catchable() {
    for call in [
        "DBOPEN()",
        "DBOPEN(\":memory:\", \"rwc\", 0, 1)",
        "DBOPEN(\":memory:\", \"invalid\")",
        "DBOPEN(\"bad\" + CHR$(0) + \"path\")",
        "DBOPEN(123)",
        "DBOPEN(\":memory:\", \"rwc\", -1)",
        "DBOPEN(\":memory:\", \"rwc\", 2147483648)",
        "DBTIMEOUT(db, 0.5)",
        "DBTIMEOUT(db, -1)",
        "DBTIMEOUT(db, 2147483648)",
        "DBTIMEOUT(db, \"1\")",
        "DBBEGIN(db, \"invalid\")",
        "DBCOMMIT(db)",
        "DBROLLBACK(db)",
        "DBEXEC(db)",
        "DBEXEC(db, \"\")",
        "DBEXEC(db, 123)",
        "DBQUERY(db)",
        "DBQUERY(db, \"SELECT ?\")",
        "DBQUERY(db, \"SELECT ?\", record)",
        "DBVALUE(result)",
        "DBVALUE(result, 0)",
        "DBVALUE(result, 2)",
        "DBVALUE(result, 1.5)",
        "DBVALUE(result, \"unknown\")",
        "DBCOLUMNNAME$(result, 0)",
        "DBCOLUMNNAME$(result, \"value\")",
        "DBCHANGES(db, 1)",
        "DBNEXT(result, 1)",
        "DBVERSION$(1)",
        "DBNEXT(1.5)",
    ] {
        let source = format!(
            "TYPE Item\nvalue AS INTEGER\nEND TYPE\nDIM record AS Item\ndb = DBOPEN(\":memory:\")\nresult = DBQUERY(db, \"SELECT 1 AS value\")\nCALL DBNEXT(result)\nWHEN EXCEPTION IN\nignored = {call}\nUSE\nPRINT \"caught\"\nEND WHEN\n"
        );
        assert_eq!(run_bas(&source), "caught\n", "{call}");
    }
}

#[test]
fn test_sqlite_invalid_text_is_catchable_and_blob_cast_preserves_bytes() {
    assert_eq!(
        run_bas(
            r#"
db = DBOPEN(":memory:")
WHEN EXCEPTION IN
    result = DBQUERY(db, "SELECT CAST(x'80FF' AS TEXT)")
USE
    PRINT EXTYPE
END WHEN
result = DBQUERY(db, "SELECT CAST(CAST(x'80FF' AS TEXT) AS BLOB)")
CALL DBNEXT(result)
PRINT HEXENCODE$(DBVALUE(result, 1))
CALL DBCLOSE(db)
"#
        ),
        "9000\n80FF\n"
    );
}

#[test]
fn test_sqlite_null_semantics_and_declared_types_remain_explicit() {
    assert_eq!(
        run_bas(
            "missing = NULL()\nPRINT missing; \":\"; TYPEOF$(missing); \":\"; ISNULL(missing)\nPRINT missing = NULL(); \":\"; missing <> NULL(); \":\"; missing = 0; \":\"; missing <> FALSE; \":\"; ISNULL(\"\")\n"
        ),
        "NULL:NULL:TRUE\nTRUE:FALSE:FALSE:TRUE:FALSE\n"
    );
    for operation in [
        "IF NULL() THEN PRINT \"wrong\"",
        "PRINT NOT NULL()",
        "PRINT NULL() + 1",
        "PRINT NULL() < 1",
        "PRINT NULL() AND TRUE",
        "PRINT NULL() & \"text\"",
        "DIM value AS NUMERIC\nvalue = NULL()",
        "DIM value AS INTEGER\nvalue = NULL()",
        "DIM value AS LONG\nvalue = NULL()",
        "DIM value AS INT64\nvalue = NULL()",
        "DIM value AS SINGLE\nvalue = NULL()",
        "DIM value AS BOOL\nvalue = NULL()",
        "DIM value AS BYTE\nvalue = NULL()",
        "DIM value AS BYTES\nvalue = NULL()",
        "DIM value AS STRING\nvalue = NULL()",
    ] {
        let (_, result) = run_bas_may_fail(&format!("{operation}\n"));
        let error = result.unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<rice::error::RuntimeError>(),
                Some(rice::error::RuntimeError::TypeMismatch { .. })
            ),
            "{operation}: {error}"
        );
    }
}

#[test]
fn test_select_case_null_supports_equality_but_rejects_ordering() {
    assert_eq!(
        run_bas(
            r#"
SELECT CASE NULL()
    CASE NULL()
        PRINT "null value"
END SELECT
SELECT CASE NULL()
    CASE IS = 0
        PRINT "wrong"
    CASE IS = NULL()
        PRINT "null equality"
END SELECT
SELECT CASE NULL()
    CASE IS <> NULL()
        PRINT "wrong"
    CASE IS <> 0
        PRINT "null inequality"
END SELECT
SELECT CASE 1
    CASE IS = NULL()
        PRINT "wrong"
    CASE IS <> NULL()
        PRINT "value inequality"
END SELECT
"#
        ),
        "null value\nnull equality\nnull inequality\nvalue inequality\n"
    );

    for (subject, case) in [
        ("NULL()", "NULL() TO NULL()"),
        ("NULL()", "1 TO 2"),
        ("1", "NULL() TO 2"),
        ("1", "0 TO NULL()"),
        ("NULL()", "IS < 1"),
        ("NULL()", "IS <= NULL()"),
        ("NULL()", "IS > 1"),
        ("NULL()", "IS >= NULL()"),
        ("1", "IS < NULL()"),
        ("1", "IS <= NULL()"),
        ("1", "IS > NULL()"),
        ("1", "IS >= NULL()"),
    ] {
        let source = format!("SELECT CASE {subject}\nCASE {case}\nPRINT \"wrong\"\nEND SELECT\n");
        let (output, result) = run_bas_may_fail(&source);
        assert!(output.is_empty(), "{source}: {output}");
        let error = result.unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<rice::error::RuntimeError>(),
                Some(rice::error::RuntimeError::TypeMismatch { .. })
            ),
            "{source}: {error}"
        );
    }
}
