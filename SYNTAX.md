# Rice BASIC Language Reference

Rice is a structured BASIC interpreter in the style of QBasic. This document covers the full syntax supported by Rice.

## Running Programs

```bash
rice                  # Start interactive REPL
rice myprogram.bas    # Execute a file
```

In the REPL, type statements and press Enter to execute them. Type `END` or press Ctrl+D to exit.

---

## Comments

```basic
REM This is a comment
' This is also a comment
x = 5  ' Inline comment after a statement
```

---

## Data Types

| Type      | Suffix | Description               |
|-----------|--------|---------------------------|
| INTEGER   | `%`    | Whole numbers             |
| LONG      | `&`    | Large whole numbers       |
| SINGLE    | `!`    | Single-precision float    |
| DOUBLE    | `#`    | Double-precision float    |
| STRING    | `$`    | Text                      |

Variables with different suffixes are distinct — `x%`, `x$`, and `x` are three separate variables.

---

## Variables and Constants

```basic
' Implicit declaration (no DIM needed)
x = 10
name$ = "Alice"
rate# = 3.14159

' Explicit declaration
DIM count AS INTEGER
DIM message AS STRING

' Constants (cannot be changed after definition)
CONST PI = 3.14159
CONST MAX_SIZE = 100
CONST GREETING = "Hello"

' LET is optional
LET y = 20
y = 20          ' Same thing
```

Undeclared variables auto-initialize to `0` (numeric) or `""` (string).

---

## Operators

### Arithmetic

| Operator | Description            | Example          |
|----------|------------------------|------------------|
| `+`      | Addition               | `5 + 3` → `8`   |
| `-`      | Subtraction            | `5 - 3` → `2`   |
| `*`      | Multiplication         | `5 * 3` → `15`  |
| `/`      | Division (float)       | `7 / 2` → `3.5` |
| `\`      | Integer division       | `7 \ 2` → `3`   |
| `MOD`    | Modulo (remainder)     | `7 MOD 3` → `1` |
| `^`      | Exponentiation         | `2 ^ 10` → `1024` |

### Comparison

All return `-1` (true) or `0` (false).

| Operator | Description       |
|----------|-------------------|
| `=`      | Equal             |
| `<>`     | Not equal         |
| `<`      | Less than         |
| `>`      | Greater than      |
| `<=`     | Less than/equal   |
| `>=`     | Greater than/equal|

### Logical / Bitwise

| Operator | Description                  |
|----------|------------------------------|
| `AND`    | Logical/bitwise AND          |
| `OR`     | Logical/bitwise OR           |
| `NOT`    | Logical/bitwise NOT (unary)  |
| `XOR`    | Exclusive OR                 |
| `EQV`    | Equivalence (NOT XOR)        |
| `IMP`    | Implication                  |

### String Concatenation

```basic
greeting$ = "Hello, " + "World!"
```

### Operator Precedence (highest to lowest)

1. `^` (right-associative)
2. Unary `-`, `+`
3. `*`, `/`
4. `\`
5. `MOD`
6. `+`, `-`
7. `=`, `<>`, `<`, `>`, `<=`, `>=`
8. `NOT`
9. `AND`
10. `OR`
11. `XOR`
12. `EQV`
13. `IMP`

---

## Output

### PRINT

```basic
PRINT "Hello, World!"
PRINT 42
PRINT "Value:"; x           ' Semicolon: no gap between items
PRINT "A", "B", "C"         ' Comma: tab to next 14-column zone
PRINT x;                    ' Trailing semicolon: suppress newline
PRINT                       ' Print a blank line
PRINT TAB(20); "Indented"   ' Move to column 20
PRINT SPC(5); "Spaced"      ' Insert 5 spaces
```

**Number formatting**: positive numbers are printed with a leading and trailing space. Negative numbers have a `-` instead of the leading space.

```basic
PRINT 42          ' Outputs:  42
PRINT -7          ' Outputs: -7
PRINT "abc"       ' Outputs: abc
```

---

## Input

### INPUT

```basic
INPUT x                        ' Prompts: ?
INPUT "Your name"; name$       ' Prompts: Your name?
INPUT "Enter two numbers"; a, b  ' Read multiple values (comma-separated)
```

### LINE INPUT

Reads an entire line, including commas and spaces.

```basic
LINE INPUT "Enter a sentence: "; s$
```

---

## Control Flow

### IF / THEN / ELSE

**Single-line form:**

```basic
IF x > 0 THEN PRINT "positive" ELSE PRINT "non-positive"
```

**Block form:**

```basic
IF x > 10 THEN
    PRINT "big"
