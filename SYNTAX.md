# RICE BASIC syntax index

This index covers the implemented statement families and links to their specifications. Read the [language reference](docs/language-reference.md) for semantics, the [dialect table](docs/dialects.md) for mode differences, and [compatibility limits](docs/compatibility.md) for deviations and unknowns. Rice defaults to QBasic 1.1 compatibility mode; its ANSI mode is selected explicitly. Neither mode is a claim of complete historical conformance.

In the syntax forms below, `[ ... ]` means optional text, `{ ... }` means alternatives, and `...` means repetition. These markers are not literal BASIC syntax. `name` is a bare identifier, `expr` is an expression, `label` is a named label or literal line number, and `type` is a supported type name. Block bodies and terminators should be on separate physical lines. Forms marked **QB only** are rejected in ANSI mode; other forms are available in both modes, subject to the linked limits.

## Running and selecting a dialect

```sh
cargo build
cargo run -- program.bas
cargo run -- --dialect ansi program.bas
cargo run -- --dialect qb program.bas
cargo run -- --compat program.bas
cargo run
```

```basic
OPTION DIALECT "ANSI"
```

Source selectors `QB`, `QBASIC`, `QBASIC 1.1`, `QBASIC1.1`, and `QUICKBASIC` select QB, case-insensitively. The first recognized source directive selects the mode for the whole source and overrides the CLI selection. See [Runtime and tooling](docs/runtime.md) for installed-binary usage, REPL commands, and the LSP.

## Source structure and expressions

```basic
' Comment to end of line
REM Another comment
x = 1: y = 2: PRINT x + y
10 PRINT "numbered statement"
done:
PRINT "named label"
```

Keywords and identifiers are case-insensitive. Identifiers begin with an ASCII letter or `_`, followed by ASCII letters, digits, or `_`. `$` is accepted in both modes; QB additionally accepts `%`, `!`, `#`, and `&`. Suffixes distinguish names but do not enforce primitive assignment types. Use spaces around concatenation `&`, especially after an unsuffixed name in QB.

Numbers include `12`, `.5`, `1.5E-3`, and `1D3`. QB additionally accepts unsigned hexadecimal `&HFF` and octal `&O77`, with optional trailing `&`. Decimal numeric suffixes are unsupported. Strings use double quotes with no quote/backslash escape syntax; use `CHR$(34)` to include a double quote. There is no line continuation or `?` PRINT abbreviation.

| Precedence, highest first | Operators |
|---|---|
| Primary | Literals, names, `name(args)`, `record.field`, `(expr)` |
| Power, right associative | `^` |
| Unary sign | `+`, `-` |
| Multiplicative | `*`, `/` |
| Modulus | `MOD` |
| Additive/concatenation | `+`, `-`, `&` |
| Comparison | `=`, `<>`, `<`, `>`, `<=`, `>=` |
| Negation | `NOT` |
| Conjunction | `AND` |
| Disjunction | `OR` |
| Exclusive OR | `XOR` |

QB comparisons return `-1`/`0`; ANSI comparisons return `1`/`0`. QB logical operators are 64-bit bitwise operations; ANSI uses logical nonzero/zero tests. Both eagerly evaluate operands. MOD uses real-number floor modulus in both modes. `&` concatenates strings in both modes; string `+` is QB-only. There is no integer division `\`, `IMP`, or `EQV`. See [Expressions and operators](docs/language-reference.md#expressions-and-operators).

## Values, declarations, arrays, and assignment

```text
[LET] name = expr
[LET] name(index [, index ...]) = expr
DIM [SHARED] declaration [, declaration ...]
CONST name = expr
OPTION BASE {0 | 1}
REDIM [PRESERVE] declaration [, declaration ...]
ERASE name [, name ...]
SWAP name, name
CLEAR
```

A `declaration` is `name [(bound [, bound ...])] [AS type]`. A `bound` is `upper` or `lower TO upper`, both inclusive. Types are `NUMERIC`, `INTEGER`, `LONG`, `SINGLE`, `DOUBLE`, `STRING`, `STRING * integer-literal`, or a declared TYPE name. Every number is stored as `f64` regardless of its type spelling.

```basic
OPTION BASE 0
DIM values(10), table(1 TO 3, 1 TO 4)
DIM message$ AS STRING
values(0) = 42
PRINT values(0)
PRINT LBOUND(table, 1); UBOUND(table, 2)
REDIM PRESERVE values(20)
```

Rice's initial OPTION BASE is **1 in both modes**. Ordinary array access does not enforce declared bounds/rank. Arrays use flattened variable keys; AS STRING does not determine untouched primitive element defaults, so use `$` names for string arrays. REDIM/ERASE have prefix-based removal and metadata limitations. SWAP accepts bare names only. See [Arrays](docs/language-reference.md#arrays) and [Values, variables, and assignment](docs/language-reference.md#values-variables-and-assignment).

QB-only declaration forms:

```text
OPTION EXPLICIT
{DEFINT | DEFLNG | DEFSNG | DEFDBL | DEFSTR} letter[-letter] [, letter[-letter] ...]
COMMON [SHARED] name[()] [AS type] [, name[()] [AS type] ...]
```

OPTION EXPLICIT becomes active when executed and permits declaration by suffix, DEFtype, loop variable, or declaration statement; it is not static type checking.

## Control flow

```text
IF expr THEN statement [: statement ...] [ELSE statement [: statement ...]]

