# RICE BASIC Language Reference

RICE BASIC is a structured BASIC interpreter implementing ANSI X3.113-1991 (Full BASIC), with an optional QuickBasic compatibility mode. This document covers the syntax supported by RICE BASIC.

## Running Programs

```bash
rice                  # Start interactive REPL
rice myprogram.bas    # Execute a file
```

In the REPL, type statements and press Enter to execute them. Type `SYSTEM` or press Ctrl+D to exit.

The REPL supports old-school line-number editing: type numbered lines to build a program, then use `RUN`, `LIST`, `NEW`, and `DELETE` to manage and execute it. Unnumbered lines execute immediately.

## Dialects

ANSI mode is the default. QuickBasic compatibility mode can be selected with:

```bash
rice --dialect qb program.bas
rice --compat program.bas
```

or in a complete source file:

```basic
OPTION DIALECT "QB"
```

QuickBasic mode accepts numeric type suffixes in variable names, `-1` true values, string `+` concatenation, bitwise numeric logical operators, `GOSUB`/`RETURN`, `ON ... GOTO`/`ON ... GOSUB`, hex/octal literals, and QuickBasic-style `OPEN file$ FOR mode AS #n` file syntax. See [docs/dialects.md](docs/dialects.md) for the full dialect coverage.

---

## Comments

```basic
REM This is a comment
' This is also a comment
x = 5  ' Inline comment after a statement
```

---

## Data Types

RICE BASIC follows the ANSI standard by default with two fundamental types:

| Type    | Description                                       |
|---------|---------------------------------------------------|
| NUMERIC | All numbers. Stored as double-precision float (f64). |
| STRING  | Variable-length text.                             |

In ANSI mode, there are no type suffixes for numeric subtypes. All numeric values are double-precision. Variables ending in `$` are strings; all others are numeric. QuickBasic mode accepts numeric suffixes as part of variable names; see [docs/dialects.md](docs/dialects.md).

Undeclared variables auto-initialize to `0` (numeric) or `""` (string).

---

## Variables and Constants

```basic
' Implicit declaration (no DIM needed)
x = 10
name$ = "Alice"

' Explicit declaration
DIM count AS NUMERIC
DIM message AS STRING

' Constants (cannot be changed after definition)
CONST PI = 3.14159
CONST MAX_SIZE = 100
CONST GREETING$ = "Hello"

' LET is optional
LET y = 20
y = 20          ' Same thing

' SWAP two variables
SWAP a, b

' CLEAR all variables back to defaults
CLEAR
```

---

## Operators

### Arithmetic

| Operator | Description            | Example          |
|----------|------------------------|------------------|
| `+`      | Addition               | `5 + 3` → `8`   |
| `-`      | Subtraction            | `5 - 3` → `2`   |
| `*`      | Multiplication         | `5 * 3` → `15`  |
| `/`      | Division (float)       | `7 / 2` → `3.5` |
| `MOD`    | Modulo (works on reals) | `7 MOD 3` → `1` |
| `^`      | Exponentiation         | `2 ^ 10` → `1024` |

### String Concatenation

Use `&` for string concatenation (`+` is always arithmetic):

```basic
greeting$ = "Hello, " & "World!"
```

### Comparison

All return `1` (true) or `0` (false):

| Operator | Description       |
|----------|-------------------|
| `=`      | Equal             |
| `<>`     | Not equal         |
| `<`      | Less than         |
| `>`      | Greater than      |
| `<=`     | Less than/equal   |
| `>=`     | Greater than/equal|

### Logical

These are logical (not bitwise) operators:

| Operator | Description      |
|----------|------------------|
| `AND`    | Logical AND      |
| `OR`     | Logical OR       |
| `NOT`    | Logical NOT      |
| `XOR`    | Exclusive OR     |

### Operator Precedence (highest to lowest)

1. `^` (right-associative)
2. Unary `-`, `+`
3. `*`, `/`
4. `MOD`
5. `+`, `-`
6. `&` (string concatenation)
7. `=`, `<>`, `<`, `>`, `<=`, `>=`
8. `NOT`
9. `AND`
10. `OR`
11. `XOR`

---

## Output

### PRINT

```basic
PRINT "Hello, World!"
PRINT 42
PRINT "Value:"; x           ' Semicolon: no gap between items
PRINT "A", "B", "C"         ' Comma: tab to next 16-column zone
PRINT x;                    ' Trailing semicolon: suppress newline
PRINT                       ' Print a blank line
PRINT TAB(20); "Indented"   ' Move to column 20
PRINT SPC(5); "Spaced"      ' Insert 5 spaces
```

