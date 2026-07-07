# RICE BASIC Language Reference

This reference describes shared RICE BASIC syntax and calls out dialect differences where relevant. RICE BASIC uses QBasic 1.1 compatibility mode by default and also provides ANSI X3.113-1991 mode; see [Dialects](dialects.md) for the full compatibility table.

## Running and Dialect Selection

```bash
rice program.bas
rice --dialect ansi program.bas
rice --dialect qb program.bas
rice --compat program.bas
```

QBasic 1.1 compatibility mode is the default. ANSI mode can be requested inside a complete source file:

```basic
OPTION DIALECT "ANSI"
```

`OPTION DIALECT "QB"`, `OPTION DIALECT "QBasic 1.1"`, `--dialect qb`, and `--compat` select the default QBasic-compatible mode explicitly.

## Data Types

RICE BASIC provides two fundamental runtime data types:

| Type    | Description                                              |
|---------|----------------------------------------------------------|
| NUMERIC | All numbers (integer and floating-point). Stored as 64-bit float (`f64`). |
| STRING  | Variable-length text.                                    |

In ANSI mode, `$` marks string variables and numeric variables have no type suffixes. QuickBasic mode accepts `%`, `!`, `#`, `&`, and `$` suffixes as part of variable names, but numeric values are still stored as `f64`. The default type for undeclared variables is NUMERIC.

### Auto-Initialization

Variables that are used before being assigned are auto-initialized: numeric types to `0`, strings to `""`.

### Truth Values

Following the ANSI standard:
- **True** = `1`
- **False** = `0`

Any non-zero value is considered true in conditional expressions.

In QuickBasic mode, comparisons return `-1` for true and `0` for false.

---

## Variables and Declarations

### Implicit Declaration

Variables can be used without declaration. Numeric is the default type:

```basic
x = 10
name = "Alice"
count = 0
total = 0.0
```

### OPTION EXPLICIT

Require every variable to be explicitly declared before use. A variable is considered declared when it appears in `DIM`, `REDIM`, `SHARED`, `STATIC`, `COMMON`, `CONST`, a `DEFtype` letter range, a type suffix, a procedure parameter list, or a `FOR` loop variable.

```basic
OPTION EXPLICIT
DIM x AS NUMERIC
x = 10
' y = 20   ' would raise an error — Y is not declared
```

`OPTION EXPLICIT` is a global directive; once enabled, it applies to the whole program.

### DIM

Explicitly declare variables and arrays:

```basic
DIM x AS NUMERIC
DIM name AS STRING
DIM scores(10) AS NUMERIC         ' Array with indices 1-10
DIM grid(3, 4) AS NUMERIC         ' 2D array
DIM items(1 TO 100) AS STRING     ' Custom bounds
```

### CONST

Define constants that cannot be reassigned:

```basic
CONST PI = 3.14159265358979
CONST MAX_SIZE = 100
CONST GREETING = "Hello"
```

### LET

The `LET` keyword is optional for assignment:

```basic
LET x = 10   ' Explicit LET
x = 10       ' Same thing
```

### COMMON

Declare variables as shared across the module. `COMMON` and `COMMON SHARED` are equivalent in RICE BASIC.

```basic
COMMON SHARED total, count
DIM total AS NUMERIC
DIM count AS NUMERIC

SUB AddToTotal(n AS NUMERIC)
    total = total + n
END SUB
```

`CHAIN` is not supported; `COMMON` is provided for single-source shared-state declarations.

### SWAP

Exchange the values of two variables:

```basic
SWAP a, b
```

### CLEAR

Reset all variables to their default values (0 or ""):

```basic
CLEAR
```

---

## Operators

### Arithmetic Operators

| Operator | Description             | Example        |
|----------|-------------------------|----------------|
| `+`      | Addition                | `3 + 4` = 7   |
| `-`      | Subtraction             | `10 - 3` = 7  |
| `*`      | Multiplication          | `3 * 4` = 12  |
| `/`      | Division                | `7 / 2` = 3.5 |
| `MOD`    | Modulo (remainder)      | `7 MOD 3` = 1 |
| `^`      | Exponentiation          | `2 ^ 3` = 8   |

### Comparison Operators

All comparisons return `1` (true) or `0` (false):

