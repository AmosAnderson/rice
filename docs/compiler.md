# Native Compiler

RICE BASIC includes an experimental native compiler powered by Cranelift. This compiles BASIC programs directly to machine code, producing standalone executables.

## Status

The compiler is in Phase 1 and supports a subset of the language. For full compatibility, use the interpreter (`cargo run -- program.bas`). The compiler is best suited for programs using core features: arithmetic, control flow, PRINT, and simple variables.

## Usage

### Compile to Executable

```bash
rice --compile program.bas
```

This produces an executable named after the source file (e.g., `program`). To specify a different output name:

```bash
rice --compile program.bas -o myapp
```

Then run the executable directly:

```bash
./myapp
```

### Inspect Intermediate Representation

To see the IR that the compiler generates (useful for debugging or understanding the compilation process):

```bash
rice --emit-ir program.bas
```

This prints the IR to stdout without producing an executable.

## Example

Given `hello.bas`:

```basic
PRINT "Hello from compiled BASIC!"
FOR i = 1 TO 5
    PRINT i
NEXT i
```

Compile and run:

```bash
rice --compile hello.bas
./hello
```

## Limitations

Phase 1 of the compiler does not yet support all interpreter features. Currently unsupported:

- File I/O
- User-defined types (TYPE)
- CHAIN/COMMON
- Error handling (ON ERROR)
- Some built-in functions

For programs using these features, use the interpreter instead.
