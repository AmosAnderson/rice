# Quick Start Guide

## Installation

RICE BASIC requires the Rust toolchain. Build from source:

```bash
git clone <repository-url>
cd rice
cargo build --release
```

The binary is located at `target/release/rice`.

## Running Programs

### Interactive REPL

Start the REPL with no arguments:

```bash
cargo run
```

The REPL maintains state between lines, so you can define variables and use them later:

```
Ok
DIM x AS NUMERIC
Ok
x = 42
Ok
PRINT x
42
Ok
```

The REPL also supports old-school line-number editing. Type numbered lines to build a program, then use `RUN` to execute, `LIST` to display, `NEW` to clear, and `DELETE` to remove lines:

```
Ok
10 PRINT "Hello, World!"
20 PRINT "Goodbye!"
RUN
Hello, World!
Goodbye!
Ok
```

### Executing Files

Save your program with a `.bas` extension and run it:

```bash
cargo run -- myprogram.bas
```

### Selecting a Dialect

QBasic 1.1 compatibility mode is the default. To run an ANSI-style program, use a flag or put `OPTION DIALECT "ANSI"` in the source:

```bash
cargo run -- --dialect ansi ansi-program.bas
```

Use `--dialect qb`, `--dialect qbasic`, or `--compat` only when you want to request the default QBasic-compatible mode explicitly. See [Dialects](dialects.md) for the exact compatibility rules.

## Your First Program

Create a file called `hello.bas`:

```basic
PRINT "Hello, World!"
```

Run it:

```bash
cargo run -- hello.bas
```

## A More Complete Example

```basic
' FizzBuzz in RICE BASIC
FOR i = 1 TO 30
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

## Basic Concepts

### Variables

Variables are auto-initialized (0 for numbers, "" for strings). You can declare them explicitly or just use them:

```basic
x = 10                   ' Auto-created as numeric
name$ = "Alice"          ' String value
DIM count AS NUMERIC     ' Explicit declaration
CONST PI = 3.14159       ' Constant (cannot be reassigned)
```

RICE BASIC stores two runtime value types: NUMERIC and STRING. QBasic mode accepts suffixes such as `%`, `!`, `#`, `&`, and `$` in variable names for compatibility; ANSI mode rejects numeric suffixes other than `$`.

### Control Flow

```basic
' IF/ELSEIF/ELSE
IF score >= 90 THEN
    PRINT "A"
ELSEIF score >= 80 THEN
    PRINT "B"
ELSE
    PRINT "C"
END IF

' FOR loop
FOR i = 1 TO 10 STEP 2
    PRINT i
NEXT i

' DO loop
DO
    INPUT "Enter a number (0 to quit): ", n
LOOP UNTIL n = 0
```

### String Operations

ANSI BASIC uses `&` for string concatenation and colon slicing for substrings:

```basic
greeting = "Hello" & ", " & "World!"
PRINT greeting

' String slicing (1-based)
word = "Hello"
PRINT word(1:3)    ' "Hel" - characters 1 through 3
PRINT word(3:5)    ' "llo" - characters 3 through 5
```

See [String Slicing](string-slicing.md) for full details.

### Subroutines and Functions

```basic
SUB Greet (name AS STRING)
    PRINT "Hello, " & name & "!"
END SUB

FUNCTION Square (x AS NUMERIC) AS NUMERIC
    Square = x * x
END FUNCTION

CALL Greet("World")
PRINT Square(5)
```

### Arrays

Arrays default to base 1:

```basic
DIM scores(10) AS NUMERIC       ' 10 elements (1-10)
DIM grid(3, 3) AS NUMERIC       ' 2D array
DIM names(1 TO 5) AS STRING     ' Custom bounds

scores(1) = 95
grid(1, 2) = 3.14
names(1) = "Alice"
```

### File I/O

```basic
' Write to a file
OPEN #1: NAME "output.txt", ACCESS OUTPUT
PRINT #1, "Hello, file!"
CLOSE #1

' Read from a file
OPEN #1: NAME "output.txt", ACCESS INPUT
LINE INPUT #1, text
PRINT text
CLOSE #1
```

### Error Handling

ANSI BASIC uses structured exception handling:

```basic
WHEN EXCEPTION IN
    OPEN #1: NAME "data.txt", ACCESS INPUT
    INPUT #1, value
    CLOSE #1
USE
    PRINT "Error: "; EXTEXT$
END WHEN
```

## Case Insensitivity

RICE BASIC is case-insensitive. All of the following are equivalent:

```basic
PRINT "hello"
print "hello"
Print "hello"
```

## Comments

```basic
REM This is a comment
' This is also a comment
x = 10 ' Inline comment
```

## Line Structure

Multiple statements can appear on one line separated by colons:

```basic
x = 1 : y = 2 : PRINT x + y
```

Optional line numbers are supported:

```basic
10 PRINT "Line 10"
20 GOTO 10
```