| Operator | Description           |
|----------|-----------------------|
| `=`      | Equal to              |
| `<>`     | Not equal to          |
| `<`      | Less than             |
| `>`      | Greater than          |
| `<=`     | Less than or equal    |
| `>=`     | Greater than or equal |

### Logical Operators

In QBasic mode, these operators perform bitwise operations on numeric values. In ANSI mode, they operate on truth values:

| Operator | Description      | Example                    |
|----------|------------------|----------------------------|
| `AND`    | Logical AND      | `(x > 0) AND (x < 10)`    |
| `OR`     | Logical OR       | `(x = 0) OR (x = 1)`      |
| `NOT`    | Logical NOT      | `NOT (x = 0)`             |
| `XOR`    | Exclusive OR     | `(x = 1) XOR (y = 1)`     |

### String Concatenation

In QBasic mode, `+` concatenates strings and `&` also concatenates strings:

```basic
result$ = "Hello" + ", " + "World!"
```

In ANSI mode, use `&`; `+` remains arithmetic.

### Operator Precedence

From highest to lowest:

| Precedence | Operator(s)                      | Associativity |
|------------|----------------------------------|---------------|
| 1 (highest)| `^`                              | Right         |
| 2          | Unary `-`, `+`                   | Prefix        |
| 3          | `*`, `/`                         | Left          |
| 4          | `MOD`                            | Left          |
| 5          | `+`, `-`, `&`                    | Left          |
| 6          | `=`, `<>`, `<`, `>`, `<=`, `>=`  | Left          |
| 7          | `NOT`                            | Prefix        |
| 8          | `AND`                            | Left          |
| 9          | `OR`                             | Left          |
| 10 (lowest)| `XOR`                            | Left          |

Use parentheses to override precedence:

```basic
result = (2 + 3) * 4   ' = 20, not 14
```

---

## Control Flow

### IF...THEN...ELSE

**Block form:**

```basic
IF condition THEN
    ' statements
ELSEIF condition THEN
    ' statements
ELSE
    ' statements
END IF
```

**Single-line form:**

```basic
IF x > 0 THEN PRINT "Positive" ELSE PRINT "Non-positive"
```

### SELECT CASE

```basic
SELECT CASE grade
    CASE "A"
        PRINT "Excellent"
    CASE "B", "C"
        PRINT "Good"
    CASE "D" TO "F"
        PRINT "Needs improvement"
    CASE IS >= 90
        PRINT "High score"
    CASE ELSE
        PRINT "Unknown"
END SELECT
```

Case tests support:
- Single values: `CASE 1`
- Multiple values: `CASE 1, 2, 3`
- Ranges: `CASE 1 TO 10`
- Comparisons: `CASE IS > 100`

### FOR...NEXT

```basic
FOR i = 1 TO 10
    PRINT i
NEXT i

FOR i = 10 TO 1 STEP -1
    PRINT i
NEXT i

' STEP is optional; defaults to 1
FOR i = 0 TO 1 STEP 0.1
    PRINT i
NEXT i
```

Use `EXIT FOR` to leave a FOR loop early.

### DO...LOOP

Four variations:

```basic
' Test at top (WHILE)
DO WHILE count < 10
    count = count + 1
LOOP

' Test at bottom (WHILE)
DO
    count = count + 1
LOOP WHILE count < 10

' Test at top (UNTIL)
DO UNTIL count >= 10
    count = count + 1
LOOP

' Test at bottom (UNTIL)
DO
    count = count + 1
LOOP UNTIL count >= 10
```

Use `EXIT DO` to leave a DO loop early.

### GOTO

```basic
GOTO myLabel
PRINT "This is skipped"
myLabel:
PRINT "Jumped here"
```

Line numbers are also supported:

```basic
10 PRINT "Start"
20 GOTO 10
```

### GOSUB / RETURN

`GOSUB` and `RETURN` are available in the default QBasic compatibility mode and unavailable in ANSI mode:

```basic
GOSUB 100
PRINT "back"
END

100 PRINT "inside subroutine"
RETURN
```

QuickBasic mode also supports computed jumps:

```basic
ON choice GOTO 100, 200, 300
ON choice GOSUB 100, 200, 300
```

### ON ERROR / RESUME

Classic QuickBasic-style error handlers are available in the default QBasic-compatible mode at top-level scope:

```basic
10 ON ERROR GOTO 100
20 PRINT 1 / 0
30 END
100 PRINT ERR; ERL
110 RESUME NEXT
```

