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
- Runtime values include NUMERIC/DOUBLE `f64`, INTEGER `i16`, LONG `i32`, SINGLE `f32`, INT64 `i64`, BOOL, BYTE `u8`, BYTES, STRING, and records. All declared types are enforced across variables, arrays, fields, parameters, results, and persistent/shared bindings. Integer assignments require exact in-range values; fixed strings pad/truncate to their Unicode character length. Unsuffixed implicit variables remain dynamic unless DEFtype applies. See `docs/data-types.md`.

- Undefined variables initialize to the selected type's default unless OPTION EXPLICIT requires a declaration; preserve its documented declaration exceptions.
- Statement-level `=` is assignment; inside expressions it is comparison.
- `MOD` works on real numbers.
- PRINT has no leading space for positive numbers and uses 16-character comma zones.
- Arrays use flattened keys; avoid broad storage refactors unless the task and tests specifically cover them.
- Rice BASIC defines one language; its specification is authoritative.
- OPTION BASE defaults to 1.
- Suffixes `%`, `&`, `!`, `#`, `$` enforce INTEGER, LONG, SINGLE, DOUBLE, STRING unless AS overrides them. DIM/STATIC without AS use the suffix or NUMERIC. DEFtype establishes enforced types for newly initialized implicit bindings.
- Legacy numeric truth is -1/0; comparisons involving native values return BOOL.
- Both `+` and `&` concatenate strings or byte buffers.
- Numeric logical operators are bitwise; BOOL operands select boolean logic.
- Parameters default to BYREF. Parenthesized expressions suppress copy-back.
- GOSUB/RETURN, computed ON GOTO/GOSUB, and top-level ON ERROR/RESUME remain supported.
- Structured binary GET/PUT follows retained declarations and recursive UDT layouts. FIELD/LSET/RSET, packed conversions, and APPEND remain supported.

## Parser And Interpreter Gotchas

- `Interpreter::run_source` prescans labels, DATA, SUB, and FUNCTION definitions before execution; prescan recurses into eligible control blocks, not procedure bodies, and ordering matters. TYPE definitions are also collected. Calls do not independently prescan DATA or nested definitions.
- `GOTO`, `EXIT*`, `END`, `RETRY`, and `CONTINUE` propagate via `ControlFlow` through `exec_block()`.
- Single-line vs block `IF` depends on tokens after `THEN`; `ELSE` can terminate a single-line statement parse.
- Function-call syntax is ambiguous at parse time; runtime resolution is builtin, then user-defined function, then array.
- FREEFILE, EOF, LOF, and LOC are implemented directly in `src/interpreter.rs`, not the builtin registry.

## Tests

- Integration fixtures live in `tests/programs/*.bas`; add a fixture plus a `#[test]` in `tests/integration.rs` when behavior spans the parser/interpreter.
- Test helpers: `run_file("tests/programs/foo.bas")`, `run_bas("PRINT 42\n")`, `run_bas_with_tmpdir(src)` with `{DIR}`, and `run_bas_may_fail(src)` for expected runtime errors.
- `SharedOutput` captures PRINT output for assertions.
- TIMER, DATE$, TIME$, and RND are nondeterministic; assert shape or bounds instead of exact values.

## Documentation

- `docs/README.md` indexes the Rice language specification; `SYNTAX.md` is a compact syntax index.
- `docs/language-reference.md` owns core syntax/semantics; `docs/builtins.md` owns the complete function inventory. Topic guides cover files, errors, console, formatting, procedures, records, MAT, strings, and modules.
- `docs/compatibility.md` records implementation limits, ignored syntax, and remaining verification gaps.
- `docs/runtime.md` specifies CLI, REPL, host operations, library entry points, and LSP limits.
- When changing behavior, update its owning reference and relevant LSP hover/completion documentation. Describe accepted-but-ignored syntax explicitly and document only verified runtime behavior.
