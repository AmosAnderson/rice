# Quick start

Rice defaults to QBasic compatibility mode. Its ANSI-style mode and shared extensions are documented in the [language reference](language-reference.md). Neither mode implements every feature of its namesake; consult [compatibility notes](compatibility.md) when porting code.

## Build and run

From this repository, with a Rust toolchain supporting edition 2024:

```sh
cargo build --release --bin rice
cargo run
cargo run -- hello.bas
cargo run -- --dialect ansi hello.bas
```

The release binary is `target/release/rice` (`rice.exe` on Windows). Save this as `hello.bas`:

```basic
PRINT "Hello, World!"
```

Relative file paths in a program refer to the process working directory. The CLI takes one source file and no extra program arguments. See [runtime](runtime.md) for all flags and editor setup.

## Choose a dialect

Put a directive at the start of a program for predictable parsing and editor diagnostics:

```basic
OPTION DIALECT "QB"
PRINT 1 = 1                ' -1
PRINT "Hello" + " world"
```

```basic
OPTION DIALECT "ANSI"
PRINT 1 = 1                ' 1
PRINT "Hello" & " world"
```

QB uses bitwise numeric `AND`, `OR`, `NOT`, and `XOR` and defaults procedure parameters to BYREF. ANSI uses logical operators and defaults parameters to BYVAL. **Both currently default array lower bounds to 1.** A source directive applies to the whole source unit, even if placed later. See [dialects](dialects.md) for exact rules.

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

Names are case-insensitive. Uninitialized numeric variables read as `0`, `$` names as `""`. Numeric storage is `f64`; QB numeric suffixes distinguish names but do not enforce integer or single-precision storage. `AS` declarations and suffixes mostly establish defaults, not strict assignment types, so use consistent types in your own code.

PRINT semicolons add no spacing; commas advance to 16-character zones; a trailing separator suppresses the newline. Positive numbers have no automatic leading/trailing space. String positions are 1-based Unicode character positions. Quotes are not escaped by doubling them; use `CHR$(34)` to insert a quote.

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

Both modes also support single-line IF, WHILE/WEND, DO/LOOP, SELECT CASE, and GOTO. Use explicit comparison conditions when sharing code between modes; `NOT` has different numeric behavior in QB. See [control flow](language-reference.md).

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

Bounds are inclusive. Write explicit lower bounds for portable intent, or set `OPTION BASE 0`/`OPTION BASE 1`. Ordinary array access does not enforce declared bounds or rank. Use `$`-suffixed string-array names so missing elements receive string defaults. See [arrays and limitations](language-reference.md).

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

A function returns by assigning to its own name. Explicit BYVAL avoids the dialect-dependent default. Rice implements BYREF as copy-in/copy-out for bare variable arguments, with limitations for arrays and fields; read [procedures and scope](procedures.md) before using shared state.

## Text files

This example works in either mode and creates `output.txt` in the working directory:

```basic
OPEN "output.txt" FOR OUTPUT AS #1
PRINT #1, "Hello, file!"
CLOSE #1

OPEN "output.txt" FOR INPUT AS #1
LINE INPUT #1, text$
PRINT text$
CLOSE #1
```

ANSI-style OPEN syntax is also accepted in both modes:

```basic
OPEN #1: NAME "output.txt", ACCESS INPUT, ORGANIZATION SEQUENTIAL
LINE INPUT #1, text$
PRINT text$
CLOSE #1
```

See [file I/O](file-io.md) before using RANDOM/BINARY records: GET/PUT semantics differ substantially by dialect.

## Errors

Structured exceptions work in both modes:

```basic
WHEN EXCEPTION IN
    PRINT 1 / 0
USE
    PRINT "Caught: "; EXTYPE; " "; EXTEXT$
END WHEN
```

QB also supports classic top-level handlers:

```basic
OPTION DIALECT "QB"
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
NEW
SYSTEM
```

`RUN` uses a fresh interpreter; `NEW` only clears stored program lines. `SYSTEM` or Ctrl+D exits; Ctrl+C while editing also exits. The [runtime guide](runtime.md) explains range commands, history, and further exit/state quirks.

Use `REM` or an apostrophe for comments. Colons separate statements, and optional numeric or named labels support branches. Additional examples and complete syntax are in the [documentation index](README.md).