`ERROR n` raises a BASIC error code. `ERR` returns the most recent code, and `ERL` returns the numbered line where the error occurred, or 0 for unnumbered statements. `RESUME`, `RESUME NEXT`, and `RESUME label` are exact for top-level statements; nested-block resume behavior is limited by the interpreter's current control-flow model.

### END / STOP / SYSTEM

```basic
END       ' End program execution
STOP      ' Stop execution
SYSTEM    ' Exit to system
```

When running from the REPL, `END` and `STOP` end the BASIC program normally. `Ctrl+D` exits at the REPL prompt. There is not currently a graceful break key that returns to `Ok` while a BASIC program is running.

---

## Arrays

### Declaration

```basic
DIM arr(10) AS NUMERIC           ' Indices 1-10 (OPTION BASE 1 is the default)
DIM arr(1 TO 10) AS NUMERIC      ' Indices 1-10
DIM matrix(3, 4) AS NUMERIC      ' 2D array
DIM cube(2, 3, 4) AS NUMERIC     ' 3D array
```

### OPTION BASE

Set the default lower bound for arrays. ANSI BASIC defaults to 1:

```basic
OPTION BASE 0    ' Arrays start at 0 instead of 1
DIM arr(10)      ' Now indices 0-10
```

### REDIM

Resize an array dynamically:

```basic
REDIM arr(20) AS NUMERIC              ' Resize, contents cleared
REDIM PRESERVE arr(30) AS NUMERIC     ' Resize, contents preserved
```

### ERASE

Reset arrays to default values:

```basic
ERASE arr, matrix
```

See also [MAT Operations](mat-operations.md) for matrix initialization and arithmetic.

---

## Input and Output

### PRINT

```basic
PRINT "Hello, World!"
PRINT x; y; z            ' Semicolons: no space between items
PRINT x, y, z            ' Commas: tab to next column zone
PRINT "Value: "; x;      ' Trailing semicolon: no newline
PRINT                    ' Blank line
PRINT TAB(20); "Column 20"
PRINT SPC(10); "After 10 spaces"
```

### PRINT USING

Formatted output (see [PRINT USING Formatting](print-using.md) for full details):

```basic
PRINT USING "###.##"; 3.14159        '   3.14
PRINT USING "$$#,###.##"; 1234.5     ' $1,234.50
```

### INPUT

```basic
INPUT x                        ' Prompt with "? "
INPUT "Enter value: ", x      ' Comma: prompt with no "? "
INPUT "Enter value: "; x      ' Semicolon: prompt with "? " appended
INPUT "Name, Age: "; n, age   ' Multiple variables
```

### LINE INPUT

Read an entire line (no parsing of commas):

```basic
LINE INPUT "Enter text: "; text
```

---

## Built-in Functions

### Math Functions

| Function            | Description                          | Example              |
|---------------------|--------------------------------------|----------------------|
| `ABS(n)`            | Absolute value                       | `ABS(-5)` = 5       |
| `INT(n)`            | Floor (greatest integer <= n)        | `INT(3.7)` = 3      |
| `FIX(n)`            | Truncate toward zero                 | `FIX(-3.7)` = -3    |
| `SGN(n)`            | Sign: -1, 0, or 1                   | `SGN(-5)` = -1      |
| `SQR(n)`            | Square root                          | `SQR(9)` = 3        |
| `EXP(n)`            | e raised to the power n             | `EXP(1)` = 2.718... |
| `LOG(n)`            | Natural logarithm                    | `LOG(2.718)` = ~1   |
| `SIN(n)`            | Sine (radians)                       | `SIN(0)` = 0        |
| `COS(n)`            | Cosine (radians)                     | `COS(0)` = 1        |
| `TAN(n)`            | Tangent (radians)                    | `TAN(0)` = 0        |
| `ATN(n)`            | Arctangent (returns radians)         | `ATN(1)` = 0.785... |
| `ASIN(n)`           | Arc sine (returns radians)           | `ASIN(1)` = 1.570...|
| `ACOS(n)`           | Arc cosine (returns radians)         | `ACOS(1)` = 0       |
| `COT(n)`            | Cotangent                            | `COT(1)` = 0.642... |
| `CSC(n)`            | Cosecant                             | `CSC(1)` = 1.188... |
| `SEC(n)`            | Secant                               | `SEC(0)` = 1        |
| `ANGLE(x, y)`       | Two-argument arctangent              | `ANGLE(1, 1)` = 0.785... |
| `ROUND(n[, places])` | Round to nearest                    | `ROUND(3.7)` = 4    |
| `CEIL(n)`           | Ceiling (smallest integer >= n)      | `CEIL(3.2)` = 4     |
| `CINT(n)`           | Round to nearest integer (half→even) | `CINT(2.5)` = 2     |
| `CLNG(n)`           | Round to nearest long (half→even)    | `CLNG(2.5)` = 2     |
| `CSNG(n)`           | Reduce to single precision           | `CSNG(1.1)`         |
| `CDBL(n)`           | Double-precision value               | `CDBL(3)` = 3       |
| `TRUNCATE(n, places)` | Truncate to decimal places         | `TRUNCATE(3.789, 2)` = 3.78 |
| `REMAINDER(a, b)`   | IEEE remainder                       | `REMAINDER(7, 3)` = 1 |
| `MAXNUM`            | Largest representable number         | 1.7976...e+308      |
| `PI`                | Value of pi                          | 3.14159...           |

