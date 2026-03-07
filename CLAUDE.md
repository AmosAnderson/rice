# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

RICE BASIC is a structured BASIC interpreter written in Rust (QBasic/FreeBASIC dialect). No graphics or sound support. Supports both interactive REPL and file execution.

## Build & Test Commands

```bash
cargo build                    # Build
cargo run                     # Start REPL
cargo run -- file.bas          # Execute a .bas file
cargo test                    # Run all tests (unit + integration)
cargo test --lib              # Run unit tests only
cargo test --test integration  # Run integration tests only
cargo test test_hello          # Run a single test by name
```

Rust edition 2024 (`Cargo.toml`). Uses `thiserror` for error types, `rustyline` for REPL, `pretty_assertions` and `tempfile` for tests, `tower-lsp`/`tokio`/`serde_json` for the LSP server.

There is also an LSP binary:
```bash
cargo build --bin rice-lsp     # Build the language server (stdio-based)
```

REPL and file execution share interpreter code paths; keep behavior parity between them.

## Architecture

Classic interpreter pipeline: `Source → Lexer → Tokens → Parser → AST → Tree-Walking Interpreter → Output`

All hand-written (no parser generators).

### Module Map

- **`token.rs`** — Token enum, TypeSuffix (`% & ! # $`), Span. All identifiers stored UPPERCASE.
- **`lexer.rs`** — Hand-written tokenizer. Case-insensitive. Detects line numbers at line start. Recognizes compound keywords (`END IF`, `END SUB`, `LINE INPUT`). Attaches type suffixes to identifiers.
- **`ast.rs`** — `Stmt` and `Expr` enums. `LabeledStmt` wraps statements with optional line labels. Key types: `PrintStmt`, `IfStmt`, `ForStmt`, `DoLoopStmt`, `SelectCaseStmt`, `SubDef`, `FunctionDef`.
- **`parser.rs`** — Recursive descent. Expression parsing uses precedence climbing (IMP → EQV → XOR → OR → AND → NOT → comparison → +/- → MOD → \\ → */÷ → unary → ^). `at_stmt_end()` also treats `ELSE` as a terminator for single-line IF support.
- **`interpreter.rs`** — Tree-walking evaluator. Uses `ControlFlow` enum (Normal, ExitFor, ExitDo, ExitSub, ExitFunction, Goto, Gosub, Return, End, Resume, ResumeNext) for control flow. `SharedOutput` wrapper enables testable output capture. `FileHandle` struct manages open files with `BufReader`/`BufWriter` for text and binary I/O. Error handler state (error_handler, current_error, error_resume_pc) enables ON ERROR GOTO/RESUME. ERR and ERL are resolved as interpreter-state functions.
- **`format_using.rs`** — PRINT USING format engine. Supports QBasic numeric specifiers (`#`, `.`, `+`, `-`, `$$`, `**`, `**$`, `,`, `^^^^`) and string specifiers (`!`, `\ \`, `&`). Escape with `_`. Overflow prefix `%`.
- **`environment.rs`** — `Rc<RefCell<Environment>>` scope chain. Variable key = name + suffix (`X%` and `X$` are different variables). GOSUB return stack and label map stored here.
- **`value.rs`** — `Value` enum (Integer, Long, Single, Double, Str). QBasic-style PRINT formatting (leading space for positive numbers). Type coercion ladder: Integer < Long < Single < Double.
- **`builtins.rs`** — Built-in function registry. Math (ABS, INT, SQR, SIN, etc.), string (LEFT$, MID$, LEN, etc.), conversion (CINT, VAL, STR$, etc.).
- **`repl.rs`** — Interactive REPL using rustyline. Environment persists across lines.
- **`error.rs`** — `LexError`, `ParseError`, `RuntimeError` enums via `thiserror`. These are the public error types returned through the pipeline.
- **`main.rs`** — CLI: no args → REPL, one arg → execute file.

### Key Design Decisions

- **`=` disambiguation**: at statement level `=` is assignment; inside expressions `=` is comparison
- **Single-line vs block IF**: if tokens follow THEN on the same line, it's single-line
- **Auto-initialization**: undefined variables auto-initialize to 0 or "" (classic BASIC behavior)
- **`name(args)` ambiguity**: resolved at runtime — check builtin registry, then user functions, then arrays
- **GOTO/GOSUB**: label map built during prescan; ControlFlow::Goto bubbles up to exec_block which resolves it
- **Truth values**: true = `-1`, false = `0` (QBasic convention); do not change
- **Prescan ordering**: `Interpreter::run_source` pre-scans labels, DATA, SUB/FUNCTION definitions before execution; preserve this ordering

### Code Conventions

- Follow existing Rust style: `snake_case` for functions/fields, `CamelCase` for types/enums. Use `match` and `Result`-based error propagation.
- Prefer extending existing files (`parser.rs`, `interpreter.rs`, `value.rs`, `builtins.rs`) over adding new modules/abstractions.
- Module boundaries are declared in `src/lib.rs`; avoid unnecessary public API churn.
- All identifiers are normalized to UPPERCASE internally; preserve case-insensitive BASIC behavior.
- Arrays are currently implemented with flattened keys; avoid broad refactors without targeted tests.
- Builtins are centralized in `builtins.rs`; function resolution order: builtin → user-defined function → array.

### Test Programs

Integration tests in `tests/programs/*.bas` cover: hello world, arithmetic, variables, FizzBuzz, while loops, do/loops, select case, gosub/return, recursive factorial, string functions, DATA/READ, SUB calls, file I/O (text, binary, append, WRITE#/INPUT# round-trip, FREEFILE, EOF, LOF).

To add a new integration test: create a `.bas` file in `tests/programs/`, then add a test function in `tests/integration.rs` using the `run_file()` helper (or `run_bas()` for inline source). The interpreter's `SharedOutput` captures PRINT output for assertion.

## Status of BASIC Features

**Working**: PRINT, PRINT USING, LET, DIM, CONST, INPUT, LINE INPUT, IF/ELSEIF/ELSE, FOR/NEXT, WHILE/WEND, DO/LOOP, SELECT CASE, GOTO, GOSUB/RETURN, EXIT FOR/DO/SUB/FUNCTION, SUB/FUNCTION definitions, CALL, DECLARE, DATA/READ/RESTORE, SWAP, all string/math/conversion builtins, ERR/ERL, OPTION BASE, REDIM, ERASE, File I/O (OPEN, CLOSE, PRINT#, WRITE#, INPUT#, LINE INPUT#, GET, PUT), file functions (FREEFILE, EOF, LOF, LOC), ON ERROR GOTO/RESUME, ON n GOTO/GOSUB, RANDOMIZE/RND, WRITE (console), SLEEP, CLEAR, NAME/KILL/MKDIR/RMDIR/CHDIR, SHELL, ENVIRON$, MID$ (statement form), LSET/RSET, SHARED, STATIC, DEFtype (DEFINT/DEFLNG/DEFSNG/DEFDBL/DEFSTR), DEF FN, MKI$/MKL$/MKS$/MKD$/CVI/CVL/CVS/CVD.

**Not implemented**: TYPE (user-defined types), proper array storage (currently uses flattened key hack), BYVAL semantics, FIELD (random-access file fields), SEEK statement/function, LBOUND/UBOUND (stubs only).
