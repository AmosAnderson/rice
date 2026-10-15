# RICE BASIC syntax index

This index covers Rice BASIC and links to its authoritative [language reference](docs/language-reference.md). [Implementation limits](docs/compatibility.md) describe unsupported and partial features.

In the syntax forms below, `[ ... ]` means optional text, `{ ... }` means alternatives, and `...` means repetition. These markers are not literal BASIC syntax. `name` is a bare identifier, `expr` is an expression, `label` is a named label or literal line number, and `type` is a supported type name. Block bodies and terminators should be on separate physical lines.

## Running Rice

```sh
cargo build
cargo run -- program.bas
cargo run -- --check program.bas
cargo run
```

See [Runtime and tooling](docs/runtime.md) for installed-binary usage, REPL commands, and the LSP.

## Source structure and expressions

```basic
' Comment to end of line
REM Another comment
x = 1: y = 2: PRINT x + y
10 PRINT "numbered statement"
done:
PRINT "named label"
```

Keywords and identifiers are case-insensitive. Identifiers begin with an ASCII letter or `_`, followed by ASCII letters, digits, or `_`. Supported suffixes are `$`, `%`, `!`, `#`, and `&`. Suffixes distinguish names and enforce INTEGER (`%`), LONG (`&`), SINGLE (`!`), DOUBLE (`#`), and STRING (`$`), unless an explicit AS overrides them. Use spaces around concatenation `&`, especially after an unsuffixed name.

Numbers include `12`, `.5`, `1.5E-3`, and `1D3`. Rice also accepts unsigned hexadecimal `&HFF` and octal `&O77`, with optional trailing `&`. Decimal numeric suffixes are unsupported. Strings use double quotes; a doubled quote inside a literal represents one quote (`"say ""hi"""`). Backslashes remain literal. There is no line continuation or `?` PRINT abbreviation.

| Precedence, highest first | Operators |
|---|---|
| Primary | Literals, names, `name(args)`, `record.field`, `(expr)` |
| Power, right associative | `^` |
| Unary sign | `+`, `-` |
| Multiplicative | `*`, `/` |
| Integer division | `\` |
| Modulus | `MOD` |
| Additive/concatenation | `+`, `-`, `&` |
| Comparison | `=`, `<>`, `<`, `>`, `<=`, `>=`, `LIKE` |
| Negation | `NOT` |
| Conjunction | `AND` |
| Disjunction | `OR` |
| Exclusive OR | `XOR` |
| Equivalence | `EQV` |
| Implication | `IMP` |

Legacy numeric/string comparisons return `-1` for true and `0` for false. Comparisons involving native INT64/BYTE/BOOL/BYTES return BOOL. Numeric logical operators are 64-bit bitwise operations; native BOOL logic is logical. Both eagerly evaluate operands. EQV complements XOR; IMP combines the complement of the left operand with the right using OR, after integer conversion. With legacy numeric operands, `\` rounds operands to nearest-even signed 64-bit integers and truncates the quotient toward zero, with zero-divisor and overflow checks. MOD uses floor modulus for both exact integers and real numbers. Both `&` and `+` concatenate two strings or two BYTES buffers. See [Expressions and operators](docs/language-reference.md#expressions-and-operators).

`text$ LIKE pattern$` performs case-sensitive whole-string wildcard matching: `*` matches any sequence, `?` one Unicode scalar, `#` one ASCII digit, and `[abc]`, `[a-z]`, `[!abc]` match sets/ranges or their complements. Malformed classes raise an error. See [wildcard matching](docs/language-reference.md#wildcard-matching) for literal escapes and exact rules.

## Values, declarations, arrays, and assignment

```text
[LET] name = expr
[LET] name(index [, index ...]) = expr
DIM [SHARED] declaration [, declaration ...]
CONST name [AS type] = expr [, name [AS type] = expr ...]
OPTION BASE {0 | 1}
REDIM [PRESERVE] declaration [, declaration ...]
ERASE name [, name ...]
SWAP target, target
CLEAR
```

