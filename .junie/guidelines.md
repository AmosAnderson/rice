# Junie Guidelines for Rice

## Project Overview
Rice is a QBasic/FreeBASIC dialect BASIC interpreter written in Rust. It supports interactive REPL and file execution. No graphics or sound.

## Build & Test
- `cargo build` — build the project
- `cargo test` — run all tests (unit + integration)
- `cargo test --lib` — unit tests only
- `cargo test --test integration` — integration tests only
- `cargo test <test_name>` — run a single test by name

Rust edition 2024. Dependencies: `thiserror`, `rustyline`, `pretty_assertions`.

## Architecture
Pipeline: Source → Lexer → Tokens → Parser → AST → Tree-Walking Interpreter → Output

Key modules: `token.rs`, `lexer.rs`, `ast.rs`, `parser.rs`, `interpreter.rs`, `environment.rs`, `value.rs`, `builtins.rs`, `repl.rs`, `error.rs`, `main.rs`.

## Code Style
- All identifiers stored UPPERCASE internally
- Hand-written lexer and recursive descent parser (no parser generators)
- `Rc<RefCell<Environment>>` scope chain for variables
- `ControlFlow` enum for non-local control flow (GOTO, GOSUB, EXIT, etc.)
- QBasic-style PRINT formatting (leading space for positive numbers)
- Type suffix convention: `X%` (integer), `X&` (long), `X!` (single), `X#` (double), `X$` (string)

## Testing
- Integration tests in `tests/integration.rs` using `run_file()` or `run_bas()` helpers
- Test programs in `tests/programs/*.bas`
- `SharedOutput` captures PRINT output for assertions
- To add a test: create `.bas` file in `tests/programs/`, add test function in `tests/integration.rs`
