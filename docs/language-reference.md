# RICE BASIC Language Reference

## Data Types

RICE BASIC has five built-in data types:

| Type    | Suffix | Description                                                        |
|---------|--------|--------------------------------------------------------------------|
| INTEGER | `%`    | Whole numbers. Internally stored as 64-bit integer (`i64`).        |
| LONG    | `&`    | Whole numbers. Internally stored as 64-bit integer (`i64`).        |
| SINGLE  | `!`    | Floating-point. Internally stored as 64-bit float (`f64`).         |
| DOUBLE  | `#`    | Floating-point. Internally stored as 64-bit float (`f64`).         |
| STRING  | `$`    | Variable-length text.                                              |

> **Note:** Unlike QBasic, INTEGER and LONG both use 64-bit storage internally, so they are not limited to 16-bit or 32-bit ranges in practice. The `CINT()` function enforces the classic -32,768 to 32,767 range, and `CLNG()` enforces -2,147,483,648 to 2,147,483,647, but regular arithmetic on INTEGER/LONG variables does not clip to those ranges. SINGLE and DOUBLE are both stored as 64-bit floats (`f64`), so there is no precision difference between them in RICE BASIC.

Numeric literals without a decimal point are INTEGER (or LONG if they exceed INTEGER range). Literals with a decimal point or exponent notation are DOUBLE by default.

### Type Coercion

When mixing numeric types in expressions, the narrower type is promoted to the wider type:

```
INTEGER < LONG < SINGLE < DOUBLE
```

### Auto-Initialization

Variables that are used before being assigned are auto-initialized: numeric types to `0`, strings to `""`.

### Truth Values

Following QBasic convention:
- **True** = `-1`
- **False** = `0`

Any non-zero value is considered true in conditional expressions.

---

## Variables and Declarations

### Implicit Declaration

Variables can be used without declaration. The type is determined by the suffix:

```basic
x = 10          ' Numeric (default)
name$ = "Alice" ' String
count% = 0      ' Integer
total# = 0.0    ' Double
```

### DIM

Explicitly declare variables and arrays:

```basic
DIM x AS INTEGER
DIM name AS STRING
DIM scores(10) AS DOUBLE          ' Array with indices 0-10
DIM grid(3, 4) AS INTEGER         ' 2D array
DIM items(1 TO 100) AS STRING     ' Custom bounds
```

### CONST

Define constants that cannot be reassigned:

```basic
CONST PI = 3.14159265358979
CONST MAX_SIZE = 100
CONST GREETING$ = "Hello"
```

### LET

The `LET` keyword is optional for assignment:

```basic
LET x = 10   ' Explicit LET
x = 10       ' Same thing
```

### DEFTYPE Statements

Set the default type for variables based on their first letter:

```basic
DEFINT A-D        ' Variables starting with A, B, C, D default to INTEGER
DEFLNG E-H        ' Default to LONG
DEFSNG I-L        ' Default to SINGLE
DEFDBL M-P        ' Default to DOUBLE
DEFSTR S-T        ' Default to STRING
```

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

| Operator | Description             | Example      |
|----------|-------------------------|--------------|
| `+`      | Addition                | `3 + 4` = 7  |
| `-`      | Subtraction             | `10 - 3` = 7 |
| `*`      | Multiplication          | `3 * 4` = 12 |
| `/`      | Floating-point division | `7 / 2` = 3.5|
| `\`      | Integer division        | `7 \ 2` = 3  |
| `MOD`    | Modulo (remainder)      | `7 MOD 3` = 1|
| `^`      | Exponentiation          | `2 ^ 3` = 8  |

### Comparison Operators

All comparisons return `-1` (true) or `0` (false):

| Operator | Description           |
|----------|-----------------------|
| `=`      | Equal to              |
| `<>`     | Not equal to          |
| `<`      | Less than             |
| `>`      | Greater than          |
| `<=`     | Less than or equal    |
| `>=`     | Greater than or equal |

### Logical / Bitwise Operators

| Operator | Description      | Example                    |
|----------|------------------|----------------------------|
| `AND`    | Logical AND      | `(x > 0) AND (x < 10)`    |
| `OR`     | Logical OR       | `(x = 0) OR (x = 1)`      |
| `NOT`    | Logical NOT      | `NOT (x = 0)`             |
| `XOR`    | Exclusive OR     | `a XOR b`                 |
| `EQV`    | Equivalence      | `a EQV b`                 |
| `IMP`    | Implication      | `a IMP b`                 |

On integer operands, these operators work bitwise.

### String Concatenation

```basic
result$ = "Hello" + ", " + "World!"
```

### Operator Precedence

From highest to lowest:

| Precedence | Operator(s)                      | Associativity |
|------------|----------------------------------|---------------|
| 1 (highest)| `^`                              | Right         |
| 2          | Unary `-`, `+`                   | Prefix        |
| 3          | `*`, `/`                         | Left          |
| 4          | `\`                              | Left          |
| 5          | `MOD`                            | Left          |
| 6          | `+`, `-`                         | Left          |
| 7          | `=`, `<>`, `<`, `>`, `<=`, `>=`  | Left          |
| 8          | `NOT`                            | Prefix        |
| 9          | `AND`                            | Left          |
| 10         | `OR`                             | Left          |
| 11         | `XOR`                            | Left          |
| 12         | `EQV`                            | Left          |
| 13 (lowest)| `IMP`                            | Left          |

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

### WHILE...WEND

```basic
WHILE condition
    ' statements
