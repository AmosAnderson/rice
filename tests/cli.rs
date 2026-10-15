use std::process::{Command, Output};

fn fixture() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/programs/command_line.bas")
        .to_str()
        .unwrap()
        .to_owned()
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rice"))
        .args(args)
        .output()
        .unwrap()
}

fn expect_success(output: Output, arguments: &str, truth: i32) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "{:?}", output.stderr);
    assert_eq!(
        String::from_utf8(output.stdout)
            .unwrap()
            .replace("\r\n", "\n"),
        format!("[{arguments}]\n[{arguments}]\n{truth}\n")
    );
}

#[test]
fn cli_command_arguments_exclude_filename_and_dialect_options() {
    let path = fixture();
    expect_success(run(&[&path]), "", -1);
    expect_success(run(&[&path, "Alpha", "second.bas"]), "Alpha second.bas", -1);
    expect_success(
        run(&[
            "--dialect",
            "ansi",
            &path,
            "Mixed",
            "--compat",
            "Case",
            "--dialect",
            "ANSI",
        ]),
        "Mixed Case",
        1,
    );
    expect_success(
        run(&["--dialect", "ansi", &path, "--compat", "tail"]),
        "tail",
        -1,
    );
}

#[test]
fn cli_command_arguments_preserve_spaces_unicode_and_empty_fields() {
    let path = fixture();
    expect_success(
        run(&[&path, "", "Mixed Case", "é😀", ""]),
        " Mixed Case é😀 ",
        -1,
    );
    expect_success(run(&[&path, ""]), "", -1);
    expect_success(run(&[&path, "left", "", "right"]), "left  right", -1);
    expect_success(run(&[&path, "say \"hello\""]), "say \"hello\"", -1);
}

#[test]
fn cli_separator_passes_dash_arguments_without_parsing_them() {
    let path = fixture();
    expect_success(
        run(&[&path, "--", "--dialect", "ansi", "--compat", "--", "-x"]),
        "--dialect ansi --compat -- -x",
        -1,
    );
    expect_success(
        run(&["--dialect", "ansi", "--", &path, "--compat"]),
        "--compat",
        1,
    );

    let directory = tempfile::tempdir().unwrap();
    std::fs::copy(&path, directory.path().join("-program.bas")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rice"))
        .current_dir(directory.path())
        .args(["--", "-program.bas", "value"])
        .output()
        .unwrap();
    expect_success(output, "value", -1);
}

#[test]
fn cli_invalid_options_still_fail_before_the_separator() {
    let path = fixture();
    for (args, message) in [
        (vec!["--dialect"], "--dialect requires an argument"),
        (
            vec![path.as_str(), "arg", "--dialect"],
            "--dialect requires an argument",
        ),
        (
            vec!["--dialect", "unknown", path.as_str()],
            "unknown dialect: unknown",
        ),
        (
            vec![path.as_str(), "--dialect", "--"],
            "unknown dialect: --",
        ),
        (
            vec![path.as_str(), "arg", "--unknown"],
            "unknown option: --unknown",
        ),
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{args:?}"
        );
    }
}

#[test]
fn cli_source_dialect_override_keeps_program_arguments() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("override.bas");
    std::fs::write(
        &path,
        format!(
            "OPTION DIALECT \"QB\"\n{}",
            std::fs::read_to_string(fixture()).unwrap()
        ),
    )
    .unwrap();
    expect_success(
        run(&["--dialect", "ansi", path.to_str().unwrap(), "payload"]),
        "payload",
        -1,
    );
}

#[test]
fn cli_check_accepts_syntax_without_output_or_runtime_effects() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("check.bas");
    let created_file = directory.path().join("must-not-exist.txt");
    std::fs::write(
        &path,
        format!(
            "PRINT \"must not print\"\nOPEN \"{}\" FOR OUTPUT AS #1\n\
             PRINT #1, \"must not write\"\nCLOSE #1\nPRINT 1 / 0\nGOTO missing_label\n",
            created_file.display()
        ),
    )
    .unwrap();
    for args in [
        vec!["--check", path.to_str().unwrap()],
        vec![path.to_str().unwrap(), "--check", "payload"],
        vec!["--check", "--", path.to_str().unwrap(), "--unknown"],
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        assert!(!created_file.exists());
    }
}

#[test]
fn cli_check_reports_the_same_lex_and_parse_errors_as_execution() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid.bas");
    for (source, message) in [
        ("PRINT \"unterminated\n", "unterminated string literal"),
        ("IF 1 THEN\nPRINT 42\n", "expected KwEndIf"),
    ] {
        std::fs::write(&path, source).unwrap();
        let output = run(&["--check", path.to_str().unwrap()]);
        let execution = run(&[path.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, execution.stderr);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn cli_check_respects_cli_and_source_dialects() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("dialect.bas");
    let qb_source = "GOSUB 100\nEND\n100 RETURN\n";
    std::fs::write(&path, qb_source).unwrap();
    let path_arg = path.to_str().unwrap();
    for options in [
        vec!["--check", path_arg],
        vec!["--dialect", "qb", "--check", path_arg],
        vec!["--dialect", "ansi", "--check", path_arg, "--compat"],
    ] {
        let output = run(&options);
        assert!(output.status.success(), "{options:?}");
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    let ansi_args = ["--check", "--dialect", "ansi", path_arg];
    let output = run(&ansi_args);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("GOSUB is not supported"));

    std::fs::write(&path, format!("OPTION DIALECT \"QB\"\n{qb_source}")).unwrap();
    let output = run(&ansi_args);
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());

    std::fs::write(&path, format!("OPTION DIALECT \"ANSI\"\n{qb_source}")).unwrap();
    let output = run(&["--check", "--dialect", "qb", path_arg]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("GOSUB is not supported"));
}

#[test]
fn cli_check_requires_a_readable_source_file() {
    for args in [vec!["--check"], vec!["--check", "--"]] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("--check requires a source file"));
    }

    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.bas");
    let invalid_utf8 = directory.path().join("invalid-utf8.bas");
    std::fs::write(&invalid_utf8, [0xff, 0xfe]).unwrap();
    for (path, message) in [(&missing, "Cannot open"), (&invalid_utf8, "Cannot read")] {
        let output = run(&["--check", path.to_str().unwrap()]);
        let execution = run(&[path.to_str().unwrap()]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, execution.stderr);
        assert!(String::from_utf8_lossy(&output.stderr).contains(message));
    }
}

#[test]
fn cli_check_after_separator_is_a_program_argument() {
    let path = fixture();
    expect_success(run(&[&path, "--", "--check"]), "--check", -1);

    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("--check"), "PRINT 42\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rice"))
        .current_dir(directory.path())
        .args(["--check", "--", "--check"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}