**Number formatting**: numbers are printed without leading or trailing spaces. Negative numbers have a leading `-`.

```basic
PRINT 42          ' Outputs: 42
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

### WHILE / END WHILE

```basic
x = 1
WHILE x <= 10
    PRINT x
    x = x + 1
END WHILE
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

### GOTO

```basic
' Jump to a named label
GOTO skip
PRINT "This is skipped"
skip:
PRINT "Jumped here"
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
    PRINT "Hello, " & name & "!"
END SUB
```

### FUNCTION (returns a value)

Assign to the function name to set the return value.

```basic
DECLARE FUNCTION Square(n AS NUMERIC)

PRINT Square(5)        ' Prints 25

FUNCTION Square(n AS NUMERIC)
    Square = n * n
END FUNCTION
```

Functions can be recursive:

```basic
FUNCTION Factorial(n AS NUMERIC)
    IF n <= 1 THEN
        Factorial = 1
    ELSE
        Factorial = n * Factorial(n - 1)
    END IF
END FUNCTION

PRINT Factorial(10)    ' Prints 3628800
```

Use `EXIT SUB` or `EXIT FUNCTION` to return early.

### Parameters: BYVAL (default in ANSI mode)

In ANSI Full BASIC, parameters are passed by value by default. Changes inside the procedure do not affect the original variable:

```basic
SUB Test (x AS NUMERIC)
    x = x + 1        ' Does not affect the caller's variable
END SUB
```

QuickBasic compatibility mode uses `BYREF` by default and supports explicit `BYVAL`/`BYREF`.

---

## User-Defined Types

```basic
TYPE PersonType
    Name AS STRING
    Age AS NUMERIC
    Salary AS NUMERIC
END TYPE

DIM p AS PersonType
p.Name = "Alice"
p.Age = 30
p.Salary = 65000.50
PRINT p.Name; p.Age
```

Arrays of types and passing types to procedures are supported. See the [User-Defined Types guide](docs/user-defined-types.md) for details.

---

## Strings

ANSI Full BASIC supports colon slicing; LEFT$/RIGHT$/MID$ are also available as compatibility functions:

```basic
LET A$ = "Hello, World!"
PRINT A$(1:5)          ' "Hello"
PRINT A$(8:12)         ' "World"
PRINT A$(1:)           ' Entire string from position 1
```

String concatenation uses `&`:

```basic
LET A$ = "Hello" & ", " & "World!"
PRINT A$               ' Hello, World!
```

---

## Arrays

```basic
DIM scores(10) AS NUMERIC         ' Indices 1 to 10 (OPTION BASE 1 is the default)
DIM grid(1 TO 5, 1 TO 5) AS NUMERIC   ' 2D array with explicit bounds
DIM names(20) AS STRING

scores(1) = 95
scores(2) = 87
grid(1, 1) = 3.14

' Change default lower bound
OPTION BASE 0

' Resize a dynamic array
REDIM arr(50) AS NUMERIC
REDIM PRESERVE arr(100) AS NUMERIC   ' Keep existing data

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

## File I/O

RICE BASIC uses ANSI Full BASIC file I/O syntax.

### Opening and Closing Files

```basic
' Open for sequential text output (creates/overwrites)
OPEN #1: NAME "data.txt", ACCESS OUTPUT

' Open for sequential text input (file must exist)
OPEN #1: NAME "data.txt", ACCESS INPUT

' Open for read/write (OUTIN)
OPEN #1: NAME "data.txt", ACCESS OUTIN

' Specify organization explicitly
OPEN #1: NAME "data.bin", ORGANIZATION STREAM, ACCESS OUTIN

' Close a specific file
CLOSE #1

' Close multiple files
CLOSE #1, #2, #3

