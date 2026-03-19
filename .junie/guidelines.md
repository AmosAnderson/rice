# Junie Guidelines for RICE BASIC

## Project Overview
RICE BASIC is a QBasic/FreeBASIC dialect BASIC interpreter and compiler written in Rust. It supports interactive REPL, file execution, and native compilation via Cranelift. No graphics or sound.

## Build & Test
- `cargo build` — build the project
- `cargo test` — run all tests (unit + integration)
- `cargo test --lib` — unit tests only
- `cargo test --test integration` — integration tests only
- `cargo test <test_name>` — run a single test by name

Rust edition 2024. Dependencies: `thiserror`, `rustyline`, `crossterm`, `tower-lsp`/`tokio`/`serde_json`, `cranelift-*`. Dev: `pretty_assertions`, `tempfile`.

## Architecture
Two execution paths from a shared frontend:
1. Interpreter: Source → Lexer → Tokens → Parser → AST → Tree-Walking Interpreter → Output
2. Compiler: Source → Lexer → Tokens → Parser → AST → RiceIR → Cranelift → Native Executable

Key modules: `token.rs`, `lexer.rs`, `ast.rs`, `parser.rs`, `interpreter.rs`, `environment.rs`, `value.rs`, `builtins.rs`, `repl.rs`, `error.rs`, `main.rs`, `compiler/` (ir, lowerer, codegen, linker), `runtime/` (value_ffi, io_ffi).

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
