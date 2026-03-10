# Project Guidelines

## Code Style
- Follow existing Rust style: `snake_case` for functions/fields, `CamelCase` for types/enums.
- Keep parser/interpreter logic explicit with `match` and `Result`-based error propagation.
- Preserve case-insensitive BASIC behavior and uppercase identifier normalization.
- Maintain module boundaries in `src/lib.rs`; avoid unnecessary public API churn.
- Prefer extending existing files (`src/parser.rs`, `src/interpreter.rs`, `src/value.rs`, `src/builtins.rs`) over adding new modules/abstractions.

## Architecture
- Core pipeline: Source → Lexer → Tokens → Parser → AST → Tree-walking Interpreter → Output. All hand-written (no parser generators).
- `Interpreter::run_source` is the execution entry point; it pre-scans labels, DATA, SUB/FUNCTION, and DEF FN definitions before execution. Prescan recurses into nested blocks (IF, FOR, WHILE, DO, SELECT CASE); preserve this ordering.
- Control transfer (`GOTO`, `GOSUB`, `RETURN`, `EXIT*`, `END`) relies on `ControlFlow` enum variants bubbling up through `exec_block()`.
- Environment keying is name + type suffix (`X%` and `X$` are distinct) in `src/environment.rs`. Scopes use `Rc<RefCell<Environment>>` chain.
- Builtins are centralized in `src/builtins.rs`; resolution order: builtin → user-defined function → array.
- `=` disambiguation: at statement level `=` is assignment; inside expressions `=` is comparison.
- Expression parsing uses precedence climbing: IMP → EQV → XOR → OR → AND → NOT → comparison → +/- → MOD → \\ → */÷ → unary → ^.

## Build and Test
- Build: `cargo build`
- Build LSP: `cargo build --bin rice-lsp`
- Start REPL: `cargo run`
- Run a BASIC file: `cargo run -- file.bas`
- Full tests: `cargo test`
- Unit tests only: `cargo test --lib`
- Integration tests only: `cargo test --test integration`
- Single test by name: `cargo test test_hello`
- Rust edition 2024. Dependencies: `thiserror`, `rustyline`, `tower-lsp`/`tokio`/`serde_json`. Dev: `pretty_assertions`, `tempfile`.

### Integration Test Helpers
- `run_file("tests/programs/foo.bas")` — load and execute a `.bas` file, returns captured output.
- `run_bas("PRINT 42\n")` — parse/execute inline BASIC source.
- `run_bas_with_tmpdir(src)` — execute with a temp directory, use `{DIR}` placeholder in source for paths. Returns `(output, TempDir)`.
- `run_bas_may_fail(src)` — returns both output and `Result` for testing error conditions.
- Fixture pattern: add `.bas` under `tests/programs/`, then add a test function in `tests/integration.rs`.

## Extending the Interpreter

### Adding a new statement
1. Add `Token::Kw*` variant to `src/token.rs`.
2. Add `"KEYWORD" => Token::KwKeyword` entry in the lexer's `match word.as_str()` table in `src/lexer.rs`.
3. Add `Stmt::*` variant to the enum in `src/ast.rs`.
4. Add `Token::Kw* => self.parse_*()` case in `parse_statement()` in `src/parser.rs`.
5. Add `Stmt::* => ...` case in `exec_stmt()` in `src/interpreter.rs`. Simple statements return `ControlFlow::Normal`; control flow statements return the appropriate `ControlFlow` variant.
6. If the statement needs prescan (labels, data, definitions), add handling in the prescan phase.

### Adding a builtin function
1. Write `fn builtin_name(args: &[Value]) -> Result<Value, RuntimeError>` in `src/builtins.rs`.
2. Call `reg.register("NAME", builtin_name, arity)` in `BuiltinRegistry::new()`. Use arity `0` for variadic.
3. Use `args[n].to_f64()?`, `.to_i64()?`, `.to_string_val()?` for type coercion; return `Value::Integer`, `Value::Double`, `Value::Str`, etc.

### Adding a new error
1. Add variant to `LexError`, `ParseError`, or `RuntimeError` in `src/error.rs` with `#[error(...)]` attribute.
2. For `RuntimeError`: add QBasic error code mapping in `qbasic_error_code()` if applicable.

## Project Conventions
- BASIC truth values are numeric: true = `-1`, false = `0`; do not change casually.
- `PRINT` formatting follows QBasic behavior (leading space for positive numbers, comma zones).
- Single-line vs block `IF` is parser-sensitive; `ELSE` is treated as statement terminator in relevant contexts.
- Undefined variables auto-initialize by suffix (`0` for numeric, `""` for string).
- Arrays are currently implemented with flattened keys; avoid broad refactors without targeted tests.
- Not yet implemented: proper array storage, LBOUND/UBOUND (stubs only).

## Security
- Interpreter executes untrusted BASIC source without built-in time/resource limits.
- CLI reads files directly from user-provided path; avoid introducing hidden side effects.
- Time/date-related builtins can be nondeterministic; preserve deterministic behavior where tests require it.