' Close all open files
CLOSE
```

File numbers range from 1 to 255. Use `FREEFILE` to get the next available number:

```basic
f = FREEFILE
OPEN #f: NAME "myfile.txt", ACCESS OUTPUT
```

Access modes: INPUT, OUTPUT, OUTIN. Organization: SEQUENTIAL (default), STREAM.

### Writing to Files

**PRINT#** -- writes formatted output (same formatting as PRINT):

```basic
OPEN #1: NAME "output.txt", ACCESS OUTPUT
PRINT #1, "Hello, World!"
PRINT #1, x; y; z
CLOSE #1
```

**WRITE#** -- writes comma-separated values with strings in quotes (CSV-style):

```basic
OPEN #1: NAME "data.csv", ACCESS OUTPUT
WRITE #1, "Alice", 30, 95.5
WRITE #1, "Bob", 25, 88.0
CLOSE #1
```

### Reading from Files

**LINE INPUT#** -- reads an entire line:

```basic
OPEN #1: NAME "data.txt", ACCESS INPUT
DO WHILE NOT EOF(1)
    LINE INPUT #1, x$
    PRINT x$
LOOP
CLOSE #1
```

**INPUT#** -- reads comma-delimited fields (pairs with WRITE#):

```basic
OPEN #1: NAME "data.csv", ACCESS INPUT
INPUT #1, name$, age, score
PRINT name$; age; score
CLOSE #1
```

### File Positioning

Use SET POINTER and ASK POINTER for stream I/O positioning:

```basic
OPEN #1: NAME "data.bin", ORGANIZATION STREAM, ACCESS OUTIN
SET #1: POINTER 100       ' Move to byte position 100
ASK #1: POINTER pos       ' Get current position
```

### File Functions

| Function     | Description                                    |
|--------------|------------------------------------------------|
| `FREEFILE`   | Returns lowest unused file number (1-255)      |
| `EOF(n)`     | Returns 1 (true) at end of file, 0 otherwise  |
| `LOF(n)`     | Returns file length in bytes                   |
| `LOC(n)`     | Returns current byte position in file          |

### Complete Example

```basic
' Write records
OPEN #1: NAME "people.txt", ACCESS OUTPUT
WRITE #1, "Alice", 30
WRITE #1, "Bob", 25
WRITE #1, "Carol", 35
CLOSE #1

' Read them back
OPEN #1: NAME "people.txt", ACCESS INPUT
DO WHILE NOT EOF(1)
    INPUT #1, name$, age
    PRINT name$; " is"; age; "years old"
LOOP
CLOSE #1
```

---

## Built-in Functions

### String Functions

| Function                 | Description                            | Example                       |
|--------------------------|----------------------------------------|-------------------------------|
| `LEN(s$)`               | Length of string                        | `LEN("abc")` → `3`           |
| `INSTR(s$, find$)`      | Find substring (0 if not found)        | `INSTR("Hello", "ll")` → `3` |
| `INSTR(start, s$, find$)` | Find from position                   | `INSTR(4, "abcabc", "abc")` → `4` |
| `LTRIM$(s$)`             | Remove leading spaces                  | `LTRIM$("  hi")` → `"hi"`    |
| `RTRIM$(s$)`             | Remove trailing spaces                 | `RTRIM$("hi  ")` → `"hi"`    |
| `SPACE$(n)`              | String of n spaces                     | `SPACE$(3)` → `"   "`        |
| `STRING$(n, s$)`         | Repeat character n times               | `STRING$(5, "*")` → `"*****"` |
| `CHR$(n)`                | Character from ASCII code              | `CHR$(65)` → `"A"`           |
| `ASC(s$)`                | ASCII code of first character          | `ASC("A")` → `65`            |
| `STR$(n)`                | Number to string                       | `STR$(42)` → `"42"`          |
| `VAL(s$)`                | String to number                       | `VAL("3.14")` → `3.14`       |

For substring operations, ANSI-style code should prefer colon slicing such as `A$(3:7)`. LEFT$/RIGHT$/MID$ are available for compatibility.

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
| `ASIN(n)`       | Arc sine                       | `ASIN(1)` → `1.5708...` |
| `ACOS(n)`       | Arc cosine                     | `ACOS(0)` → `1.5708...` |
| `COT(n)`        | Cotangent                      | `COT(1)` → `0.6421...`  |
| `CSC(n)`        | Cosecant                       | `CSC(1)` → `1.1884...`  |
| `SEC(n)`        | Secant                         | `SEC(0)` → `1`          |
| `ANGLE(x, y)`   | Angle of vector (x, y)        | `ANGLE(1, 1)` → `0.785...` |
| `CEIL(n)`       | Ceiling                        | `CEIL(2.1)` → `3`       |
| `TRUNCATE(n)`   | Truncate toward zero           | `TRUNCATE(-2.9)` → `-2` |
| `REMAINDER(a,b)` | IEEE remainder                | `REMAINDER(7, 3)` → `1` |
| `ROUND(n)`      | Round to nearest integer       | `ROUND(2.5)` → `3`      |
| `MAXNUM`        | Maximum numeric value          | `MAXNUM` → `1.798e+308` |
| `PI`            | Pi constant                    | `PI` → `3.14159...`     |
| `RND`           | Random number in [0, 1)        | `RND` → `0.317...`      |

### Date/Time Functions

| Function  | Description                              |
|-----------|------------------------------------------|
| `DATE$`   | Current date as MM-DD-YYYY               |
| `TIME$`   | Current time as HH:MM:SS                 |
| `TIMER`   | Seconds elapsed since midnight           |

### System Functions

| Function        | Description                              |
|-----------------|------------------------------------------|
| `ENVIRON$(s$)`  | Get environment variable value           |
| `FREEFILE`      | Next available file number               |
| `EOF(n)`        | End-of-file test (1 if true, 0 if false) |
| `LOF(n)`        | File length in bytes                     |
| `LOC(n)`        | Current position in file                 |

---

## Multiple Statements Per Line

Use `:` to put multiple statements on one line:

```basic
x = 1 : y = 2 : PRINT x + y
```

---

## Error Handling

RICE BASIC uses ANSI structured exception handling (not ON ERROR GOTO):

### WHEN EXCEPTION

```basic
WHEN EXCEPTION IN
    ' Code that might cause errors
    OPEN #1: NAME "missing.txt", ACCESS INPUT
    LINE INPUT #1, x$
    CLOSE #1
