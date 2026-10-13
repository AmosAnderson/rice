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
