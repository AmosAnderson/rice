# CLAUDE.md

Guidance for coding assistants working in this repository. Read [AGENTS.md](AGENTS.md) first; its contribution rules apply.

## Rules

- Do not commit, amend, push, or otherwise change git history without explicit user approval.
- Preserve existing interpreter semantics unless the requested change explicitly calls for changing them. When behavior changes, update its documentation and appropriate regression coverage.
- Do not infer full QBasic or ANSI support from a keyword, a code comment, or the dialect name. Check parser and runtime behavior.

## Project and commands

Rice BASIC 0.14.0 is a hand-written tree-walking interpreter with QBasic compatibility defaults and an ANSI-style mode. There is no compiler backend, graphics API, or sound API beyond the terminal bell. See [Cargo.toml](Cargo.toml) for the version, Rust edition 2024, and actual dependencies (`thiserror`, `rustyline`, `crossterm`, `tower-lsp`, `tokio`; dev dependency `tempfile`).

```sh
cargo build
cargo run
cargo run -- program.bas
cargo run -- --dialect ansi program.bas
cargo build --bin rice-lsp
cargo test
cargo test --lib
cargo test --test integration
cargo test --bin rice-lsp
cargo test test_hello
```

The CLI runs on an 8 MiB stack thread because large debug interpreter frames can exhaust a smaller stack. `Interpreter::run_source` is the shared execution entry point for source strings and REPL submissions. `run_file` reads UTF-8 source and calls it; it does not change the working directory.

## Source navigation

| File | Responsibility / change point |
|---|---|
| `src/lib.rs` | Public modules, default dialect, whole-source directive detection, keyboard and screen helpers. |
| `src/token.rs`, `src/lexer.rs` | Tokens/spans, uppercase names, comments/literals, suffix rules, compound keywords. |
| `src/ast.rs`, `src/parser.rs` | Statement/expression AST and recursive-descent grammar; operator precedence is in the [reference](docs/language-reference.md). |
| `src/interpreter.rs` | Prescan, execution, call frames, control-flow propagation, stateful builtins, files, console, exceptions. |
| `src/environment.rs` | Parent scope reads, local/shared writes, constants/declarations, labels. Default array base is 1 in both modes. |
| `src/value.rs` | Two primitive values (`Numeric(f64)`, `Str`) plus aggregate `Record`; type metadata, conversions, formatting, binary byte mapping. |
| `src/builtins.rs` | Pure builtin registry. Exact and variadic arities use separate registration methods. |
| `src/mat.rs` | Two-dimensional numeric matrix operations. |
| `src/format_using.rs` | PRINT USING numeric/string formatting. |
| `src/repl.rs` | Immediate execution, block-depth heuristic, stored numbered lines, RUN/LIST/NEW/DELETE, highlighting/history. |
| `src/main.rs` | CLI parsing and interpreter thread. |
| `src/error.rs` | Lexer/parser/runtime errors and host I/O code mapping. |
| `src/bin/rice_lsp.rs` | Stdio LSP diagnostics, completions, hover strings, current-document definitions. |

Prefer extending these existing files over introducing new modules. Keep names normalized to uppercase; full QB suffixes are part of storage keys. Arrays use flattened keys: avoid a broad storage refactor without a task and tests specifically requiring it.

## Behavioral details to preserve and verify

- `=` is assignment at statement level and comparison inside expressions. Parentheses remain explicit AST nodes because they suppress BYREF writeback.
- Function-shaped syntax resolves interpreter functions, registry functions, user-defined functions, then arrays. Bare zero-argument builtin spellings vary; see [builtins](docs/builtins.md).
- `run_source` detects dialect before lexing, parses, then prescans DATA, labels, types, SUB, and FUNCTION definitions before execution. Prescan recurses eligible control blocks, not into procedure definitions; calls do not independently prescan nested definitions/DATA.
- Control-flow values propagate through `exec_block`; preserve loop exits, GOTO, GOSUB/RETURN continuations, END, RETRY/CONTINUE, and classic RESUME behavior. Label targets for each procedure are separate from caller targets.
- QB defaults to BYREF and bitwise numeric logic with true `-1`; ANSI defaults to BYVAL and logical operations with true `1`. Both use array base 1 unless changed. String `+` is QB-only; `&` works in both modes.
- Only `$` is a string suffix. Numeric suffixes distinguish QB names without enforcing numeric widths in memory. Ordinary assignments are permissive; defaults, OPTION EXPLICIT, UDT metadata, and I/O conversions impose distinct rules. Do not conflate declaration metadata with strict type checking.
- PRINT has no positive-number padding and uses 16-character comma zones. Console tracking is separate from terminal dimensions; consult [console](docs/console.md).
- FILE functions including FREEFILE, EOF, LOF, LOC, SEEK and INPUT$ are in `interpreter.rs`. QB and ANSI GET/PUT have different implementations. See [file I/O](docs/file-io.md).
- REPL immediate state persists; RUN replaces the interpreter while retaining the dialect, and NEW only clears stored program lines. See [runtime](docs/runtime.md).

The authoritative documentation inventory is [docs/README.md](docs/README.md). Check the [compatibility register](docs/compatibility.md) for partial features and known gaps before extending behavior.

## Tests and maintenance

Integration assertions are in `tests/integration.rs`, with fixtures in `tests/programs/*.bas`. Helpers include `run_file(path)`, `run_bas(source)`, `run_bas_with_tmpdir(source)` (substitutes `{DIR}`), and `run_bas_may_fail(source)`. `SharedOutput` captures PRINT output. Tests that use `Interpreter::with_io` can provide controlled input and noninteractive output.

Use focused tests for the changed behavior, and run required checks. TIMER, DATE$, TIME$, and RND are nondeterministic without overrides/seeding; assert shape, range, or relationships instead of wall-clock values. Unit/integration success is a regression result, not a dialect conformance certificate.

For a new statement, update token/lexer, AST/parser, execution, any required prescan/control-flow handling, documentation, and useful tests. For a new pure builtin, register exact arity with `register(name, function, count)` or variable arity with `register_variadic`; **arity 0 means exactly zero arguments**, not variadic. Stateful builtins belong in interpreter dispatch and may need parser spelling support. Update LSP documentation strings if the feature is exposed there.

When modifying errors, inspect both `error.rs` host mappings and interpreter exception-code handling. When modifying documented behavior, update the owning topic guide and syntax/core/dialect indexes as needed; avoid maintaining contradictory copies of the full specification.