USE
    PRINT "Error:"; EXTYPE; EXTEXT$
END WHEN
```

### EXTYPE and EXTEXT$

| Function   | Description                                      |
|------------|--------------------------------------------------|
| `EXTYPE`   | Numeric exception type code                      |
| `EXTEXT$`  | Descriptive text for the exception               |

### RETRY and CONTINUE

Use `RETRY` in the `USE` block to re-execute the statement that caused the exception:

```basic
DIM filename AS STRING
filename = "primary.txt"

WHEN EXCEPTION IN
    OPEN #1: NAME filename, ACCESS INPUT
USE
    filename = "backup.txt"
    RETRY
END WHEN
```

Use `CONTINUE` to skip the failed statement and resume with the next one:

```basic
WHEN EXCEPTION IN
    x = 1 / 0         ' Division by zero - will be skipped
    PRINT "Continued"  ' This runs after CONTINUE
USE
    PRINT "Skipping error: "; EXTEXT$
    CONTINUE
END WHEN
```

---

## PRINT USING

Format output using a template string. The format string is followed by a semicolon and then the values to format.

```basic
PRINT USING "format"; value1; value2; ...
PRINT #n, USING "format"; value1; value2; ...
```

### Numeric Format Specifiers

| Specifier | Description                                            | Example                           |
|-----------|--------------------------------------------------------|-----------------------------------|
| `#`       | Digit placeholder (space-padded, right-aligned)        | `"###"` with 5 → `"  5"`         |
| `.`       | Decimal point position                                 | `"##.##"` with 1.5 → `" 1.50"`   |
| `+`       | Show sign (leading or trailing)                        | `"+##"` with 5 → `"+ 5"`         |
| `-`       | Trailing minus (negative only)                         | `"##-"` with -5 → `" 5-"`        |
| `$$`      | Floating dollar sign                                   | `"$$##.##"` with 1.5 → `" $1.50"` |
| `**`      | Fill leading spaces with asterisks                     | `"**##.##"` with 1 → `"***1.00"` |
| `**$`     | Asterisk fill with floating dollar                     | `"**$##.##"` with 1 → `"**$1.00"` |
| `,`       | Thousands separator (before decimal point)             | `"#,###"` with 1234 → `"1,234"`  |
| `^^^^`    | Scientific notation exponent                           | `"##.##^^^^"` with 1234 → `"12.34E+02"` |

### String Format Specifiers

| Specifier   | Description                             | Example                            |
|-------------|-----------------------------------------|------------------------------------|
| `!`         | First character only                    | `"!"` with `"Hello"` → `"H"`      |
| `\ \`       | Fixed-width field (width = chars between `\`)| `"\   \"` with `"Hi"` → `"Hi   "` |
| `&`         | Entire string as-is                     | `"&"` with `"Hello"` → `"Hello"`  |

### Special Characters

| Character | Description                 |
|-----------|-----------------------------|
| `_`       | Next character is literal   |

### Overflow

If a number is too wide for the format field, the output is prefixed with `%`.

