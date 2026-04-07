# RICE BASIC Language Documentation

RICE BASIC is a structured BASIC interpreter written in Rust, implementing the ANSI X3.113-1991 (Full BASIC) standard. It supports an interactive REPL and file execution. No graphics or sound support.

## Documentation

- **[Language Reference](language-reference.md)** - Complete reference for all statements, operators, data types, and built-in functions
- **[File I/O Guide](file-io.md)** - Working with files: sequential, relative, and stream access
- **[Error Handling](error-handling.md)** - WHEN EXCEPTION structured error handling
- **[Procedures and Scope](procedures.md)** - SUB, FUNCTION, scope rules, SHARED, STATIC
- **[User-Defined Types](user-defined-types.md)** - TYPE...END TYPE, dot notation, arrays of types
- **[MAT Operations](mat-operations.md)** - Matrix operations: MAT PRINT, MAT READ, arithmetic, INV, TRN, DET
- **[String Slicing](string-slicing.md)** - Colon slicing, & concatenation, slice assignment
- **[Console Features](console.md)** - CLS, LOCATE, and other text-mode console control
- **[PRINT USING Formatting](print-using.md)** - Format specifiers for formatted output
- **[Quick Start Guide](quickstart.md)** - Getting started with RICE BASIC

## Running RICE BASIC

```bash
cargo build                              # Build the interpreter
cargo run                               # Start the interactive REPL
cargo run -- program.bas                  # Execute a .bas file
```
