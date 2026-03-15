# RICE BASIC Language Documentation

RICE BASIC is a structured BASIC interpreter and compiler written in Rust, implementing a QBasic/FreeBASIC dialect. It supports an interactive REPL, file execution, and native compilation via Cranelift. No graphics or sound support.

## Documentation

- **[Language Reference](language-reference.md)** - Complete reference for all statements, operators, data types, and built-in functions
- **[File I/O Guide](file-io.md)** - Working with files: text, binary, and random access modes
- **[Error Handling](error-handling.md)** - ON ERROR GOTO, RESUME, and error codes
- **[Procedures and Scope](procedures.md)** - SUB, FUNCTION, DEF FN, scope rules, SHARED, STATIC
- **[User-Defined Types](user-defined-types.md)** - TYPE...END TYPE, dot notation, arrays of types
- **[Multi-Module Programming](multi-module.md)** - CHAIN and COMMON for multi-file programs
- **[Console Features](console.md)** - CLS, LOCATE, COLOR, INKEY$, and other text-mode console control
- **[PRINT USING Formatting](print-using.md)** - Format specifiers for formatted output
- **[Native Compiler](compiler.md)** - Compiling BASIC programs to standalone executables
- **[Quick Start Guide](quickstart.md)** - Getting started with RICE BASIC

## Running RICE BASIC

```bash
cargo build                              # Build the interpreter
cargo run                               # Start the interactive REPL
cargo run -- program.bas                  # Execute a .bas file
cargo run -- --compile program.bas        # Compile to native executable
cargo run -- --compile program.bas -o out # Specify output name
cargo run -- --emit-ir program.bas        # Print intermediate representation
```
