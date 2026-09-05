# Rice BASIC documentation

This documentation specifies **Rice BASIC 0.14.0 as implemented**, for the default QBasic compatibility mode and the ANSI-style mode. It is not a reproduction of either external language standard or a claim of full conformance. Known deviations, ignored syntax, and unverified areas are part of the specification.

## Complete specification

| Reference | Contents |
|---|---|
| [Language reference](language-reference.md) | Source structure, literals, names, types, precedence, statements, arrays, control flow, DATA, options. |
| [Dialect rules](dialects.md) | Mode selection, behavior comparison, features available in each mode. |
| [Built-in functions](builtins.md) | Every implemented expression function, signature, return value, restrictions, and calling quirks. |
| [Procedures and scope](procedures.md) | SUB, FUNCTION, DEF, CALL, DECLARE, BYREF/BYVAL, SHARED, COMMON, STATIC. |
| [User-defined types](user-defined-types.md) | TYPE declarations, fields, nested records, arrays, initialization and serialization. |
| [File I/O](file-io.md) | Both OPEN syntaxes, text input/output, positions, binary/random records, FIELD buffers. |
| [Error handling](error-handling.md) | Structured WHEN/USE and QB ON ERROR/RESUME, registers, codes, control-flow limits. |
| [Console](console.md) | PRINT, INPUT, terminal state/control, keyboard functions, screen buffer limits. |
| [PRINT USING](print-using.md) | Numeric/string format grammar, format reuse, overflow, separators. |
| [MAT](mat-operations.md) | Matrix syntax, dimensions, arithmetic, inverse, determinant, I/O limitations. |
| [String slicing](string-slicing.md) | Colon substrings, slice assignment, MID$ assignment, Unicode. |
| [Module boundaries](multi-module.md) | Single-source execution and unsupported module/linker directives. |
| [Runtime](runtime.md) | CLI, REPL, state, host operations, library entry points, LSP/editor setup. |
| [Compatibility and unknowns](compatibility.md) | Consolidated porting risks, unsupported features, remaining verification gaps. |

The [syntax index](../SYNTAX.md) provides a compact entry point to the specification. Topic pages cover both modes unless a section explicitly names one dialect.

## Getting started

Read the [quick start](quickstart.md), or run from the repository:

```sh
cargo build
cargo run
cargo run -- program.bas
cargo run -- --dialect ansi program.bas
```

For implementation work, use [AGENTS.md](../AGENTS.md), [CLAUDE.md](../CLAUDE.md), and the [source map](../README.md#implementation-and-development). The [test fixtures](../tests/programs) and [integration assertions](../tests/integration.rs) are executable examples. Code, tests, and focused behavior probes underpin this reference; unknown external compatibility is identified explicitly rather than inferred from keyword names.
