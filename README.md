# RICE BASIC

A structured BASIC interpreter written in Rust, in the style of QBasic/FreeBASIC. Supports both an interactive REPL and file execution. No graphics or sound -- pure text-mode BASIC.

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
RICE BASIC v0.9.0
Type SYSTEM or press Ctrl+D to exit.

Ok
PRINT "Hello, World!"
Hello, World!
Ok
```

The REPL features 24-bit ANSI syntax highlighting, persistent environment across lines, and automatic multi-line block detection (FOR/NEXT, IF/END IF, SUB/END SUB, etc.).

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

RICE BASIC implements a broad subset of QBasic:

### Data Types

| Type    | Suffix | Description            |
|---------|--------|------------------------|
| INTEGER | `%`    | Whole numbers          |
| LONG    | `&`    | Large whole numbers    |
| SINGLE  | `!`    | Single-precision float |
| DOUBLE  | `#`    | Double-precision float |
| STRING  | `$`    | Text strings           |

### Statements

- **Output**: PRINT, PRINT USING
- **Input**: INPUT, LINE INPUT
- **Variables**: LET, DIM, CONST, SWAP, OPTION BASE, REDIM, ERASE
- **Control flow**: IF/ELSEIF/ELSE/END IF, FOR/NEXT, WHILE/WEND, DO/LOOP, SELECT CASE, GOTO, GOSUB/RETURN, EXIT FOR/DO/SUB/FUNCTION, ON ERROR GOTO/RESUME, END, STOP, SYSTEM
- **Procedures**: SUB/END SUB, FUNCTION/END FUNCTION, CALL, DECLARE
- **Data**: DATA, READ, RESTORE
- **File I/O**: OPEN, CLOSE, PRINT#, WRITE#, INPUT#, LINE INPUT#, GET, PUT

### Built-in Functions

- **String**: LEN, LEFT$, RIGHT$, MID$, INSTR, UCASE$, LCASE$, LTRIM$, RTRIM$, SPACE$, STRING$, CHR$, ASC, STR$, VAL, HEX$, OCT$
- **Math**: ABS, SGN, INT, FIX, SQR, EXP, LOG, SIN, COS, TAN, ATN, RND
- **Conversion**: CINT, CLNG, CSNG, CDBL
- **File**: FREEFILE, EOF, LOF, LOC
- **Error handling**: ERR, ERL
- **Other**: TIMER, DATE$, TIME$, RANDOMIZE

### File I/O

RICE BASIC supports text and binary file operations:

```basic
' Write to a file
OPEN "data.txt" FOR OUTPUT AS #1
PRINT #1, "Hello, File!"
WRITE #1, "Alice", 30
CLOSE #1

' Read from a file
OPEN "data.txt" FOR INPUT AS #1
DO WHILE NOT EOF(1)
    LINE INPUT #1, x$
    PRINT x$
LOOP
CLOSE #1

' Binary file access
OPEN "data.bin" FOR BINARY AS #1
PUT #1, 1, value$
GET #1, 1, result$
CLOSE #1
```

File modes: INPUT, OUTPUT, APPEND, RANDOM, BINARY.

## Architecture

Classic interpreter pipeline, entirely hand-written (no parser generators):

```
Source -> Lexer -> Tokens -> Parser -> AST -> Tree-Walking Interpreter -> Output
```

### Module Map

| Module           | Purpose                                              |
|------------------|------------------------------------------------------|
| `token.rs`       | Token enum, type suffixes, spans                     |
| `lexer.rs`       | Hand-written tokenizer, case-insensitive             |
| `ast.rs`         | Statement and expression AST nodes                   |
| `parser.rs`      | Recursive descent parser with precedence climbing    |
| `interpreter.rs` | Tree-walking evaluator, file handle management, error trapping |
| `format_using.rs`| PRINT USING format engine (numeric + string specifiers) |
| `environment.rs` | Scope chain, variable storage, label map             |
| `value.rs`       | Value types, QBasic-style formatting, coercion       |
| `builtins.rs`    | Built-in function registry                           |
| `repl.rs`        | Interactive REPL with syntax highlighting            |
| `error.rs`       | Lexer, parser, and runtime error types               |
| `main.rs`        | CLI entry point                                      |

## What's Not Implemented

- Graphics (SCREEN, PSET, LINE, CIRCLE, etc.)
- Sound (SOUND, BEEP, PLAY)
- Screen control (LOCATE, WIDTH, COLOR)
- User-defined types (TYPE...END TYPE)
- DEFtype statements (DEFINT, DEFSNG, etc.)
- ON n GOTO/GOSUB (computed jumps)
- SHARED/STATIC/BYVAL parameter semantics
- LSET/RSET

## Dependencies

- [thiserror](https://crates.io/crates/thiserror) -- error type derivation
- [rustyline](https://crates.io/crates/rustyline) -- REPL line editing and history

## License

See repository for license information.
