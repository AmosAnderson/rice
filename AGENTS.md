# AGENTS.md

## Commands

- `cargo build` builds the interpreter; `cargo build --bin rice-lsp` builds the stdio LSP binary.
- `cargo run` starts the REPL; `cargo run -- file.bas` executes a BASIC file.
- `cargo test` runs unit and integration tests; use `cargo test --lib`, `cargo test --test integration`, or `cargo test test_hello` for focused runs.
- Do not commit, amend, push, or otherwise change git history without explicit user approval.

## Architecture

- Pipeline is hand-written and interpreter-only: `Source -> Lexer -> Tokens -> Parser -> AST -> Tree-walking Interpreter -> Output`.
- Real execution entrypoint is `Interpreter::run_source`; REPL and file execution should preserve behavior parity.
- CLI entrypoint is `src/main.rs`; it runs the interpreter on an 8 MB stack thread to avoid debug-mode stack exhaustion from large interpreter match frames.
- `src/bin/rice_lsp.rs` is a separate stdio LSP server binary.
- Prefer extending existing core files over adding new modules: tokens in `src/token.rs`, lexing in `src/lexer.rs`, AST in `src/ast.rs`, parsing in `src/parser.rs`, execution in `src/interpreter.rs`, builtins in `src/builtins.rs`, values in `src/value.rs`.

## BASIC Semantics To Preserve

- Identifiers are case-insensitive and normalized to UPPERCASE internally.
- Only two primitive types exist in memory: NUMERIC `f64` and STRING; variables ending in `$` are strings, all others numeric.
- Undefined variables auto-initialize to `0` or `""`.
- Statement-level `=` is assignment; inside expressions it is comparison.
- `MOD` works on real numbers.
- PRINT formatting intentionally has no leading space for positive numbers and uses 16-character comma zones.
- Arrays currently use flattened keys; avoid broad array-storage refactors unless the task and tests are specifically about that.
- **Dialect Semantics**: Rice BASIC supports QBasic 1.1 compatibility mode by default and ANSI mode via `OPTION DIALECT "ANSI"` or `--dialect ansi`. `OPTION DIALECT "QB"`, `OPTION DIALECT "QBasic 1.1"`, `--dialect qb`, and `--compat` explicitly select the default QBasic-compatible mode.
  - **ANSI Mode Semantics**:
    - Suffixes other than `$` are not supported.
    - Truth values are `1.0` (true) and `0.0` (false).
    - String concatenation is `&`, not `+`; `+` remains arithmetic.
    - Logical `AND`, `OR`, `NOT`, `XOR` are logical, not bitwise.
    - `OPTION BASE` defaults to 1.
    - Subroutine/function parameters are `BYVAL` by default.
    - `GOSUB`/`RETURN` and `ON GOTO/GOSUB` are not supported.
  - **QuickBasic Mode Semantics**:
    - Suffixes (`%`, `!`, `#`, `&`, `$`) are supported in variable names to distinguish different variables (which all hold `f64` or `String` values in memory).
    - Truth values are `-1.0` (true) and `0.0` (false).
    - String concatenation is `+` (for strings) or `&`.
    - Logical `AND`, `OR`, `NOT`, `XOR` perform bitwise operations on numeric values.
    - Subroutine/function parameters are `BYREF` by default. Passing a parenthesized argument (e.g. `MySub (x)`) forces it to be evaluated as an expression, passing it by value (`BYVAL`).
    - `GOSUB`/`RETURN` (using a return address stack) and `ON GOTO/GOSUB` are supported.
    - Classic top-level `ON ERROR GOTO`/`RESUME`, `ERROR`, `ERR`, and `ERL` are supported.
    - Structured binary record I/O (`GET`/`PUT` with recursive UDT field serialization), `FIELD`/`LSET`/`RSET`, packed MK*/CV* conversion functions, and `APPEND` file mode are supported.

## Parser And Interpreter Gotchas

- `Interpreter::run_source` prescans labels, DATA, SUB, and FUNCTION definitions before execution; prescan recurses into nested blocks and ordering matters.
- `GOTO`, `EXIT*`, `END`, `RETRY`, and `CONTINUE` propagate via `ControlFlow` through `exec_block()`.
- Single-line vs block `IF` depends on tokens after `THEN`; `ELSE` can terminate a single-line statement parse.
- Function-call syntax is ambiguous at parse time; runtime resolution is builtin, then user-defined function, then array.
- FREEFILE, EOF, LOF, and LOC are implemented directly in `src/interpreter.rs`, not the builtin registry.

## Tests

- Integration fixtures live in `tests/programs/*.bas`; add a fixture plus a `#[test]` in `tests/integration.rs` when behavior spans the parser/interpreter.
- Test helpers: `run_file("tests/programs/foo.bas")`, `run_bas("PRINT 42\n")`, `run_bas_with_tmpdir(src)` with `{DIR}`, and `run_bas_may_fail(src)` for expected runtime errors.
- `SharedOutput` captures PRINT output for assertions.
- TIMER, DATE$, TIME$, and RND are nondeterministic; assert shape or bounds instead of exact values.
