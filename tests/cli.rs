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

fn expect_success(output: Output, arguments: &str) {
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
        format!("[{arguments}]\n[{arguments}]\n-1\n")
    );
}

#[test]
fn cli_command_arguments_exclude_filename() {
    let path = fixture();
    expect_success(run(&[&path]), "");
    expect_success(run(&[&path, "Alpha", "second.bas"]), "Alpha second.bas");
}

#[test]
fn cli_command_arguments_preserve_spaces_unicode_and_empty_fields() {
    let path = fixture();
    expect_success(
        run(&[&path, "", "Mixed Case", "é😀", ""]),
        " Mixed Case é😀 ",
    );
    expect_success(run(&[&path, ""]), "");
    expect_success(run(&[&path, "left", "", "right"]), "left  right");
    expect_success(run(&[&path, "say \"hello\""]), "say \"hello\"");
}

#[test]
fn cli_separator_passes_dash_arguments_without_parsing_them() {
    let path = fixture();
    expect_success(
        run(&[&path, "--", "--dialect", "ansi", "--compat", "--", "-x"]),
        "--dialect ansi --compat -- -x",
    );
    expect_success(run(&["--", &path, "--compat"]), "--compat");

    let directory = tempfile::tempdir().unwrap();
    std::fs::copy(&path, directory.path().join("-program.bas")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rice"))
        .current_dir(directory.path())
        .args(["--", "-program.bas", "value"])
        .output()
        .unwrap();
    expect_success(output, "value");
}

#[test]
fn cli_removed_and_unknown_options_fail_before_the_separator() {
    let path = fixture();
    for option in ["--compat", "-compat", "--dialect", "-dialect", "--unknown"] {
        for args in [
            vec![option],
            vec![option, "qb", &path],
            vec![&path, "arg", option],
        ] {
            let output = run(&args);
            assert_eq!(output.status.code(), Some(1), "{args:?}");
            assert!(output.stdout.is_empty(), "{args:?}");
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains(&format!("unknown option: {option}")),
                "{args:?}"
            );
        }
    }
}

#[test]
fn cli_rejects_removed_source_directives_in_check_and_execution() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("removed-option.bas");
    for name in ["QB", "ANSI", "QBasic 1.1", "unknown"] {
        std::fs::write(
            &path,
            format!("PRINT \"must not execute\"\nOPTION DIALECT \"{name}\"\n"),
        )
        .unwrap();
        let execution = run(&[path.to_str().unwrap()]);
        let check = run(&["--check", path.to_str().unwrap()]);
        assert_eq!(execution.status.code(), Some(1));
        assert_eq!(check.status.code(), Some(1));
        assert!(execution.stdout.is_empty());
        assert!(check.stdout.is_empty());
        assert_eq!(execution.stderr, check.stderr);
        assert!(String::from_utf8_lossy(&execution.stderr).contains("OPTION"));
    }
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
fn cli_check_accepts_default_language_features() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("language.bas");
    std::fs::write(&path, "OPTION EXPLICIT\nDIM value%\nvalue% = 7 \\ 2\nPRINT value%; 2 AND 1\nGOSUB 100\nEND\n100 RETURN\n").unwrap();
    let output = run(&["--check", path.to_str().unwrap()]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
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
    expect_success(run(&[&path, "--", "--check"]), "--check");

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

#[cfg(any(unix, windows))]
#[test]
fn cli_sqlite_uri_shaped_filenames_are_literal_files() {
    let directory = tempfile::tempdir().unwrap();
    let filename = "file:literal.sqlite?mode=memory&cache=shared";
    let source = format!(
        r#"
WHEN EXCEPTION IN
    db = DBOPEN("{filename}")
    PRINT "opened"
    CALL DBEXEC(db, "CREATE TABLE item(value)")
    CALL DBEXEC(db, "INSERT INTO item VALUES (7)")
    CALL DBCLOSE(db)
    db = DBOPEN("{filename}", "ro")
    result = DBQUERY(db, "SELECT value FROM item")
    CALL DBNEXT(result)
    PRINT DBVALUE(result, 1)
    CALL DBCLOSE(db)
USE
    PRINT EXTYPE
END WHEN
"#
    );
    std::fs::write(directory.path().join("literal-filename.bas"), source).unwrap();
    // Isolate the working directory in the child, without changing the test
    // process's directory while other integration tests execute in parallel.
    let output = Command::new(env!("CARGO_BIN_EXE_rice"))
        .current_dir(directory.path())
        .arg("literal-filename.bas")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout)
        .unwrap()
        .replace("\r\n", "\n");
    #[cfg(unix)]
    {
        assert_eq!(stdout, "opened\n7\n");
        assert!(directory.path().join(filename).is_file());
    }
    #[cfg(windows)]
    {
        // Windows may accept this literal path as an NTFS alternate stream,
        // or reject it on another filesystem. URI interpretation is forbidden:
        // mode=memory would lose the table after close and print opened\n9000\n.
        assert!(
            matches!(stdout.as_str(), "opened\n7\n" | "9000\n"),
            "unexpected database output: {stdout:?}"
        );
    }
    if stdout == "opened\n7\n" {
        // Verify persistence at the exact literal path, including stream paths
        // on Windows, without relying on platform-specific metadata behavior.
        let mut file = std::fs::File::open(directory.path().join(filename)).unwrap();
        let mut header = [0; 16];
        std::io::Read::read_exact(&mut file, &mut header).unwrap();
        assert_eq!(&header, b"SQLite format 3\0");
    }
    assert!(!directory.path().join("literal.sqlite").exists());
}
