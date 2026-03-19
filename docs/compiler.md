# Native Compiler

RICE BASIC includes a native compiler powered by Cranelift. This compiles BASIC programs directly to machine code, producing standalone executables.

## Status

The compiler is at near-parity with the interpreter and supports most language features: arithmetic, control flow, PRINT, variables, arrays, SUB/FUNCTION, DEF FN, file I/O, user-defined types (TYPE), error handling (ON ERROR GOTO/RESUME), REDIM PRESERVE, console features (CLS, LOCATE, COLOR, INKEY$, SCREEN()), and more.

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

The compiler supports nearly all interpreter features. Currently unsupported in compiled mode:

- **CHAIN** — dynamically loads and executes another .bas file, which is fundamentally incompatible with ahead-of-time compilation. Using CHAIN in compiled mode produces a compile-time error. Use the interpreter for multi-module programs that rely on CHAIN.
- **LBOUND/UBOUND** — stubs only (same as interpreter).
- Proper array storage — uses flattened keys (same as interpreter).