### Format Repetition

If there are more values than format fields, the format string repeats automatically:

```basic
PRINT USING "###"; 1; 2; 3    ' Prints "  1  2  3"
```

---

## Console Features

### Cursor and Screen

```basic
CLS                       ' Clear screen
LOCATE row, col           ' Move cursor to row, col (1-based)
COLOR foreground, background  ' Set text colors (ANSI codes)
BEEP                      ' Sound a terminal bell
WIDTH columns             ' Set terminal width
VIEW PRINT top TO bottom  ' Set scrolling region
VIEW PRINT                ' Reset scrolling region
```

### Console Functions

| Function          | Description                                      |
|-------------------|--------------------------------------------------|
| `CSRLIN`          | Returns the current cursor row (1-based)         |
| `POS(0)`          | Returns the current cursor column (1-based)      |
| `INKEY$`          | Reads a key without waiting (returns "" if none) |
| `INPUT$(n)`       | Reads n characters from keyboard                 |
| `INPUT$(n, #f)`   | Reads n bytes from file #f                       |
| `SCREEN(r, c)`    | Returns ASCII code of character at row r, col c  |

---

## MAT Operations

MAT support for numeric arrays:

```basic
DIM A(3, 3), B(3, 3), C(3, 3)
MAT A = ZER
MAT B = IDN
MAT C = A + B
MAT PRINT C
```

Operations: MAT PRINT, MAT READ, MAT INPUT, MAT arithmetic (+, -, *), scalar multiply, INV (inverse), TRN (transpose), DET (determinant), ZER (zeros), CON (ones), IDN (identity). See the [MAT Operations guide](docs/mat-operations.md) for details.

---

## REPL Line-Number Mode

The interactive REPL supports classic BASIC line-number editing:

```
Ok
10 PRINT "Hello"
20 PRINT "World"
LIST
10 PRINT "Hello"
20 PRINT "World"
RUN
Hello
World
Ok
```

| Command              | Description                           |
|----------------------|---------------------------------------|
| `RUN`                | Execute the stored program            |
| `LIST`               | Display all stored lines              |
| `LIST 10`            | Display line 10                       |
| `LIST 10-50`         | Display lines 10 through 50          |
| `NEW`                | Clear the stored program              |
| `DELETE 10`          | Delete line 10                        |
| `DELETE 10-50`       | Delete lines 10 through 50           |

Typing a line number with a statement stores it. Typing a bare line number deletes that line. Lines without numbers execute immediately.

---

## Program Control

```basic
END     ' Terminate the program
STOP    ' Halt execution (same as END)
SYSTEM  ' Exit to system (also exits the REPL)
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
DIM a(N) AS NUMERIC

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
DECLARE FUNCTION Fib(n AS NUMERIC)

FOR i = 1 TO 15
    PRINT Fib(i);
NEXT i
PRINT

FUNCTION Fib(n AS NUMERIC)
    IF n <= 2 THEN
        Fib = 1
    ELSE
        Fib = Fib(n - 1) + Fib(n - 2)
    END IF
END FUNCTION
```

---

## Limitations

RICE BASIC intentionally omits or limits:

- **Graphics modes**: No `SCREEN` (mode switching), `PSET`, `LINE`, `CIRCLE`, `DRAW`, `PAINT`, `PALETTE`, `WINDOW`
- **Sound**: No `SOUND`, `PLAY`
- **Memory access**: No `DEF SEG`, `PEEK`, `POKE`
- **Legacy error handling**: No `ON ERROR GOTO`, `ERR`, `ERL` (use `WHEN EXCEPTION` instead)
- **Legacy procedures**: `GOSUB`/`RETURN` are QuickBasic-mode only; `DEF FN` is not supported (use `SUB`/`FUNCTION` instead)
- **String slicing**: ANSI colon slicing is supported; `LEFT$`, `RIGHT$`, and `MID$` are also available as compatibility functions
- **Legacy operators**: No `\` (integer division), `IMP`, `EQV`
- **Proper array storage**: Arrays use a flattened key representation; `LBOUND`/`UBOUND` are stubs only

RICE BASIC supports text-mode console features including `CLS`, `LOCATE`, `COLOR`, `BEEP`, `WIDTH`, `VIEW PRINT`, `CSRLIN`, `POS`, `INKEY$`, `INPUT$`, and `SCREEN()`.

All keywords are case-insensitive: `PRINT`, `Print`, and `print` all work.
