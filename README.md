# RICE BASIC

An ANSI X3.113-1991 (Full BASIC) interpreter written in Rust. Supports an interactive REPL and file execution. No graphics or sound -- pure text-mode BASIC.

## Getting Started

### Build

```bash
cargo build
```

### Run the REPL

```bash
cargo run
```

```
RICE BASIC v0.10.0
Type SYSTEM or press Ctrl+D to exit.
Commands: RUN, LIST, NEW, DELETE

Ok
PRINT "Hello, World!"
Hello, World!
Ok
```

The REPL features 24-bit ANSI syntax highlighting, automatic multi-line block detection (FOR/NEXT, IF/END IF, SUB/END SUB, etc.), and old-school line-number program editing:

```
Ok
10 FOR i = 1 TO 5
20 PRINT i
30 NEXT i
RUN
1
2
3
4
5
Ok
LIST
10 FOR i = 1 TO 5
20 PRINT i
30 NEXT i
Ok
```

Unnumbered lines execute immediately. Numbered lines are stored and can be managed with `RUN`, `LIST`, `NEW`, and `DELETE`.

### Execute a File

```bash
cargo run -- myprogram.bas
```

### Run Tests

```bash
cargo test                     # All tests (unit + integration)
cargo test --lib               # Unit tests only
cargo test --test integration  # Integration tests only
cargo test test_hello          # A single test by name
```

## Language Features

RICE BASIC implements the ANSI X3.113-1991 Full BASIC standard:

### Data Types

| Type    | Description                    |
|---------|--------------------------------|
| NUMERIC | Double-precision float (f64)   |
| STRING  | Text strings ($ suffix)        |

There are no type suffixes for numeric subtypes. All numeric values are double-precision. Variables ending in `$` are strings; all others are numeric.

### Operators

- **Arithmetic**: `+`, `-`, `*`, `/`, `^`, `MOD` (works on reals)
- **String concatenation**: `&`
- **Comparison**: `=`, `<>`, `<`, `>`, `<=`, `>=`
- **Logical**: `AND`, `OR`, `NOT`, `XOR` (logical, not bitwise)
- **Truth values**: 1 = true, 0 = false

### Statements

- **Output**: PRINT, PRINT USING, WRITE
- **Input**: INPUT, LINE INPUT
- **Variables**: LET, DIM, CONST, SWAP, OPTION BASE (default 1), REDIM, ERASE, SHARED, STATIC, CLEAR, TYPE...END TYPE (user-defined types)
- **Control flow**: IF/ELSEIF/ELSE/END IF, FOR/NEXT, WHILE/END WHILE, DO/LOOP, SELECT CASE, GOTO, EXIT FOR/DO/SUB/FUNCTION, RANDOMIZE, END, STOP, SYSTEM, SLEEP
- **Procedures**: SUB/END SUB, FUNCTION/END FUNCTION, CALL, DECLARE (BYVAL by default)
- **Data**: DATA, READ, RESTORE
- **Error handling**: WHEN EXCEPTION IN...USE...END WHEN, RETRY, CONTINUE, EXTYPE, EXTEXT$
- **File I/O**: OPEN (ANSI syntax), CLOSE, PRINT#, INPUT#, LINE INPUT#, SET POINTER, ASK POINTER
- **File system**: NAME...AS, KILL, MKDIR, RMDIR, CHDIR
- **Console**: CLS, LOCATE, COLOR, BEEP, WIDTH, VIEW PRINT
- **MAT operations**: MAT PRINT, MAT READ, MAT INPUT, MAT arithmetic (+, -, *), scalar multiply, INV, TRN, DET, ZER, CON, IDN
- **System**: SHELL

### Strings

ANSI Full BASIC uses colon slicing instead of LEFT$/RIGHT$/MID$:

```basic
LET A$ = "Hello, World!"
PRINT A$(1:5)          ! "Hello"
PRINT A$(8:12)         ! "World"
```

String concatenation uses `&`:

```basic
LET A$ = "Hello" & ", " & "World!"
PRINT A$               ! Hello, World!
```

### Built-in Functions

- **Math**: ABS, INT, FIX, SGN, SQR, SIN, COS, TAN, ATN, EXP, LOG, ROUND, ASIN, ACOS, COT, CSC, SEC, ANGLE, CEIL, TRUNCATE, REMAINDER, MAXNUM, PI, RND
- **String**: LEN, INSTR, LTRIM$, RTRIM$, SPACE$, STRING$, CHR$, ASC, STR$, VAL
- **File**: FREEFILE, EOF, LOF, LOC
- **Console**: CSRLIN, POS, INKEY$, INPUT$, SCREEN()
- **System**: ENVIRON$, TIMER, DATE$, TIME$