ELSEIF x > 5 THEN
    PRINT "medium"
ELSE
    PRINT "small"
END IF
```

### FOR / NEXT

```basic
FOR i = 1 TO 10
    PRINT i
NEXT i

' With STEP
FOR i = 10 TO 1 STEP -1
    PRINT i
NEXT i

' EXIT FOR leaves the loop early
FOR i = 1 TO 100
    IF i = 5 THEN EXIT FOR
    PRINT i
NEXT i
```

### WHILE / WEND

```basic
x = 1
WHILE x <= 10
    PRINT x
    x = x + 1
WEND
```

### DO / LOOP

Four forms:

```basic
' Test at top with WHILE
DO WHILE x < 10
    x = x + 1
LOOP

' Test at top with UNTIL
DO UNTIL x >= 10
    x = x + 1
LOOP

' Test at bottom with WHILE (always runs at least once)
DO
    x = x + 1
LOOP WHILE x < 10

' Test at bottom with UNTIL
DO
    x = x + 1
LOOP UNTIL x >= 10

' Infinite loop (use EXIT DO to break out)
DO
    x = x + 1
    IF x = 10 THEN EXIT DO
LOOP
```

### SELECT CASE

```basic
SELECT CASE score
    CASE 100
        PRINT "Perfect!"
    CASE 90 TO 99
        PRINT "Excellent"
    CASE 80, 85
        PRINT "Good"
    CASE IS >= 70
        PRINT "Passing"
    CASE ELSE
        PRINT "Needs improvement"
END SELECT
```

Case tests can be:
- A single value: `CASE 5`
- Multiple values: `CASE 1, 2, 3`
- A range: `CASE 10 TO 20`
- A comparison: `CASE IS > 100`

### GOTO and GOSUB

```basic
' Jump to a label
GOTO skip
PRINT "This is skipped"
skip:
PRINT "Jumped here"

' Subroutine call (use GOSUB/RETURN, not to be confused with SUB)
GOSUB greet
PRINT "Back from gosub"
END

greet:
    PRINT "Hello!"
    RETURN
```

Labels can be names (followed by `:`) or line numbers:

```basic
100 PRINT "Line 100"
200 GOTO 100
```

---

## Procedures

### SUB (no return value)

```basic
DECLARE SUB Greet(name AS STRING)

Greet "World"
CALL Greet("World")    ' Alternative call syntax

SUB Greet(name AS STRING)
    PRINT "Hello, " + name + "!"
END SUB
```

### FUNCTION (returns a value)

Assign to the function name to set the return value.

```basic
DECLARE FUNCTION Square(n AS DOUBLE)

PRINT Square(5)        ' Prints 25

FUNCTION Square(n AS DOUBLE)
    Square = n * n
END FUNCTION
```

Functions can be recursive:

```basic
FUNCTION Factorial(n AS INTEGER)
    IF n <= 1 THEN
        Factorial = 1
    ELSE
        Factorial = n * Factorial(n - 1)
    END IF
END FUNCTION