WEND
```

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

```basic
GOSUB printHeader
PRINT "Main program"
END

printHeader:
PRINT "=== Header ==="
RETURN
```

### ON n GOTO / ON n GOSUB

Computed branching:

```basic
ON choice GOTO label1, label2, label3
ON choice GOSUB sub1, sub2, sub3
```

If `choice` is out of range (less than 1 or greater than the number of labels), execution falls through to the next statement.

### END / STOP / SYSTEM

```basic
END       ' End program execution
STOP      ' Stop execution
SYSTEM    ' Exit to system
```

---

## Arrays

### Declaration

```basic
DIM arr(10) AS INTEGER           ' Indices 0-10 (11 elements)
DIM arr(1 TO 10) AS INTEGER      ' Indices 1-10 (10 elements)
DIM matrix(3, 4) AS DOUBLE       ' 2D array
DIM cube(2, 3, 4) AS INTEGER    ' 3D array
```

### OPTION BASE

Set the default lower bound for arrays:

```basic
OPTION BASE 1    ' Arrays start at 1 instead of 0
DIM arr(10)      ' Now indices 1-10
```

### REDIM

Resize an array dynamically:

```basic
REDIM arr(20) AS INTEGER              ' Resize, contents cleared
REDIM PRESERVE arr(30) AS INTEGER     ' Resize, contents preserved
```

### ERASE

Reset arrays to default values:

```basic
ERASE arr, matrix
```

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
PRINT USING "###.##"; 3.14159        ' "  3.14"
PRINT USING "$$#,###.##"; 1234.5     ' " $1,234.50"
PRINT USING "!"; "Hello"             ' "H"
```

### WRITE

Output values in comma-delimited format (strings are quoted):

```basic
WRITE 1, 2.5, "hello"    ' Output: 1,2.5,"hello"
```

### INPUT

```basic
INPUT x                        ' Prompt with "? "
INPUT "Enter value: ", x      ' Comma: prompt with no "? "
INPUT "Enter value: "; x      ' Semicolon: prompt with "? " appended
INPUT "Name, Age: "; n$, age  ' Multiple variables
```

### LINE INPUT

Read an entire line (no parsing of commas):

```basic
LINE INPUT "Enter text: "; text$
```

---

## Built-in Functions

### Math Functions

| Function  | Description                          | Example              |
|-----------|--------------------------------------|----------------------|
| `ABS(n)`  | Absolute value                       | `ABS(-5)` = 5       |
| `INT(n)`  | Floor (greatest integer <= n)        | `INT(3.7)` = 3      |
| `FIX(n)`  | Truncate toward zero                 | `FIX(-3.7)` = -3    |
| `SGN(n)`  | Sign: -1, 0, or 1                   | `SGN(-5)` = -1      |
| `SQR(n)`  | Square root                          | `SQR(9)` = 3        |
| `EXP(n)`  | e raised to the power n             | `EXP(1)` = 2.718... |
| `LOG(n)`  | Natural logarithm                    | `LOG(2.718)` = ~1   |
| `SIN(n)`  | Sine (radians)                       | `SIN(0)` = 0        |
| `COS(n)`  | Cosine (radians)                     | `COS(0)` = 1        |
| `TAN(n)`  | Tangent (radians)                    | `TAN(0)` = 0        |
| `ATN(n)`  | Arctangent (returns radians)         | `ATN(1)` = 0.785... |

### Random Numbers

```basic
RANDOMIZE seed     ' Seed the generator (use TIMER for non-deterministic)
RANDOMIZE          ' Seed with TIMER
x = RND            ' Next random number (0 to 1, exclusive)
x = RND(0)         ' Repeat last random number
x = RND(-1)        ' Reseed with negative value
```

### String Functions

