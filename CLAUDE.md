# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rice is a structured BASIC interpreter written in Rust (QBasic/FreeBASIC dialect). No graphics or sound support. Supports both interactive REPL and file execution.

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

Rust edition 2024 (`Cargo.toml`). Uses `thiserror` for error types, `rustyline` for REPL, `pretty_assertions` for test diffs.

## Architecture

Classic interpreter pipeline: `Source → Lexer → Tokens → Parser → AST → Tree-Walking Interpreter → Output`

All hand-written (no parser generators).

### Module Map

- **`token.rs`** — Token enum, TypeSuffix (`% & ! # $`), Span. All identifiers stored UPPERCASE.
- **`lexer.rs`** — Hand-written tokenizer. Case-insensitive. Detects line numbers at line start. Recognizes compound keywords (`END IF`, `END SUB`, `LINE INPUT`). Attaches type suffixes to identifiers.
- **`ast.rs`** — `Stmt` and `Expr` enums. `LabeledStmt` wraps statements with optional line labels. Key types: `PrintStmt`, `IfStmt`, `ForStmt`, `DoLoopStmt`, `SelectCaseStmt`, `SubDef`, `FunctionDef`.
- **`parser.rs`** — Recursive descent. Expression parsing uses precedence climbing (IMP → EQV → XOR → OR → AND → NOT → comparison → +/- → MOD → \\ → */÷ → unary → ^). `at_stmt_end()` also treats `ELSE` as a terminator for single-line IF support.
- **`interpreter.rs`** — Tree-walking evaluator. Uses `ControlFlow` enum (Normal, ExitFor, ExitDo, ExitSub, ExitFunction, Goto, Gosub, Return, End) for control flow. `SharedOutput` wrapper enables testable output capture.
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

### Test Programs

Integration tests in `tests/programs/*.bas` cover: hello world, arithmetic, variables, FizzBuzz, while loops, do/loops, select case, gosub/return, recursive factorial, string functions, DATA/READ, SUB calls.

To add a new integration test: create a `.bas` file in `tests/programs/`, then add a test function in `tests/integration.rs` using the `run_file()` helper (or `run_bas()` for inline source). The interpreter's `SharedOutput` captures PRINT output for assertion.

## Status of BASIC Features

**Working**: PRINT, LET, DIM, CONST, INPUT, LINE INPUT, IF/ELSEIF/ELSE, FOR/NEXT, WHILE/WEND, DO/LOOP, SELECT CASE, GOTO, GOSUB/RETURN, EXIT FOR/DO/SUB/FUNCTION, SUB/FUNCTION definitions, CALL, DECLARE, DATA/READ/RESTORE, SWAP, all string/math/conversion builtins, OPTION BASE, REDIM, ERASE.

**Stubbed (parsing works, runtime not implemented)**: File I/O (OPEN/CLOSE/PRINT#/INPUT#), ON ERROR GOTO, PRINT USING, GET/PUT.

**Not implemented**: TYPE (user-defined types), DEFtype statements, ON n GOTO/GOSUB, proper array storage (currently uses flattened key hack), SHARED/STATIC/BYVAL semantics, LSET/RSET.
