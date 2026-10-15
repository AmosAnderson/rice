# Quick start

The [language reference](language-reference.md) defines Rice's syntax and semantics. See [implementation limits](compatibility.md) for unsupported operations and known restrictions.

## Build and run

From this repository, with a Rust toolchain supporting edition 2024:

```sh
cargo build --release --bin rice
cargo run
cargo run -- hello.bas
```

The release binary is `target/release/rice` (`rice.exe` on Windows). Save this as `hello.bas`:

```basic
PRINT "Hello, World!"
```

Relative file paths in a program refer to the process working directory. Pass program arguments with `rice program.bas -- one two`; `COMMAND$()` returns `"one two"`. The `--` separator also allows arguments beginning with `-`. See [runtime](runtime.md) for all flags, argument rules, and editor setup.

## Basic language rules

```basic
PRINT 1 = 1                ' -1
PRINT "Hello" + " world"
```

Numeric `AND`, `OR`, `NOT`, and `XOR` are bitwise. Native BOOL values use logical
operations. Procedure parameters default to BYREF, and array lower bounds default
to 1. See the [language reference](language-reference.md) for exact rules.

## Variables, strings, and output

```basic
DIM count AS NUMERIC
count = 3
name$ = "Alice"
CONST rate = 1.5
PRINT name$; ": "; count * rate
PRINT LEFT$(name$, 3)
PRINT name$(2:4)
PRINT "Hello" & ", " & name$
```

Names are case-insensitive. Declared and suffixed variables enforce their type: INTEGER is i16, LONG i32, SINGLE f32, NUMERIC/DOUBLE f64, and INT64 i64. BOOL, BYTE, BYTES, STRING, and records are also supported. Fixed strings pad or truncate to their character count. Unsuffixed implicit variables remain dynamic unless a DEFtype rule applies. Use CINT64("9223372036854775807") for an exact large integer. See [Data types and declarations](data-types.md).

PRINT semicolons add no spacing; commas advance to 16-character zones; a trailing separator suppresses the newline. Positive numbers have no automatic leading/trailing space. String positions are 1-based Unicode character positions. Double a quote inside a string to include it: `PRINT "She said ""hi"""` prints `She said "hi"`. This works; `CHR$(34)` also produces a quote.

## Decisions and loops

```basic
FOR i = 1 TO 15
    IF i MOD 15 = 0 THEN
        PRINT "FizzBuzz"
    ELSEIF i MOD 3 = 0 THEN
        PRINT "Fizz"
    ELSEIF i MOD 5 = 0 THEN
        PRINT "Buzz"
    ELSE
        PRINT i
    END IF
NEXT i
```

Rice also supports single-line IF, WHILE/WEND, DO/LOOP, SELECT CASE, and GOTO. See [control flow](language-reference.md).

## Arrays

```basic
DIM scores(1 TO 3) AS NUMERIC
scores(1) = 95
scores(2) = 82
scores(3) = 100
FOR i = LBOUND(scores) TO UBOUND(scores)
    PRINT scores(i)
NEXT i
```

Bounds are inclusive. Set OPTION BASE 0/1 or use explicit lower bounds. Ordinary indexing does not enforce bounds or rank. Array declarations enforce their element types; STRING defaults empty and STRING * n defaults to n spaces and pads/truncates assignments. Explicit AS overrides suffix shorthand. See [arrays and limitations](language-reference.md#arrays).

## Procedures

```basic
SUB Greet(BYVAL name$ AS STRING)
    PRINT "Hello, " & name$ & "!"
END SUB

FUNCTION Square(BYVAL x AS NUMERIC) AS NUMERIC
    Square = x * x
END FUNCTION

CALL Greet("World")
PRINT Square(5)
```

A function returns by assigning to its own name. Use BYVAL when a procedure should receive an independent value. Rice implements BYREF as copy-in/copy-out for variables and individual array elements, with indices evaluated once before the call. Whole-array parameters and record-field write-back remain unsupported; read [procedures and scope](procedures.md) before using shared state.

## Text files

This example works and creates `output.txt` in the working directory:

```basic
OPEN "output.txt" FOR OUTPUT AS #1
PRINT #1, "Hello, file!"
CLOSE #1

OPEN "output.txt" FOR INPUT AS #1
LINE INPUT #1, text$
PRINT text$
CLOSE #1
```

The named-channel OPEN syntax is also accepted:

```basic
OPEN #1: NAME "output.txt", ACCESS INPUT, ORGANIZATION SEQUENTIAL
LINE INPUT #1, text$
PRINT text$
CLOSE #1
```

See [file I/O](file-io.md) before using RANDOM/BINARY records: GET/PUT use typed binary layouts and raw byte buffers.

## Errors

Structured exceptions work:

```basic
WHEN EXCEPTION IN
    PRINT 1 / 0
USE
    PRINT "Caught: "; EXTYPE; " "; EXTEXT$
END WHEN
```

Rice also supports classic top-level handlers:

```basic
10 ON ERROR GOTO 100
20 PRINT 1 / 0
30 END
100 PRINT ERR; " at line "; ERL
110 RESUME NEXT
```

See [error handling](error-handling.md) for RETRY, CONTINUE, RESUME, codes, and nested-control-flow restrictions.

## Interactive use

Run `cargo run` and enter statements at the `Ok` prompt. Variables and definitions persist between immediate inputs. Enter a complete block to run it immediately, or use numbered lines to store a program:

```text
10 PRINT "Hello"
20 PRINT "Again"
LIST
RUN
DELETE 20
SAVE "hello.bas"
NEW
LOAD "hello.bas"
RUN
SYSTEM
```

`RUN` uses a fresh interpreter; `NEW` only clears stored program lines. `SAVE` writes numbered source, and `LOAD` restores that format without executing it. Check a source file's syntax without running it using `cargo run -- --check file.bas`. `SYSTEM` or Ctrl+D exits; Ctrl+C while editing also exits. The [runtime guide](runtime.md) explains file commands, range commands, history, and further exit/state quirks.

Use `REM` or an apostrophe for comments. Colons separate statements, and optional numeric or named labels support branches. Additional examples and complete syntax are in the [documentation index](README.md).