A `declaration` is `name [(bound [, bound ...])] [AS type]`. A `bound` is `upper` or `lower TO upper`, both inclusive. Types are `NUMERIC`, `INTEGER`, `LONG`, `SINGLE`, `DOUBLE`, `INT64`, `BOOL`/`BOOLEAN`, `BYTE`, `BYTES`, `STRING`, `STRING * integer-literal`, or a declared TYPE name. All declarations enforce conversion. INTEGER is i16, LONG i32, SINGLE f32, INT64 i64, BYTE u8, and NUMERIC/DOUBLE f64; STRING * n pads/truncates Unicode characters. See [Data types and declarations](docs/data-types.md).

```basic
OPTION BASE 0
DIM values(10), table(1 TO 3, 1 TO 4)
DIM message$ AS STRING
values(0) = 42
PRINT values(0)
PRINT LBOUND(table, 1); UBOUND(table, 2)
REDIM PRESERVE values(20)
```

Rice's initial OPTION BASE is **1**. Ordinary array access does not enforce declared bounds/rank. Arrays use flattened variable keys. Explicit array AS chooses defaults for untouched elements: a typed zero, false, empty buffer/string, or fresh record, overriding suffix/DEFSTR. Every declared element type validates assignments and DIM converts retained elements before replacing the declaration. Fixed strings initialize with spaces. Array metadata is interpreter-wide; REDIM/ERASE retain prefix-based removal and metadata limitations. SWAP accepts scalar names and array elements and resolves both locations once before exchanging their values. Members and slices are not SWAP targets. See [Arrays](docs/language-reference.md#arrays) and [Values, variables, and assignment](docs/language-reference.md#values-variables-and-assignment).

CONST lists execute left to right, so later initializers can use earlier names. Earlier definitions and expression side effects survive a later failure. An optional AS clause enforces each constant initializer's type; trailing commas are not accepted.

Declaration forms:

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
NEXT [name [, outer-name ...]]

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
EXIT {FOR | DO | WHILE | SELECT | SUB | FUNCTION}
{END | STOP | SYSTEM | QUIT}
```

A CASE `test` is `expr`, `lower TO upper`, or `IS comparison-operator expr`. CASE uses the first matching branch. FOR evaluates its limit/step once; STEP 0 skips its body. `NEXT j, i` closes nested FOR loops from innermost to outermost; names must match, and intervening blocks must already be closed. `EXIT WHILE` and `EXIT SELECT` leave the nearest active matching construct in the current procedure or top-level execution, passing through intervening blocks; unmatched exits raise an error. Both are Rice extensions. Loop-continue remains unsupported. Both WEND and END WHILE are accepted.

Legacy jumps:

```text
IF expr THEN line-number [ELSE line-number]
GOSUB label
RETURN [label]
ON expr {GOTO | GOSUB} label [, label ...]
```

Numeric targets at the start of a single-line THEN or ELSE branch abbreviate GOTO; either branch can instead contain ordinary statements. Named targets use explicit GOTO. `RETURN label` unwinds one GOSUB and then transfers to a named or numeric label using the caller's existing GOTO resolution; plain RETURN resumes after the call. Either form requires an active GOSUB in the current program/procedure. Computed jumps use 1-based, truncated integer selectors and fall through outside the target list. See [Control flow](docs/language-reference.md#control-flow) for label scope, nested-block jumps, loop variable behavior, and termination.

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
DECLARE FUNCTION name [(parameter [, parameter ...])] [AS type]
SHARED name[()] [AS type] [, name[()] [AS type] ...]
STATIC declaration [, declaration ...]
```