| Function                       | Description                              | Example                         |
|--------------------------------|------------------------------------------|---------------------------------|
| `LEN(s$)`                     | Length of string                          | `LEN("Hi")` = 2               |
| `LEFT$(s$, n)`                | First n characters                       | `LEFT$("Hello", 3)` = "Hel"   |
| `RIGHT$(s$, n)`               | Last n characters                        | `RIGHT$("Hello", 3)` = "llo"  |
| `MID$(s$, start [, len])`     | Substring (1-based)                      | `MID$("Hello", 2, 3)` = "ell" |
| `INSTR([start,] s$, find$)`   | Find substring position (0 if not found) | `INSTR("Hello", "ll")` = 3    |
| `UCASE$(s$)`                  | Convert to uppercase                     | `UCASE$("hi")` = "HI"         |
| `LCASE$(s$)`                  | Convert to lowercase                     | `LCASE$("HI")` = "hi"         |
| `LTRIM$(s$)`                  | Remove leading spaces                    | `LTRIM$("  hi")` = "hi"       |
| `RTRIM$(s$)`                  | Remove trailing spaces                   | `RTRIM$("hi  ")` = "hi"       |
| `SPACE$(n)`                   | String of n spaces                       | `SPACE$(3)` = "   "            |
| `STRING$(n, ch$)`             | Repeat character n times                 | `STRING$(3, "*")` = "***"     |
| `STRING$(n, code)`            | Repeat ASCII character n times           | `STRING$(3, 42)` = "***"      |
| `CHR$(n)`                     | Character from ASCII code                | `CHR$(65)` = "A"              |
| `ASC(s$)`                     | ASCII code of first character            | `ASC("A")` = 65               |
| `STR$(n)`                     | Number to string (leading space if positive) | `STR$(42)` = " 42"        |
| `VAL(s$)`                     | Parse number from string                 | `VAL("42")` = 42              |
| `HEX$(n)`                     | Hexadecimal representation               | `HEX$(255)` = "FF"            |
| `OCT$(n)`                     | Octal representation                     | `OCT$(8)` = "10"              |

### String Statement Forms

```basic
MID$(s$, start, length) = replacement$   ' Replace substring in-place
LSET var$ = source$                       ' Left-justify into fixed-length variable
RSET var$ = source$                       ' Right-justify into fixed-length variable
```

### Conversion Functions

| Function   | Description                              |
|------------|------------------------------------------|
| `CINT(n)`  | Convert to INTEGER (rounds to nearest integer) |
| `CLNG(n)`  | Convert to LONG (rounds to nearest integer)    |
| `CSNG(n)`  | Convert to SINGLE                        |
| `CDBL(n)`  | Convert to DOUBLE                        |

### Binary Conversion Functions

For binary file I/O and data manipulation:

| Function   | Description                          |
|------------|--------------------------------------|
| `MKI$(n)`  | INTEGER to 2-byte binary string      |
| `MKL$(n)`  | LONG to 4-byte binary string         |
| `MKS$(n)`  | SINGLE to 4-byte binary string       |
| `MKD$(n)`  | DOUBLE to 8-byte binary string       |
| `CVI(s$)`  | 2-byte binary string to INTEGER      |
| `CVL(s$)`  | 4-byte binary string to LONG         |
| `CVS(s$)`  | 4-byte binary string to SINGLE       |
| `CVD(s$)`  | 8-byte binary string to DOUBLE       |

### System Functions

| Function       | Description                          | Example Return            |
|----------------|--------------------------------------|---------------------------|
| `TIMER`        | Seconds since midnight (SINGLE)      | `43261.5`                |
| `DATE$`        | Current date                         | `"03-08-2026"`           |
| `TIME$`        | Current time                         | `"14:30:45"`             |
| `ENVIRON$(v$)` | Environment variable value           | `ENVIRON$("HOME")`      |
| `FREEFILE`     | Next available file number           | `1`                      |
| `EOF(n)`       | End-of-file test (-1 if true)        | `EOF(1)`                 |
| `LOF(n)`       | File length in bytes                 | `LOF(1)`                 |
| `LOC(n)`       | Current file position                | `LOC(1)`                 |
| `SEEK(n)`      | File position (1-based; record number for RANDOM) | `SEEK(1)` |
| `CSRLIN`       | Current cursor row (1-based)         | `CSRLIN`                 |
| `POS(0)`       | Current cursor column (1-based)      | `POS(0)`                 |
| `INKEY$`       | Read key without waiting ("" if none)| `INKEY$`                 |
| `INPUT$(n)`    | Read n characters from keyboard      | `INPUT$(1)`              |
| `INPUT$(n, #f)`| Read n bytes from file               | `INPUT$(10, #1)`         |
| `SCREEN(r, c)` | ASCII code at screen position        | `SCREEN(1, 1)`           |
| `COMMAND$`     | Command-line arguments (stub)        | `""`                     |

---

## DATA, READ, and RESTORE

Store and retrieve inline data:

```basic
DATA 10, 20, 30, "Hello", "World"

READ a, b, c
READ d$, e$
PRINT a; b; c      ' 10 20 30
PRINT d$; " "; e$  ' Hello World

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
COLOR foreground, background  ' Set text colors (ANSI codes)
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
NAME "old.txt" AS "new.txt"    ' Rename file
KILL "temp.txt"                ' Delete file
```
