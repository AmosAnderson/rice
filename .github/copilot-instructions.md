# Project Guidelines

## Code Style
- Follow existing Rust style: `snake_case` for functions/fields, `CamelCase` for types/enums.
- Keep parser/interpreter logic explicit with `match` and `Result`-based error propagation.
- Preserve case-insensitive BASIC behavior and uppercase identifier normalization.
- Maintain module boundaries in `src/lib.rs`; avoid unnecessary public API churn.
- Prefer extending existing files for behavior (`src/parser.rs`, `src/interpreter.rs`, `src/value.rs`) over adding new abstractions.

## Architecture
- Core pipeline is lexer → parser → AST → tree-walking interpreter; keep `Interpreter::run_source` as the execution entry.
- `src/interpreter.rs` pre-scans labels, DATA, SUB/FUNCTION definitions before execution; preserve this ordering.
- Control transfer (`GOTO`, `GOSUB`, `RETURN`, `EXIT*`, `END`) relies on `ControlFlow` bubbling through block execution.
- Environment keying is name + type suffix (`X%` and `X$` are distinct) in `src/environment.rs`.
- Builtins are centralized in `src/builtins.rs`; function resolution prefers builtin first, then user-defined function.

## Build and Test
- Build: `cargo build`
- Start REPL: `cargo run`
- Run a BASIC file: `cargo run -- file.bas`
- Full tests: `cargo test`
- Unit tests only: `cargo test --lib`
- Integration tests only: `cargo test --test integration`
- Single test by name: `cargo test test_hello`
- Integration fixture pattern: add `.bas` under `tests/programs/` and wire a test in `tests/integration.rs`.

## Project Conventions
- BASIC truth values are numeric: true = `-1`, false = `0`; do not change casually.
- `PRINT` formatting follows QBasic behavior (leading space for positive numbers, comma zones).
- Single-line vs block `IF` is parser-sensitive; `ELSE` is treated as statement terminator in relevant contexts.
- Undefined variables auto-initialize by suffix (`0` for numeric, `""` for string).
- Arrays are currently implemented with flattened keys; avoid broad refactors without targeted tests.
- Some features are intentionally partial/stubbed: file I/O runtime, `ON ERROR GOTO`, `PRINT USING`, `GET/PUT`.

## Integration Points
- Dependencies are intentionally minimal: `thiserror`, `rustyline`, `pretty_assertions`.
- REPL and file execution share interpreter code paths; keep behavior parity.
- Test output assertions rely on `SharedOutput` capture in `src/interpreter.rs`.

## Security
- Interpreter executes untrusted BASIC source without built-in time/resource limits.
- CLI reads files directly from user-provided path; avoid introducing hidden side effects.
- Time/date-related builtins can be nondeterministic; preserve deterministic behavior where tests require it.