A `parameter` is `[BYVAL | BYREF] name[()] [AS type]`. parameters default to BYREF. BYREF copies modified values back to variables and individual array elements, capturing each destination during argument evaluation; indices run once, and later aliases win on writeback. Assign to the function name to provide its result. A FUNCTION return AS clause initializes a fresh value of its declared type at each call, overriding suffix defaults. All explicit parameter and return types enforce conversion; suffix shorthand also enforces its type. Unsuffixed parameters/results without AS remain dynamic. Array parameter annotations do not implement whole-array passing. Use explicit CALL syntax with parentheses for multiple arguments; `name (x)` makes its argument an expression and suppresses BYREF writeback. The [Procedures reference](docs/procedures.md) specifies available call forms, copy-in/copy-out behavior, scope, STATIC, declaration limits, and `DEF FN` syntax.

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

Substring assignment:

```text
MID$(target, start [, length]) = string-expression
MID$(names$(i), start [, length]) = string-expression
```

Slices use 1-based inclusive endpoints. Slice assignment replaces the selected region and can change the string length. MID$ assignment overwrites a scalar or array element without changing its length; indices resolve once before the other expressions. See [User-defined types](docs/user-defined-types.md) and [String slicing](docs/string-slicing.md).

## DATA and console I/O

```text
DATA [literal [, literal ...]]
READ target [, target ...]
RESTORE [label]

PRINT [print-items]
LPRINT [print-items]
PRINT USING format-expression; value [; value ...]
WRITE [expr [, expr ...]]
INPUT ["literal prompt" {; | ,}] target [, target ...]
LINE INPUT ["literal prompt" {; | ,}] target
```

PRINT items are expressions, `TAB(expr)`, or `SPC(expr)`, separated by `;` or `,`. A trailing separator suppresses the newline. Commas use 16-column zones; numbers have no automatic leading/trailing spaces. LPRINT writes to the normal output stream. INPUT prompt commas and semicolons both append `? ` in Rice.

An input `target` is a scalar `name` or an array element `name(expr [, expr ...])`, including string arrays such as `names$(i)`. Record members, slices, and empty subscripts are rejected. INPUT resolves all indices once before reading and keeps them across retries; READ resolves and assigns each target in order. Existing integer conversion, declaration, and array-storage rules apply. These destinations work and in the file forms below.

