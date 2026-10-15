# RICE BASIC

Rice BASIC 0.20.0 is a text-mode BASIC interpreter written in Rust, with an interactive REPL, file execution, and a stdio language server. Rice defines its own language and evolves its semantics through its specification and tests.

The Rice specification is authoritative. Rice provides exact integers, booleans, byte buffers, Unicode strings, records, and floating-point numbers within one language.

## Start here

Download prebuilt `rice` and `rice-lsp` binaries for Linux (x64 and ARM64), Windows, and macOS from [GitHub Releases](https://github.com/AmosAnderson/rice/releases). The Linux ARM64 archive supports compatible Raspberry Pi systems running a 64-bit OS. Each archive includes both executables, the documentation, and the MIT license. See [release downloads and publishing](docs/runtime.md#release-downloads-and-publishing) for platforms, checksums, and the tag-based release process.

To build from source:

```sh
cargo build
cargo run                         # REPL
cargo run -- program.bas          # Execute a file
cargo run -- program.bas -- one two  # Arguments available through COMMAND$()
cargo run -- --check program.bas   # Check syntax without executing
cargo test
```

A Rust toolchain supporting edition 2024 is required. For optimized binaries, use `cargo build --release`. No graphics or sound APIs are implemented beyond the terminal bell.

```basic
FOR i = 1 TO 3
    PRINT "Hello, BASIC!"
NEXT i
```

## Documentation

The [documentation index](docs/README.md) links the complete specification and guides:

- [Quick start](docs/quickstart.md): runnable examples and everyday use.
- [Language reference](docs/language-reference.md) and [syntax index](SYNTAX.md): lexical rules, types, operators, statements, arrays, control flow, and DATA.
- [Data types and declarations](docs/data-types.md): exact INT64 integers, BOOL/BOOLEAN, BYTE, and BYTES buffers.
- [Implementation limits](docs/compatibility.md): partial features and unverified behavior.
- [Built-in functions](docs/builtins.md): complete signatures, argument rules, conversions, strings, random numbers, and interpreter state.
- [Procedures and scope](docs/procedures.md), [user-defined types](docs/user-defined-types.md), and [module boundaries](docs/multi-module.md).
- [File I/O](docs/file-io.md), [errors](docs/error-handling.md), [console](docs/console.md), and [PRINT USING](docs/print-using.md).
- [MAT operations](docs/mat-operations.md) and [string slicing](docs/string-slicing.md).
- [CLI, REPL, host operations, and editor integration](docs/runtime.md).

Rice supports structured control flow (including `NEXT j, i` for nested loops), procedures, records, arrays, indexed READ/INPUT/LINE INPUT destinations, text files, string slicing, numeric MIN/MAX, MAT with console and file I/O, program arguments through `COMMAND$()`, and structured exceptions. Integer division (`\`), EQV/IMP bitwise operators, numeric IF/THEN/ELSE jump targets, GOSUB/RETURN, computed ON branches, classic error handlers, and structured binary records are also supported.

Key language rules: arrays default to **base 1**; all declarations enforce their runtime types. INTEGER, LONG, SINGLE, INT64, BOOL, BYTE, BYTES, STRING, records, and NUMERIC/DOUBLE have distinct conversion rules; numeric literals use `f64`. Fixed-length strings pad or truncate to their declared character count. PRINT uses 16-character comma zones and no leading/trailing positive-number space; array bounds remain unenforced. See [Data types and declarations](docs/data-types.md).

Version 0.15.0 adds array-element BYREF arguments and SWAP targets, empty and signed-plus DATA fields, `RETURN label`, binary record sizes through `LEN(record)`, and independent matrix determinants through `DET(matrix)`. The references above document evaluation order, error behavior, and implementation limits for each addition.

Version 0.15.1 adds native Linux ARM64 release builds of both binaries, with the full test suite run on an ARM64 runner before publication.

Version 0.16.0 adds six language features:

- `EXIT WHILE` for leaving the nearest WHILE loop.
- `EXIT SELECT` for leaving the nearest SELECT CASE block.
- Scalar or array-element `GET`/`PUT` operands, including record arrays.
- FUNCTION result defaults from `AS type`, including fresh nested records.
- Scientific decimal prefixes with E/D exponents in `VAL`.
- Indexed `ENVIRON$(index)` lookup of sorted environment entries, including interpreter overrides.

The references above document each feature, its evaluation order, and its implementation limits.

Version 0.17.0 adds six core language features:

- Comma-separated `CONST` declarations, evaluated left to right.
- Explicit `DIM`/`REDIM` array `AS` defaults for untouched elements, including unsuffixed string arrays.
- `MID$` assignment to individual array elements, with each index evaluated once.
- `REPLACE$` for substring replacement with optional start and count.
- `INSTRREV` for reverse substring search.
- Negative decimal places in `ROUND` and `TRUNCATE`, for tens, hundreds, and larger powers of ten.

All six are available. The [core reference](docs/language-reference.md), [string guide](docs/string-slicing.md), and [builtin reference](docs/builtins.md) document each addition. Array declarations enforce conversion and select binary layouts. Array storage and scope limits remain documented. REPLACE$ returns the selected suffix when a start position is supplied.

Version 0.17.1 adds four capabilities:

- `rice --check program.bas` validates syntax without running the program.
- REPL `LOAD`/`SAVE` restore and persist numbered programs.
- `STRCOMP(a$, b$[, compare])` returns string ordering, with an optional Unicode lowercase comparison mode.
- `text$ LIKE pattern$` matches whole strings using wildcards, character sets, and ranges.

STRCOMP and LIKE work as Rice extensions. See the [runtime](docs/runtime.md), [builtin](docs/builtins.md), and [operator](docs/language-reference.md#wildcard-matching) references for exact behavior and limits.

Version 0.20.0 expands the type system and strengthens runtime behavior:

- Native INTEGER, LONG, SINGLE, INT64, BOOL/BOOLEAN, BYTE, and BYTES values, with explicit conversions and byte-buffer operations.
- Consistent enforced declarations across variables, arrays, records, parameters, function results, constants, shared bindings, and static storage.
- One Rice language definition, with dialect-selection options removed.
- Fixes for scoped array access, rejected declarations, buffered file positioning and records, exception recovery, matrix boundaries, and numeric formatting.

The [data type reference](docs/data-types.md) documents conversion and migration rules. The runtime fixes include regression coverage for file I/O failures and numeric limits.

## REPL and editor support

Unnumbered input executes immediately; numbered lines build a stored program:

```text
10 PRINT "Hello"
20 PRINT "Again"
RUN
LIST
```

`RUN` starts the stored program with a fresh interpreter. `LIST`, `NEW`, and `DELETE` manage stored lines; `SAVE "program.bas"` and `LOAD "program.bas"` persist and restore numbered programs. Complete blocks can also be entered directly. The [runtime guide](docs/runtime.md) documents command ranges, state persistence, exit behavior, and limitations.

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
| `src/lib.rs`, `src/error.rs` | Public modules, console helpers, error definitions. |
| `src/bin/rice_lsp.rs` | Stdio language server. |

Tests include unit tests and BASIC fixtures in `tests/programs`, asserted by `tests/integration.rs`. Focused commands: `cargo test --lib`, `cargo test --test integration`, and `cargo test --bin rice-lsp`. See [AGENTS.md](AGENTS.md) for contribution instructions and [CLAUDE.md](CLAUDE.md) for a source-navigation guide.

Dependencies are declared in [Cargo.toml](Cargo.toml): `thiserror`, `rustyline`, `crossterm`, `tower-lsp`, and `tokio`, with `tempfile` for tests.

## License

[MIT](LICENSE).