PRINT Factorial(10)    ' Prints 3628800
```

Use `EXIT SUB` or `EXIT FUNCTION` to return early.

---

## Arrays

```basic
DIM scores(10) AS INTEGER         ' Indices 0 to 10
DIM grid(1 TO 5, 1 TO 5) AS DOUBLE   ' 2D array with explicit bounds
DIM names(20) AS STRING

scores(0) = 95
scores(1) = 87
grid(1, 1) = 3.14

' Change default lower bound
OPTION BASE 1

' Resize a dynamic array
REDIM arr(50) AS INTEGER
REDIM PRESERVE arr(100) AS INTEGER   ' Keep existing data

' Clear an array
ERASE scores
```

---

## DATA / READ / RESTORE

Embed data directly in your program:

```basic
DATA 10, 20, 30
DATA "Alice", "Bob", "Carol"

READ a, b, c             ' a=10, b=20, c=30
READ n1$, n2$, n3$       ' n1$="Alice", etc.

RESTORE                  ' Reset to the beginning of DATA
READ x                   ' x=10 again
```

---

## Built-in Functions

### String Functions

| Function                 | Description                            | Example                       |
|--------------------------|----------------------------------------|-------------------------------|
| `LEN(s$)`               | Length of string                        | `LEN("abc")` → `3`           |
| `LEFT$(s$, n)`           | First n characters                     | `LEFT$("Hello", 3)` → `"Hel"` |
| `RIGHT$(s$, n)`          | Last n characters                      | `RIGHT$("Hello", 3)` → `"llo"` |
| `MID$(s$, start)`        | Substring from position (1-based)      | `MID$("Hello", 2)` → `"ello"` |
| `MID$(s$, start, len)`   | Substring with length                  | `MID$("Hello", 2, 3)` → `"ell"` |
| `INSTR(s$, find$)`       | Find substring (0 if not found)        | `INSTR("Hello", "ll")` → `3` |
| `INSTR(start, s$, find$)` | Find from position                    | `INSTR(4, "abcabc", "abc")` → `4` |
| `UCASE$(s$)`             | Convert to uppercase                   | `UCASE$("hello")` → `"HELLO"` |
| `LCASE$(s$)`             | Convert to lowercase                   | `LCASE$("HELLO")` → `"hello"` |
| `LTRIM$(s$)`             | Remove leading spaces                  | `LTRIM$("  hi")` → `"hi"`    |
| `RTRIM$(s$)`             | Remove trailing spaces                 | `RTRIM$("hi  ")` → `"hi"`    |
| `SPACE$(n)`              | String of n spaces                     | `SPACE$(3)` → `"   "`        |
| `STRING$(n, s$)`         | Repeat character n times               | `STRING$(5, "*")` → `"*****"` |
| `CHR$(n)`                | Character from ASCII code              | `CHR$(65)` → `"A"`           |
| `ASC(s$)`                | ASCII code of first character          | `ASC("A")` → `65`            |
| `STR$(n)`                | Number to string                       | `STR$(42)` → `" 42"`         |
| `VAL(s$)`                | String to number                       | `VAL("3.14")` → `3.14`       |
| `HEX$(n)`                | Number to hexadecimal string           | `HEX$(255)` → `"FF"`         |
| `OCT$(n)`                | Number to octal string                 | `OCT$(8)` → `"10"`           |

### Math Functions

| Function        | Description                    | Example                  |
|-----------------|--------------------------------|--------------------------|
| `ABS(n)`        | Absolute value                 | `ABS(-5)` → `5`         |
| `SGN(n)`        | Sign: -1, 0, or 1             | `SGN(-5)` → `-1`        |
| `INT(n)`        | Floor (toward negative inf)    | `INT(-2.9)` → `-3`      |
| `FIX(n)`        | Truncate toward zero           | `FIX(-2.9)` → `-2`      |
| `SQR(n)`        | Square root                    | `SQR(16)` → `4`         |
| `EXP(n)`        | e to the power n               | `EXP(1)` → `2.718...`   |
| `LOG(n)`        | Natural logarithm              | `LOG(2.718...)` → `1`   |
| `SIN(n)`        | Sine (radians)                 | `SIN(0)` → `0`          |
| `COS(n)`        | Cosine (radians)               | `COS(0)` → `1`          |
| `TAN(n)`        | Tangent (radians)              | `TAN(0)` → `0`          |
| `ATN(n)`        | Arctangent (returns radians)   | `ATN(1)` → `0.7854...`  |
| `RND`           | Random number in [0, 1)        | `RND` → `0.317...`      |

### Type Conversion Functions

| Function   | Converts to   |
|------------|---------------|
| `CINT(n)`  | INTEGER       |
| `CLNG(n)`  | LONG          |
| `CSNG(n)`  | SINGLE        |
| `CDBL(n)`  | DOUBLE        |

### Other Functions

| Function  | Description                              |
|-----------|------------------------------------------|
| `TIMER`   | Seconds elapsed since midnight           |

---

## SWAP

Exchange the values of two variables:

```basic
a = 10
b = 20
SWAP a, b
PRINT a; b    ' Prints 20  10
```

---

## Multiple Statements Per Line

Use `:` to put multiple statements on one line:

```basic
x = 1 : y = 2 : PRINT x + y
```

---

## Program Control

```basic
END     ' Terminate the program
STOP    ' Halt execution (same as END)
```

---

## Example Programs

### FizzBuzz

```basic
FOR i = 1 TO 100
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