### File I/O

RICE BASIC uses ANSI Full BASIC file I/O syntax:

```basic
! Write to a file
OPEN #1: NAME "data.txt", ACCESS OUTPUT, ORGANIZATION SEQUENTIAL
PRINT #1: "Hello, File!"
CLOSE #1

! Read from a file
OPEN #1: NAME "data.txt", ACCESS INPUT, ORGANIZATION SEQUENTIAL
DO WHILE NOT EOF(1)
    LINE INPUT #1: x$
    PRINT x$
LOOP
CLOSE #1
```

Access modes: INPUT, OUTPUT, OUTIN. Organization: SEQUENTIAL, STREAM.

### Error Handling

ANSI Full BASIC structured exception handling:

```basic
WHEN EXCEPTION IN
    OPEN #1: NAME "missing.txt", ACCESS INPUT
    LINE INPUT #1: x$
    CLOSE #1
USE
    PRINT "Error"; EXTYPE; EXTEXT$
END WHEN
```

Use RETRY to re-execute the guarded block, or CONTINUE to resume after the failed statement.

### MAT Operations

Full matrix support for numeric arrays:

```basic
DIM A(3, 3), B(3, 3), C(3, 3)
MAT A = ZER
MAT B = IDN
MAT C = A + B
MAT PRINT C
```

## Editor Integration (LSP)

RICE BASIC ships with a language server (`rice-lsp`) that provides diagnostics, completions, hover documentation, and go-to-definition.

Build it with:

```bash
cargo build --release --bin rice-lsp
```

The binary will be at `target/release/rice-lsp` (or `rice-lsp.exe` on Windows). It communicates over stdio.

### Helix

Add to `~/.config/helix/languages.toml`:

```toml
[[language]]
name = "basic"
scope = "source.basic"
file-types = ["bas"]
language-servers = ["rice-lsp"]
comment-token = "'"
indent = { tab-width = 4, unit = "    " }

[language-server.rice-lsp]
command = "rice-lsp"
```

If `rice-lsp` is not on your `PATH`, use the full path to the binary:

```toml
[language-server.rice-lsp]
command = "/path/to/rice-lsp"
```

### Zed

Add to your Zed settings (`settings.json`):

```json
{
  "lsp": {
    "rice-lsp": {
      "binary": {
        "path": "rice-lsp"
      }
    }
  },
  "languages": {
    "BASIC": {
      "language_servers": ["rice-lsp"]
    }
  },
  "file_types": {
    "BASIC": ["bas"]
  }
}
```

Replace `"rice-lsp"` in `binary.path` with the full path to the binary if it is not on your `PATH`.

## Architecture

Entirely hand-written (no parser generators):

```
Source -> Lexer -> Tokens -> Parser -> AST -> Tree-Walking Interpreter -> Output
```

### Module Map

| Module             | Purpose                                              |
|--------------------|------------------------------------------------------|
| `token.rs`         | Token enum, spans                                    |
| `lexer.rs`         | Hand-written tokenizer, case-insensitive             |
| `ast.rs`           | Statement and expression AST nodes                   |
| `parser.rs`        | Recursive descent parser with precedence climbing    |
| `interpreter.rs`   | Tree-walking evaluator, file handle management, exception handling |
| `format_using.rs`  | PRINT USING format engine (numeric + string specifiers) |
| `environment.rs`   | Scope chain, variable storage, label map             |
| `value.rs`         | Value types (Numeric, Str, Record), formatting       |
| `mat.rs`           | MAT operations (arithmetic, inverse, transpose, etc.) |
| `builtins.rs`      | Built-in function registry                           |
| `repl.rs`          | Interactive REPL with syntax highlighting and line-number editing |
| `error.rs`         | Lexer, parser, and runtime error types               |
| `bin/rice_lsp.rs`  | Language server binary (stdio-based)                 |
| `main.rs`          | CLI entry point                                      |

## What's Not Implemented

- Graphics (SCREEN mode switching, PSET, LINE, CIRCLE, etc.)
- Sound (SOUND, PLAY)
- DEF SEG/PEEK/POKE (memory access)

## Dependencies

- [thiserror](https://crates.io/crates/thiserror) -- error type derivation
- [rustyline](https://crates.io/crates/rustyline) -- REPL line editing and history
- [crossterm](https://crates.io/crates/crossterm) -- cross-platform terminal manipulation
- [tower-lsp](https://crates.io/crates/tower-lsp) -- LSP server framework
- [tokio](https://crates.io/crates/tokio) -- async runtime (for LSP)
- [serde_json](https://crates.io/crates/serde_json) -- JSON serialization (for LSP)
## License

This project is licensed under the [MIT License](LICENSE).
