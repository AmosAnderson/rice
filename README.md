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

- **Output**: PRINT, PRINT USING, WRITE
- **Input**: INPUT, LINE INPUT
- **Variables**: LET, DIM, CONST, SWAP, OPTION BASE, REDIM, ERASE, SHARED, STATIC, DEFINT/DEFLNG/DEFSNG/DEFDBL/DEFSTR, CLEAR, TYPE...END TYPE (user-defined types)
- **Control flow**: IF/ELSEIF/ELSE/END IF, FOR/NEXT, WHILE/WEND, DO/LOOP, SELECT CASE, GOTO, GOSUB/RETURN, ON n GOTO/GOSUB, EXIT FOR/DO/SUB/FUNCTION, ON ERROR GOTO/RESUME, END, STOP, SYSTEM, SLEEP
- **Procedures**: SUB/END SUB, FUNCTION/END FUNCTION, DEF FN, CALL, DECLARE
- **Data**: DATA, READ, RESTORE
- **String mutation**: MID$ (assignment), LSET, RSET
- **File I/O**: OPEN, CLOSE, PRINT#, WRITE#, INPUT#, LINE INPUT#, GET, PUT
- **File system**: NAME...AS, KILL, MKDIR, RMDIR, CHDIR
- **System**: SHELL

### Built-in Functions

- **String**: LEN, LEFT$, RIGHT$, MID$, INSTR, UCASE$, LCASE$, LTRIM$, RTRIM$, SPACE$, STRING$, CHR$, ASC, STR$, VAL, HEX$, OCT$
- **Math**: ABS, SGN, INT, FIX, SQR, EXP, LOG, SIN, COS, TAN, ATN, RND
- **Conversion**: CINT, CLNG, CSNG, CDBL, MKI$, MKL$, MKS$, MKD$, CVI, CVL, CVS, CVD
- **File**: FREEFILE, EOF, LOF, LOC
- **Error handling**: ERR, ERL
- **System**: ENVIRON$, TIMER, DATE$, TIME$, RANDOMIZE

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
- Screen control (LOCATE, WIDTH, COLOR, INKEY$)
- CHAIN/COMMON (multi-module programs)
- DEF SEG/PEEK/POKE (memory access)
- FIELD (random-access file fields), SEEK
- BYVAL parameter semantics

## Dependencies

- [thiserror](https://crates.io/crates/thiserror) -- error type derivation
- [rustyline](https://crates.io/crates/rustyline) -- REPL line editing and history
- [tower-lsp](https://crates.io/crates/tower-lsp) -- LSP server framework
- [tokio](https://crates.io/crates/tokio) -- async runtime (for LSP)
- [serde_json](https://crates.io/crates/serde_json) -- JSON serialization (for LSP)

## License

See repository for license information.
