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
> DIM x AS INTEGER
> x = 42
> PRINT x
 42
```

### Executing Files

Save your program with a `.bas` extension and run it:

```bash
cargo run -- myprogram.bas
```

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
x = 10              ' Auto-created as numeric
name$ = "Alice"     ' $ suffix means STRING
DIM count AS INTEGER ' Explicit declaration
CONST PI = 3.14159  ' Constant (cannot be reassigned)
```

### Type Suffixes

Append a suffix to a variable name to specify its type:

| Suffix | Type    | Example   |
|--------|---------|-----------|
| `%`    | INTEGER | `count%`  |
| `&`    | LONG    | `big&`    |
| `!`    | SINGLE  | `price!`  |
| `#`    | DOUBLE  | `exact#`  |
| `$`    | STRING  | `name$`   |

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

' WHILE loop
WHILE x < 100
    x = x * 2
WEND

' DO loop
DO
    INPUT "Enter a number (0 to quit): ", n
LOOP UNTIL n = 0
```

### Subroutines and Functions

```basic
SUB Greet (name AS STRING)
    PRINT "Hello, "; name; "!"
END SUB

FUNCTION Square (x AS DOUBLE) AS DOUBLE
    Square = x * x
END FUNCTION

CALL Greet("World")
PRINT Square(5)
```

### Arrays

```basic
DIM scores(10) AS INTEGER       ' 11 elements (0-10)
DIM grid(3, 3) AS DOUBLE        ' 2D array
DIM names(1 TO 5) AS STRING     ' Custom bounds

scores(0) = 95
grid(1, 2) = 3.14
names(1) = "Alice"
```

### File I/O

```basic
' Write to a file
OPEN "output.txt" FOR OUTPUT AS #1
PRINT #1, "Hello, file!"
CLOSE #1

' Read from a file
OPEN "output.txt" FOR INPUT AS #1
LINE INPUT #1, text$
PRINT text$
CLOSE #1
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