### Random Numbers

```basic
RANDOMIZE seed     ' Seed the generator
RANDOMIZE          ' Seed with system time
x = RND            ' Next random number (0 to 1, exclusive)
```

### String Functions

| Function               | Description                              | Example                       |
|------------------------|------------------------------------------|-------------------------------|
| `LEN(s)`               | Length of string                          | `LEN("Hi")` = 2             |
| `INSTR(s, find)`       | Find substring (0 if not found)          | `INSTR("Hello", "ll")` = 3  |
| `INSTR(start, s, find)`| Find from position                       | `INSTR(4, "abcabc", "abc")` = 4 |
| `LTRIM$(s)`            | Remove leading spaces                    | `LTRIM$("  hi")` = "hi"     |
| `RTRIM$(s)`            | Remove trailing spaces                   | `RTRIM$("hi  ")` = "hi"     |
| `SPACE$(n)`            | String of n spaces                       | `SPACE$(3)` = "   "          |
| `STRING$(n, ch)`       | Repeat character n times                 | `STRING$(3, "*")` = "***"   |
| `CHR$(n)`              | Character from ASCII code                | `CHR$(65)` = "A"             |
| `ASC(s)`               | ASCII code of first character            | `ASC("A")` = 65              |
| `STR$(n)`              | Number to string                         | `STR$(42)` = "42"            |
| `VAL(s)`               | Parse number from string                 | `VAL("42")` = 42             |
| `LEFT$(s, n)`          | Leftmost n characters                    | `LEFT$("Hello", 3)` = "Hel" |
| `RIGHT$(s, n)`         | Rightmost n characters                   | `RIGHT$("Hello", 3)` = "llo"|
| `MID$(s, start[, len])`| Substring from position                  | `MID$("Hello", 2, 3)` = "ell"|
| `UCASE$(s)`            | Convert to uppercase                     | `UCASE$("hi")` = "HI"       |
| `LCASE$(s)`            | Convert to lowercase                     | `LCASE$("HI")` = "hi"       |
| `HEX$(n)`              | Hexadecimal representation               | `HEX$(255)` = "FF"          |
| `OCT$(n)`              | Octal representation                     | `OCT$(8)` = "10"            |
| `MKI$(n)`              | Packed 2-byte integer string             | `LEN(MKI$(1))` = 2           |
| `MKL$(n)`              | Packed 4-byte long string                | `LEN(MKL$(1))` = 4           |
| `MKS$(n)`              | Packed 4-byte single string              | `LEN(MKS$(1))` = 4           |
| `MKD$(n)`              | Packed 8-byte double string              | `LEN(MKD$(1))` = 8           |
| `CVI(s)`               | Convert packed integer string            | `CVI(MKI$(1))` = 1           |
| `CVL(s)`               | Convert packed long string               | `CVL(MKL$(1))` = 1           |
| `CVS(s)`               | Convert packed single string             | `CVS(MKS$(1))` = 1           |
| `CVD(s)`               | Convert packed double string             | `CVD(MKD$(1))` = 1           |

ANSI Full BASIC also supports colon slicing as an alternative to LEFT$/MID$/RIGHT$. See [String Slicing](string-slicing.md).