DATA contains literal numbers with optional leading `+`/`-`, strings, or single unquoted identifiers; unquoted text is uppercased. Empty comma-separated fields, including leading/trailing fields, supply the destination's typed zero, false, empty buffer, or empty string. Bare DATA has no items; `DATA ,` has two empty items. Empty fields cannot initialize a record; MAT READ treats them as zero. RESTORE labels must be attached directly to DATA; unknown labels silently rewind. See [DATA / READ / RESTORE](docs/language-reference.md#data--read--restore), [Input and output](docs/language-reference.md#input-and-output), and [PRINT USING](docs/print-using.md).

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
INPUT #number, target [, target ...]
LINE INPUT #number, target
GET #number [, [position] [, target]]
PUT #number [, [position] [, source]]
SEEK [#]number, position
SET #number: POINTER position
ASK #number: POINTER name
```

Both OPEN spellings are accepted; record positions depend on the configured record length. GET/PUT operands may be scalar names or individual array elements, including record arrays, for example `GET #1, , records(i)` and `PUT #1, , values%(i, j)`. Channel, position, and indices evaluate once before transfer. GET/PUT can omit their operand for FIELD buffers. Record field layouts include:

```text
FIELD [#]number, width AS name$ [, width AS name$ ...]
LSET name$ = expr
RSET name$ = expr
```

The [File I/O specification](docs/file-io.md) is authoritative for exact OPEN clause ordering, optional commas and targets, supported modes, pointer units, binary record types, FIELD, `MK*`/`CV*` functions, UTF-8/byte conversion, and error handling. Supported syntax and runtime effects are documented separately.

## SQLite databases

```text
db = DBOPEN(path$ [, mode$ [, timeoutMs]])
CALL DBEXEC(db, sql$ [, parameter, ...])
rows = DBQUERY(db, sql$ [, parameter, ...])
WHILE DBNEXT(rows)
    value = DBVALUE(rows, column)
WEND
CALL DBFREE(rows)
CALL DBBEGIN(db [, mode$])
CALL DBCOMMIT(db)
CALL DBROLLBACK(db)
CALL DBCLOSE(db)
```

Open modes are `rwc` (default create/read/write), `rw`, and `ro`; `:memory:`
creates a private in-memory database. Bind data with SQL `?` placeholders and
trailing values. Each call executes one statement. Queries materialize results;
columns use 1-based indices or unique names matched without ASCII case distinctions.
INTEGER results are INT64, REAL NUMERIC, TEXT STRING, BLOB BYTES, and SQL NULL is `NULL()`.
`ISNULL(value)` tests missing values; declared types reject NULL. Additional
functions provide column/row metadata, changes, last rowid, transaction state,
timeouts, and SQLite version. See the [SQLite guide](docs/sqlite.md) and complete
[builtin inventory](docs/builtins.md#sqlite-databases).

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

Classic forms:

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

`DET`/`DET()` returns the last inverse operation's determinant. `DET(matrix)` or `DET(matrix())` calculates a square matrix's determinant directly, returning zero for an exactly singular matrix and preserving the cached determinant. See [MAT operations](docs/mat-operations.md). MAT has a restricted expression grammar, not arbitrary nesting or general scalar-array arithmetic. `MAT PRINT #n, a` writes comma-separated numeric rows; `MAT INPUT #n, a` reads numeric fields in row-major order into an existing two-dimensional shape, storing the matrix only on success. Both evaluate the channel once and use the file instead of console streams. Console MAT PRINT retains space-separated rows. See the guide for errors, consumed input, and dimension/shape behavior.

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

System assignments:

```text
ENVIRON string-expression
[LET] DATE$ = string-expression
[LET] TIME$ = string-expression
```

See [System and filesystem statements](docs/language-reference.md#system-and-filesystem-statements) for host behavior and no-op cases. ENVIRON and date/time assignments set interpreter-local overrides.

The complete [Built-in function reference](docs/builtins.md) lists every implemented function, alias, argument count, type expectation, and known limitation. `MIN(number [, number ...])` and `MAX(number [, number ...])` select numeric extrema; each requires at least one numeric argument. Call spelling matters: use `RND()`, `PI()`, and `MAXNUM()` with parentheses; use bare `TIMER` and `FREEFILE`. An unknown `name(args)` may resolve as an array element instead of producing an unknown-function error. `VAL` accepts decimal prefixes with E/D exponents, and `ENVIRON$(index)` enumerates sorted environment entries (including interpreter overrides) from index 1; see the builtin reference for limits.

`LEN(string)` counts Unicode scalar values; `LEN(buffer)` and `BYTELEN(buffer)` count bytes. `LEN(record)` returns its current typed binary PUT byte count, including nested records and variable-string length headers; native scalar INT64/BOOL/BYTE layouts and raw BYTES transfers retain their native representations.

Rice also supports these features:

```text
REPLACE$(text$, find$, replacement$ [, start [, count]])
INSTRREV(text$, find$ [, start])
STRCOMP(left$, right$ [, compare])
ROUND(number [, places])
TRUNCATE(number [, places])
```

REPLACE$ returns the suffix beginning at start, with nonoverlapping matches replaced; it does not retain the preceding text. INSTRREV searches backward and returns a character position or zero. STRCOMP returns ordering -1/0/1; mode 0 (default) compares case-sensitively and mode 1 compares after Unicode lowercasing. String positions count Unicode scalar values. ROUND/TRUNCATE accept places from -308 through 308 after truncation; negative places select tens, hundreds, and higher powers of ten. The [builtin reference](docs/builtins.md) specifies defaults, empty-string behavior, validation, and overflow rules.

`COMMAND$()` (alias `COMMAND()`) returns program arguments joined with spaces. Run `rice program.bas -- argument ...` to pass them; `--` protects dash-leading arguments from Rice option parsing. New library and REPL interpreters default to empty arguments. See [runtime](docs/runtime.md) for the complete CLI and library contract.