IF expr THEN
    statements
[ELSEIF expr THEN
    statements ...]
[ELSE
    statements]
END IF

FOR name = start TO end [STEP expr]
    statements
NEXT [name]

WHILE expr
    statements
{WEND | END WHILE}

DO [{WHILE | UNTIL} expr]
    statements
LOOP

DO
    statements
LOOP [{WHILE | UNTIL} expr]

SELECT CASE expr
CASE test [, test ...]
    statements
[CASE test [, test ...]
    statements ...]
[CASE ELSE
    statements]
END SELECT

GOTO label
EXIT {FOR | DO | SUB | FUNCTION}
{END | STOP | SYSTEM | QUIT}
```

A CASE `test` is `expr`, `lower TO upper`, or `IS comparison-operator expr`. CASE uses the first matching branch. FOR evaluates its limit/step once; STEP 0 skips its body. `NEXT j, i`, `EXIT WHILE`, `EXIT SELECT`, `IF expr THEN line-number`, and loop-continue are unsupported. Both WEND and END WHILE are accepted in both modes.

QB-only legacy jumps:

```text
GOSUB label
RETURN
ON expr {GOTO | GOSUB} label [, label ...]
```

RETURN takes no target. Computed jumps use 1-based, truncated integer selectors and fall through outside the target list. See [Control flow](docs/language-reference.md#control-flow) for label scope, nested-block jumps, loop variable behavior, and termination.

## Procedures and scope

```text
SUB name [(parameter [, parameter ...])] [STATIC]
    statements
END SUB

FUNCTION name [(parameter [, parameter ...])] [AS type] [STATIC]
    statements
END FUNCTION

CALL name [(expr [, expr ...])]
name [expr [, expr ...]]
DECLARE SUB name [(parameter [, parameter ...])]
DECLARE FUNCTION name [(parameter [, parameter ...])]
SHARED name [, name ...]
STATIC declaration [, declaration ...]
```

A `parameter` is `[BYVAL | BYREF] name[()] [AS type]`. QB parameters default to BYREF; ANSI defaults to BYVAL. Assign to the function name to provide its result. Array parameter annotations do not implement whole-array passing, and primitive return/parameter type clauses do not guarantee coercion. Use explicit CALL syntax with parentheses for multiple arguments; QB `name (x)` makes its argument an expression and suppresses BYREF writeback. The [Procedures reference](docs/procedures.md) specifies available call forms, copy-in/copy-out behavior, scope, STATIC, declaration limits, and QB `DEF FN` syntax.

## Records and strings

```text
TYPE name
    field AS type
    ...
END TYPE
DIM name AS type-name
[LET] record.field[.field ...] = expr
[LET] records(index [, index ...]).field[.field ...] = expr

name$(start:end)
[LET] name$(start:end) = string-expression
```

QB substring assignment:

```text
MID$(name$, start [, length]) = string-expression
```

Slices use 1-based inclusive endpoints. Slice assignment replaces the selected region and can change the string length; MID$ assignment has different overwrite semantics. See [User-defined types](docs/user-defined-types.md) and [String slicing](docs/string-slicing.md).

## DATA and console I/O

```text
DATA [literal [, literal ...]]
READ name [, name ...]
RESTORE [label]

