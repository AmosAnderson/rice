# Rice BASIC documentation

This documentation is the authoritative specification of **Rice BASIC 0.20.0 as implemented**. Rice develops its own semantics. Implementation limits, ignored syntax, and unverified areas are documented explicitly.

## Complete specification

| Reference | Contents |
|---|---|
| [Language reference](language-reference.md) | Source structure, literals, names, types, precedence, statements, arrays, control flow, DATA, options. |
| [Data types and declarations](data-types.md) | Declared numeric widths, fixed strings, record identity, conversion, precision, and byte buffers. |
| [Built-in functions](builtins.md) | Every implemented expression function, signature, return value, restrictions, and calling quirks. |
| [Procedures and scope](procedures.md) | SUB, FUNCTION, DEF, CALL, DECLARE, BYREF/BYVAL, SHARED, COMMON, STATIC. |
| [User-defined types](user-defined-types.md) | TYPE declarations, fields, nested records, arrays, initialization and serialization. |
| [SQLite databases](sqlite.md) | Connections, bound SQL, native values and NULL, result rows, transactions, and cleanup. |
| [File I/O](file-io.md) | Both OPEN syntaxes, text input/output, positions, binary/random records, FIELD buffers. |
| [Error handling](error-handling.md) | Structured WHEN/USE and ON ERROR/RESUME, registers, codes, control-flow limits. |
| [Console](console.md) | PRINT, INPUT, terminal state/control, keyboard functions, screen buffer limits. |
| [PRINT USING](print-using.md) | Numeric/string format grammar, format reuse, overflow, separators. |
| [MAT](mat-operations.md) | Matrix syntax, dimensions, arithmetic, inverse, determinant, console/file I/O and limitations. |
| [String slicing](string-slicing.md) | Colon substrings, slice assignment, MID$ assignment, Unicode. |
| [Module boundaries](multi-module.md) | Single-source execution and unsupported module/linker directives. |
| [Runtime](runtime.md) | CLI, REPL, state, host operations, library entry points, LSP/editor setup. |
| [Implementation limits](compatibility.md) | Partial and unsupported features, runtime constraints, remaining verification gaps. |

The [syntax index](../SYNTAX.md) provides a compact entry point to the Rice specification. Topic guides expand on the core reference with detailed syntax, examples, and implementation limits.

## Getting started

Read the [quick start](quickstart.md), or run from the repository:

```sh
cargo build
cargo run
cargo run -- program.bas
```

For implementation work, use [AGENTS.md](../AGENTS.md), [CLAUDE.md](../CLAUDE.md), and the [source map](../README.md#implementation-and-development). The [test fixtures](../tests/programs) and [integration assertions](../tests/integration.rs) are executable examples. Code, tests, and focused behavior probes underpin this reference; partial features and unverified runtime behavior are identified explicitly.