### System Functions

| Function       | Description                          | Example Return            |
|----------------|--------------------------------------|---------------------------|
| `TIMER`        | Seconds since midnight               | `43261.5`                |
| `DATE$`        | Current date string (MM-DD-YYYY)     | `"03-08-2026"`           |
| `TIME$`        | Current time string (HH:MM:SS)      | `"14:30:45"`             |
| `ENVIRON$(s)`  | Get environment variable             | `ENVIRON$("PATH")`      |

`DATE$` and `TIME$` can be assigned to override the values returned by subsequent reads (the host clock is not changed):

```basic
DATE$ = "12-25-2024"
TIME$ = "14:30:00"
PRINT DATE$; " "; TIME$
```

`ENVIRON "name=value"` sets an environment variable.
| `CURDIR$`      | Current working directory            | `CURDIR$`                 |
| `COMMAND$`     | Command-line tail                    | `COMMAND$`                |
| `FREEFILE`     | Next available file number           | `1`                      |
| `EOF(n)`       | End-of-file test (dialect true value if true) | `EOF(1)`                 |
| `LOF(n)`       | File length in bytes                 | `LOF(1)`                 |
| `LOC(n)`       | Current position in file             | `LOC(1)`                 |
| `SEEK(n)`      | Next file byte position (1-based)    | `SEEK(1)`                |
| `ERR`          | Last classic BASIC error code        | `ERR`                    |
| `ERL`          | Numbered line of last classic error  | `ERL`                    |
| `LBOUND(a[,d])`| Lower bound of array dimension       | `LBOUND(a)`              |
| `UBOUND(a[,d])`| Upper bound of array dimension       | `UBOUND(a, 2)`           |
| `CSRLIN`       | Current cursor row (1-based)         | `CSRLIN`                 |
| `POS(0)`       | Current cursor column (1-based)      | `POS(0)`                 |
| `INKEY$`       | Read key without waiting ("" if none)| `INKEY$`                 |
| `INPUT$(n)`    | Read n characters from keyboard      | `INPUT$(1)`              |
| `SCREEN(r, c)` | ASCII code of character at position  | `SCREEN(1, 1)`           |

---

## DATA, READ, and RESTORE

Store and retrieve inline data:

```basic
DATA 10, 20, 30, "Hello", "World"

READ a, b, c
READ d, e
PRINT a; b; c      ' 10 20 30
PRINT d & " " & e  ' Hello World

RESTORE             ' Reset data pointer to beginning
READ x
PRINT x             ' 10

myData:
DATA 100, 200
RESTORE myData      ' Reset to specific label
READ y
PRINT y             ' 100
```

---

## Comments

```basic
REM This is a full-line comment
' This is also a comment (apostrophe form)
x = 10 ' Inline comment after a statement
```

---

## Line Structure

### Statement Separators

Multiple statements on one line with colons:

```basic
x = 1 : y = 2 : PRINT x + y
```

### Line Numbers and Labels

Both line numbers and named labels are supported as jump targets:

```basic
10 PRINT "Line 10"
20 GOTO 10

myLabel:
PRINT "At myLabel"
GOTO myLabel
```

### Case Insensitivity

All keywords and identifiers are case-insensitive. `PRINT`, `print`, and `Print` are identical.

---

## System Statements

### SHELL

Execute a system command:

```basic
SHELL "ls -la"
SHELL "dir"
```

### SLEEP

Pause execution for a given number of seconds:

```basic
SLEEP 2    ' Sleep for 2 seconds
SLEEP      ' Sleep indefinitely (until interrupted)
```

### Console Statements

```basic
CLS                           ' Clear screen
LOCATE row, col               ' Move cursor (1-based)
COLOR fg[, bg]                ' Set text colors (0-255)
BEEP                          ' Sound terminal bell
WIDTH columns                 ' Set terminal width
VIEW PRINT top TO bottom      ' Set scrolling region
VIEW PRINT                    ' Reset scrolling region
```

### File System Operations

```basic
MKDIR "newdir"                 ' Create directory
RMDIR "newdir"                 ' Remove directory
CHDIR "/path/to/dir"           ' Change directory
CHDRIVE "C"                    ' Change current drive, where available
FILES "."                      ' List directory entries
NAME "old.txt" AS "new.txt"    ' Rename file
KILL "temp.txt"                ' Delete file
```