### Number Guessing Game

```basic
RANDOMIZE TIMER
secret = INT(RND * 100) + 1
PRINT "I'm thinking of a number between 1 and 100."

DO
    INPUT "Your guess"; guess
    IF guess < secret THEN
        PRINT "Too low!"
    ELSEIF guess > secret THEN
        PRINT "Too high!"
    ELSE
        PRINT "You got it!"
    END IF
LOOP UNTIL guess = secret
```

### Bubble Sort

```basic
CONST N = 10
DIM a(N) AS INTEGER

' Fill with random values
RANDOMIZE TIMER
FOR i = 1 TO N
    a(i) = INT(RND * 100)
NEXT i

' Sort
FOR i = 1 TO N - 1
    FOR j = 1 TO N - i
        IF a(j) > a(j + 1) THEN
            SWAP a(j), a(j + 1)
        END IF
    NEXT j
NEXT i

' Print sorted array
FOR i = 1 TO N
    PRINT a(i);
NEXT i
PRINT
```

### Fibonacci with FUNCTION

```basic
DECLARE FUNCTION Fib(n AS INTEGER)

FOR i = 1 TO 15
    PRINT Fib(i);
NEXT i
PRINT

FUNCTION Fib(n AS INTEGER)
    IF n <= 2 THEN
        Fib = 1
    ELSE
        Fib = Fib(n - 1) + Fib(n - 2)
    END IF
END FUNCTION
```

---

## Differences from QBasic

Rice intentionally omits:

- **Graphics**: No `SCREEN`, `PSET`, `LINE`, `CIRCLE`, `DRAW`, `PAINT`, `PALETTE`, `COLOR` (screen colors)
- **Sound**: No `SOUND`, `BEEP`, `PLAY`
- **Screen control**: No `LOCATE` (cursor positioning), `WIDTH`, `VIEW`, `WINDOW`
- **File I/O**: `OPEN`, `CLOSE`, and related statements are parsed but not yet functional
- **Error handling**: `ON ERROR GOTO` and `RESUME` are parsed but not yet functional
- **User-defined types**: `TYPE...END TYPE` is not yet supported
- **DEFtype**: `DEFINT`, `DEFSNG`, etc. are not yet supported
- **ON n GOTO/GOSUB**: Computed jumps are not yet supported
- **PRINT USING**: Formatted output is parsed but not yet functional

All keywords are case-insensitive: `PRINT`, `Print`, and `print` all work.