PRINT [print-items]
LPRINT [print-items]
PRINT USING format-expression; value [; value ...]
WRITE [expr [, expr ...]]
INPUT ["literal prompt" {; | ,}] name [, name ...]
LINE INPUT ["literal prompt" {; | ,}] name
```

PRINT items are expressions, `TAB(expr)`, or `SPC(expr)`, separated by `;` or `,`. A trailing separator suppresses the newline. Commas use 16-column zones; numbers have no automatic leading/trailing spaces in either mode. LPRINT writes to the normal output stream. INPUT prompt commas and semicolons both append `? ` in Rice. READ/INPUT/LINE INPUT targets are bare names, not array elements.

DATA contains literal numbers, negative numbers, strings, or single unquoted identifiers; unquoted text is uppercased. RESTORE labels must be attached directly to DATA; unknown labels silently rewind. See [DATA / READ / RESTORE](docs/language-reference.md#data--read--restore), [Input and output](docs/language-reference.md#input-and-output), and [PRINT USING](docs/print-using.md).

Text-console statements:

```text
CLS
BEEP
LOCATE [row] [, column]
COLOR [foreground] [, background]
WIDTH [columns] [, rows]
VIEW PRINT [top TO bottom]
```

[Console](docs/console.md) defines omitted arguments, validation, terminal side effects, and `CSRLIN`, `POS`, `SCREEN`, `INKEY$`, and `INPUT$` behavior. There are no graphics SCREEN modes or graphics drawing statements.

## File I/O

```text
OPEN filename FOR mode [ACCESS access] [{SHARED | LOCK lock-access}] AS [#]number [LEN = length]
OPEN #number: NAME filename [, ORGANIZATION organization] [, ACCESS access]
CLOSE [[#]number [, [#]number ...]]
RESET
PRINT #number, [print-items]
PRINT #number, USING format-expression; value [; value ...]
WRITE #number, expr [, expr ...]
INPUT #number, name [, name ...]
LINE INPUT #number, name
GET #number [, [position] [, name]]
PUT #number [, [position] [, name]]
SEEK [#]number, position
SET #number: POINTER position
ASK #number: POINTER name
```

Both OPEN spellings are accepted in both modes; pointer/record interpretation can depend on the dialect. GET/PUT can omit their target for QB FIELD buffers. QB-only record field declarations/alignment include:

```text
FIELD [#]number, width AS name$ [, width AS name$ ...]
LSET name$ = expr
RSET name$ = expr
```

The [File I/O specification](docs/file-io.md) is authoritative for exact OPEN clause ordering, optional commas and targets, supported modes, pointer units, binary record types, FIELD, `MK*`/`CV*` functions, UTF-8/byte conversion, and error handling. Do not infer that all syntactic modes implement complete QBasic file semantics.

## Exception handling

```text
WHEN EXCEPTION IN
    statements
USE
    handler-statements
END WHEN
RETRY
CONTINUE
```

`IN` is required. EXTYPE and EXTEXT$ expose structured exception information. RETRY repeats the protected block, and CONTINUE exits the handler and continues with the protected statement after the failed direct statement; neither is a generic loop control.

QB-only classic forms:

```text
ON ERROR GOTO label
ON ERROR GOTO 0
ERROR expr
RESUME
RESUME NEXT
RESUME label
```

ERR and ERL expose the classic error code/numbered line. See [Error handling](docs/error-handling.md) for handler state, resume granularity, and the full code mapping.

## Matrix statements

```text
MAT PRINT [#number,] name
MAT INPUT [#number,] name
MAT READ name
MAT target = source
MAT target = {ZER | CON | IDN}
MAT target = {INV | TRN}(source)
MAT target = source {+ | - | *} source
MAT target = (scalar-expression) * source
```

DET returns the last inverse operation's determinant; use it as a bare special value as specified in [MAT operations](docs/mat-operations.md). MAT has a restricted expression grammar, not arbitrary nesting or general scalar-array arithmetic. Channel support and dimension/shape behavior are also specified there.

## System statements and builtins

```text
RANDOMIZE [expr]
SLEEP [expr]
NAME old-path AS new-path
KILL path
MKDIR path
RMDIR path
CHDIR path
CHDRIVE drive
FILES [path]
SHELL [command]
```

QB-only system assignments:

```text
ENVIRON string-expression
[LET] DATE$ = string-expression
[LET] TIME$ = string-expression
```

See [System and filesystem statements](docs/language-reference.md#system-and-filesystem-statements) for host behavior and no-op cases. ENVIRON and date/time assignments set interpreter-local overrides.

The complete [Built-in function reference](docs/builtins.md) lists every implemented function, alias, argument count, type expectation, and known limitation. Call spelling matters: use `RND()`, `PI()`, and `MAXNUM()` with parentheses; use bare `TIMER` and `FREEFILE`. An unknown `name(args)` may resolve as an array element instead of producing an unknown-function error.
