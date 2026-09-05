# RICE BASIC

Rice BASIC 0.14.0 is a text-mode BASIC interpreter written in Rust, with an interactive REPL, file execution, and a stdio language server. It defaults to **QBasic compatibility mode** and also provides an **ANSI-style mode** inspired by ANSI X3.113-1991 Full BASIC.

These are Rice dialects, not claims of complete QBasic 1.1 or ANSI conformance. Both share extensions and implementation limits. The documentation specifies the behavior implemented in this repository, including accepted syntax that has partial or no runtime effect.

## Start here

```sh
cargo build
cargo run                         # REPL, QBasic mode
cargo run -- program.bas          # Execute a file
cargo run -- --dialect ansi program.bas
cargo test
```

A Rust toolchain supporting edition 2024 is required. For optimized binaries, use `cargo build --release`. No graphics or sound APIs are implemented beyond the terminal bell.

```basic
OPTION DIALECT "QB"
FOR i = 1 TO 3
    PRINT "Hello, BASIC!"
NEXT i
```

Select ANSI with `OPTION DIALECT "ANSI"` or `--dialect ansi`. Explicit QB selections include `--compat`, `--dialect qb`, `--dialect qbasic`, `--dialect quickbasic`, `OPTION DIALECT "QB"`, and `OPTION DIALECT "QBasic 1.1"`. A recognized source directive selects the dialect for the whole source unit. See [dialect rules](docs/dialects.md).

## Documentation

The [documentation index](docs/README.md) links the complete specification and guides:

- [Quick start](docs/quickstart.md): runnable examples and everyday use.
- [Language reference](docs/language-reference.md) and [syntax index](SYNTAX.md): lexical rules, types, operators, statements, arrays, control flow, and DATA.
- [Dialect comparison](docs/dialects.md) and [compatibility gaps and unknowns](docs/compatibility.md): exact differences, partial features, and unverified behavior.
- [Built-in functions](docs/builtins.md): complete signatures, argument rules, conversions, strings, random numbers, and interpreter state.
- [Procedures and scope](docs/procedures.md), [user-defined types](docs/user-defined-types.md), and [module boundaries](docs/multi-module.md).
- [File I/O](docs/file-io.md), [errors](docs/error-handling.md), [console](docs/console.md), and [PRINT USING](docs/print-using.md).
- [MAT operations](docs/mat-operations.md) and [string slicing](docs/string-slicing.md).
- [CLI, REPL, host operations, and editor integration](docs/runtime.md).

Both modes support structured control flow, procedures, records, arrays, text files, string slicing, MAT, and structured exceptions. QB additionally enables classic GOSUB/RETURN, computed ON branches, and classic error-handler syntax, and changes truth values, logical operators, string `+`, default parameter passing, and binary record behavior.

Porting details that matter immediately: arrays default to **base 1 in both modes**; numeric values are stored as `f64` regardless of suffix; PRINT uses 16-character comma zones and no leading/trailing positive-number space; assignment types and array bounds are not strictly enforced. Read the compatibility guide before relying on historical BASIC behavior.

## REPL and editor support

Unnumbered input executes immediately; numbered lines build a stored program:

```text
10 PRINT "Hello"
20 PRINT "Again"
RUN
LIST
```

`RUN` starts the stored program with a fresh interpreter. `LIST`, `NEW`, and `DELETE` manage stored lines. Complete blocks can also be entered directly. The [runtime guide](docs/runtime.md) documents command ranges, state persistence, exit behavior, and limitations.

Build the language server with `cargo build --release --bin rice-lsp`. It provides parser diagnostics, completions, hover documentation, and current-document go-to-definition over stdio. See [editor setup and limits](docs/runtime.md#language-server).

## Implementation and development

The execution pipeline is hand-written and interpreter-only:

```text
Source -> Lexer -> Tokens -> Parser -> AST -> Tree-walking interpreter -> Output
```

| Source | Responsibility |
|---|---|
| `src/token.rs`, `src/lexer.rs` | Tokens, source spans, case-insensitive lexing. |
| `src/ast.rs`, `src/parser.rs` | Syntax tree and recursive-descent parsing. |
| `src/interpreter.rs` | Execution, procedure calls, file handles, console state, errors. |
| `src/environment.rs`, `src/value.rs` | Scope lookup, flattened array storage, primitive values and records. |
| `src/builtins.rs` | Pure builtin registry; stateful functions live in the interpreter. |
| `src/mat.rs`, `src/format_using.rs` | Matrix arithmetic and formatted printing. |
| `src/repl.rs`, `src/main.rs` | REPL/editor and CLI. |
| `src/lib.rs`, `src/error.rs` | Public modules, dialect detection, console helpers, error definitions. |
| `src/bin/rice_lsp.rs` | Stdio language server. |

Tests include unit tests and BASIC fixtures in `tests/programs`, asserted by `tests/integration.rs`. Focused commands: `cargo test --lib`, `cargo test --test integration`, and `cargo test --bin rice-lsp`. See [AGENTS.md](AGENTS.md) for contribution instructions and [CLAUDE.md](CLAUDE.md) for a source-navigation guide.

Dependencies are declared in [Cargo.toml](Cargo.toml): `thiserror`, `rustyline`, `crossterm`, `tower-lsp`, and `tokio`, with `tempfile` for tests.

## License

[MIT](LICENSE).
