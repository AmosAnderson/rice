# RICE BASIC

Rice BASIC 0.15.1 is a text-mode BASIC interpreter written in Rust, with an interactive REPL, file execution, and a stdio language server. It defaults to **QBasic compatibility mode** and also provides an **ANSI-style mode** inspired by ANSI X3.113-1991 Full BASIC.

These are Rice dialects, not claims of complete QBasic 1.1 or ANSI conformance. Both share extensions and implementation limits. The documentation specifies the behavior implemented in this repository, including accepted syntax that has partial or no runtime effect.

## Start here

Download prebuilt `rice` and `rice-lsp` binaries for Linux (x64 and ARM64), Windows, and macOS from [GitHub Releases](https://github.com/AmosAnderson/rice/releases). The Linux ARM64 archive supports compatible Raspberry Pi systems running a 64-bit OS. Each archive includes both executables, the documentation, and the MIT license. See [release downloads and publishing](docs/runtime.md#release-downloads-and-publishing) for platforms, checksums, and the tag-based release process.

To build from source:

```sh
cargo build
cargo run                         # REPL, QBasic mode
cargo run -- program.bas          # Execute a file
cargo run -- program.bas -- one two  # Arguments available through COMMAND$()
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

Both modes support structured control flow (including `NEXT j, i` for nested loops), procedures, records, arrays, indexed READ/INPUT/LINE INPUT destinations, text files, string slicing, doubled quotes in string literals, numeric MIN/MAX functions, MAT with console and file I/O, program arguments through `COMMAND$()`, and structured exceptions. QB additionally enables integer division (`\`), EQV/IMP bitwise operators, numeric IF/THEN/ELSE jump targets, classic GOSUB/RETURN, computed ON branches, and classic error-handler syntax, and changes truth values, logical operators, string `+`, default parameter passing, and binary record behavior.

Porting details that matter immediately: arrays default to **base 1 in both modes**; numeric values are stored as `f64` regardless of suffix; PRINT uses 16-character comma zones and no leading/trailing positive-number space; assignment types and array bounds are not strictly enforced. Read the compatibility guide before relying on historical BASIC behavior.

Version 0.15.0 adds array-element BYREF arguments and SWAP targets, empty and signed-plus DATA fields, QB `RETURN label`, binary record sizes through `LEN(record)`, and independent matrix determinants through `DET(matrix)`. The references above document evaluation order, error behavior, and dialect limits for each addition.

Version 0.15.1 adds native Linux ARM64 release builds of both binaries, with the full test suite run on an ARM64 runner before publication.

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
