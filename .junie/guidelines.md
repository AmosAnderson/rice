# Junie Guidelines for RICE BASIC

## Project Overview
RICE BASIC is an ANSI X3.113-1991 Full BASIC interpreter written in Rust. It supports interactive REPL and file execution. No graphics or sound.

## Build & Test
- `cargo build` — build the project
- `cargo test` — run all tests (unit + integration)
- `cargo test --lib` — unit tests only
- `cargo test --test integration` — integration tests only
- `cargo test <test_name>` — run a single test by name

Rust edition 2024. Dependencies: `thiserror`, `rustyline`, `crossterm`, `tower-lsp`/`tokio`/`serde_json`, Dev: `pretty_assertions`, `tempfile`.

## Architecture
Source → Lexer → Tokens → Parser → AST → Tree-Walking Interpreter → Output

Key modules: `token.rs`, `lexer.rs`, `ast.rs`, `parser.rs`, `interpreter.rs`, `environment.rs`, `value.rs`, `builtins.rs`, `repl.rs`, `error.rs`, `main.rs`.

## Code Style
- All identifiers stored UPPERCASE internally
- Hand-written lexer and recursive descent parser (no parser generators)
- `Rc<RefCell<Environment>>` scope chain for variables
- `ControlFlow` enum for non-local control flow (GOTO, EXIT, END, etc.). GOSUB/RETURN are not supported.
- ANSI BASIC-style PRINT formatting (no leading space for positive numbers, 16-char comma zones)
- Only two types: NUMERIC (f64) and STRING. No type suffixes. Variables ending in `$` are strings; all others are numeric.

## Testing
- Integration tests in `tests/integration.rs` using `run_file()` or `run_bas()` helpers
- Test programs in `tests/programs/*.bas`
- `SharedOutput` captures PRINT output for assertions
- To add a test: create `.bas` file in `tests/programs/`, add test function in `tests/integration.rs`
