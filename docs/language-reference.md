# RICE BASIC language reference

This reference specifies the language implemented by this repository. **QB** means Rice's default QBasic 1.1 compatibility mode; **ANSI** means Rice's ANSI-oriented mode. These are compatibility targets, not claims that every feature or edge case of either historical standard is implemented. Unless a section says otherwise, its syntax is accepted in both modes.

The specification is split across this core reference and the following detailed references:

| Subject | Reference |
|---|---|
| Dialect selection and feature differences | [Dialects](dialects.md) |
| Every builtin and its calling convention | [Built-in functions](builtins.md) |
| SUB, FUNCTION, DEF FN, parameters, scope, STATIC | [Procedures](procedures.md) |
| TYPE records and field access | [User-defined types](user-defined-types.md) |
| String slices and substring assignment | [String slicing](string-slicing.md) |
| Matrix statements | [MAT operations](mat-operations.md) |
| Files, channels, records, FIELD, packed values | [File I/O](file-io.md) |
| Formatted printing | [PRINT USING](print-using.md) |
| Keyboard and text screen | [Console](console.md) |
| Structured and classic error handling | [Error handling](error-handling.md) |
| CLI, REPL, and language server | [Runtime and tooling](runtime.md) |
| Unsupported features and unresolved compatibility | [Compatibility limits](compatibility.md) |

[SYNTAX.md](../SYNTAX.md) provides a compact syntax index. [Multiple modules](multi-module.md) explains the single-source execution model.

## Dialects and execution

```sh
rice program.bas
rice --dialect ansi program.bas
rice --dialect qb program.bas
rice --compat program.bas
```

A source directive selects the dialect before lexing and parsing the complete source:

```basic
OPTION DIALECT "ANSI"
```

Accepted source values are `ANSI`, `QB`, `QBASIC`, `QBASIC 1.1`, `QBASIC1.1`, and `QUICKBASIC`, case-insensitively. The first recognized source directive wins and overrides the CLI selection. It need not be the first line or an executed statement. An unknown string is currently accepted as a no-op; it does not report an unsupported dialect. Put one valid directive at the top of a program. CLI aliases and REPL persistence are specified in [Runtime and tooling](runtime.md).

| Behavior | QB | ANSI |
|---|---|---|
| Comparison true / false | `-1` / `0` | `1` / `0` |
| `AND`, `OR`, `XOR`, `NOT` | Signed 64-bit bitwise operations | Logical operations on nonzero/zero |
| String concatenation | `+` or `&` | `&` |
| Identifier suffixes | `$`, `%`, `!`, `#`, `&` | `$` |
| Hexadecimal/octal literals | `&H...`, `&O...` | Not supported |
| Default parameter passing | BYREF | BYVAL |
| Initial default array lower bound in Rice | **1** | **1** |
| GOSUB, RETURN, ON jumps, ON ERROR, RESUME | Supported with documented limits | Rejected |
| DEFtype, DEF FN, OPTION EXPLICIT, COMMON | Supported | Rejected |

The base-1 default in QB is a Rice incompatibility with QBasic. Use `OPTION BASE 0` or explicit bounds when porting a program that expects zero-based arrays. `INTEGER`, `LONG`, `SINGLE`, and `DOUBLE` declarations are accepted in both modes and do not impose those numeric widths in memory.

## Source text and lexical rules

Source files are UTF-8. Spaces and tabs separate tokens; LF, CRLF, and CR terminate lines. Keywords, labels, and identifiers are case-insensitive and identifiers are normalized to uppercase. String contents retain their case.

Identifiers match an ASCII letter or `_`, followed by ASCII letters, digits, or `_`, and optionally one dialect-supported suffix. Examples: `total`, `_count`, `item2`, `name$`, and, in QB, `count%`. A suffix is part of the name: `X`, `X%`, and `X#` are different variables. Dots are record member separators, not identifier characters. Unsuffixed keywords cannot be used as ordinary variable names; most builtin names are ordinary identifiers with special expression-call resolution. A suffix bypasses keyword recognition (`STRING$` and `NAME$` are identifiers).

In QB, leave whitespace before a concatenation `&` following an unsuffixed identifier: `a & b`. `a&` is one suffixed identifier. Use the same spacing habit when mixing numeric literals with ampersands.

### Literals

| Form | Examples | Meaning |
|---|---|---|
| Decimal | `0`, `12`, `12.`, `.5`, `3.25` | An `f64` number |
| Exponent | `1E3`, `2.5e-2`, `1D+3` | Decimal exponent; `D` and `E` are equivalent |
| QB hexadecimal | `&HFF`, `&HFFFF&` | Unsigned radix-16 integer converted to `f64` |
| QB octal | `&O77`, `&O77&` | Unsigned radix-8 integer converted to `f64` |
| String | `"Hello"`, `""`, `"é"` | Text between double quotes, confined to one physical line |

Signs are unary operators, not part of a number token. Decimal numeric-literal suffixes such as `1%` or `1#`, binary literals, digit separators, and hexadecimal signed-width reinterpretation are not implemented. Radix literals must fit `u64`; large integers can lose precision when stored as `f64`.

String literals have **no escape syntax**: neither `\n` nor doubled double quotes represent escaped characters. To include a quote, use `CHR$(34)`, for example `"She said " & CHR$(34) & "hi" & CHR$(34)`. `PRINT "a""b"` happens to print `ab` because PRINT accepts adjacent expressions; it does not demonstrate quote escaping.

### Lines, comments, labels, and separators

```basic
' Apostrophe comments run to the end of the physical line.
REM So do REM comments.
x = 1: y = 2: PRINT x + y
10 PRINT "A numbered statement"
again:
PRINT "A named label"
```

An unsigned decimal integer at the start of a physical line, after indentation, is a line-number label in the range `0` through `4294967295`. It is not an expression statement. A name followed by `:` in statement position is a named label. A label may occupy a line by itself. Source files execute in textual order; line numbers do not sort a file. The REPL's stored program is separately ordered by line number.

A colon separates statements. Block constructs should put their headers, bodies, and terminators on separate lines. There is no underscore line-continuation syntax, multiline string literal, or `?` shorthand for PRINT. Comments consume the remainder of their line, including any colons. `!` is not a comment marker in either mode; in QB it can be an identifier suffix, and a standalone `!` is a lexical error. Compound keywords use spaces/tabs on the same line: `END IF`, `END SUB`, `END FUNCTION`, `END SELECT`, `END TYPE`, `END DEF`, `END WHILE`, `END WHEN`, and `LINE INPUT`. Use `ELSEIF` as one word.

## Values, variables, and assignment

There are two primitive values: NUMERIC (`f64`) and STRING (UTF-8 text). TYPE declarations additionally create structured record values; see [User-defined types](user-defined-types.md). Numeric calculations use floating-point precision even for integral values. Arithmetic can produce infinity or NaN; Rice does not universally convert floating-point overflow/domain failures into BASIC errors.

A missing scalar is initialized on first read: `""` for a `$` name and `0` for other names, subject to QB `DEFSTR` rules and `OPTION EXPLICIT`. Array defaults are discussed below. Conditions accept numeric values only: zero is false; every nonzero value is true. Strings and records used as conditions cause a type mismatch.

### Declarations

```basic
DIM count AS NUMERIC
DIM title AS STRING
DIM name$                        ' String default from suffix
DIM samples(1 TO 10) AS DOUBLE
DIM code AS STRING * 8
DIM SHARED total AS NUMERIC
CONST limit = 10
CONST greeting$ = "Hello"
```

`DIM [SHARED] declaration [, declaration ...]` accepts scalar names or dimensions, each with an optional `AS type`. Types are `NUMERIC`, `INTEGER`, `LONG`, `SINGLE`, `DOUBLE`, `STRING`, `STRING * length`, or a declared TYPE name. `length` is a nonnegative integer numeric literal, not a variable or expression. Numeric types all initialize to `0`; both primitive string forms initialize to `""`. A scalar record initializes its fields recursively.

Declarations execute when reached. Repeating `DIM` resets the scalar value but does not clear existing array elements. A later `DIM` can replace bounds metadata. `AS` overrides the suffix for the initial scalar default. Fixed-length scalar strings are not padded/truncated by ordinary assignment; fixed widths apply when serializing declared record fields to binary files, not during ordinary field assignment. See the record and file references.

`CONST name = expression` evaluates the expression at execution time and creates one constant in the current scope. It has no `AS` clause or comma-separated declaration list. Duplicate definition in the same scope raises an error. Normal assignments and other checked write targets cannot change a constant; constants remain after `CLEAR`. Some declaration/reset paths do not use the same write checks, so do not treat CONST as comprehensive static enforcement.

### Assignment and type enforcement

```basic
LET count = 5
count = count + 1
name$ = "Rice"
samples(2) = 3.5
```

`LET` is optional. Statement-level `=` assigns; `=` inside an expression compares. Array and record member assignments, slices, and QB `MID$` assignment have separate forms in this reference and the linked topic guides. Assignments are not expressions and cannot be chained: `a = b = c` assigns the result of the comparison `b = c` to `a`.

**Current limitation:** ordinary scalar and array assignment stores the supplied value without enforcing the name suffix or declared primitive type. Thus `n$ = 7` and `DIM n AS NUMERIC` followed by `n = "text"` currently succeed. `READ`, `SWAP`, and `LINE INPUT` also have their own permissive behavior. Operations subsequently check the actual value type: arithmetic does not parse numeric strings, and string concatenation does not automatically format numbers. Explicit conversion functions are listed in [Built-in functions](builtins.md). Record field assignments and binary serialization have their own validation/coercion rules.

### QB default types

```basic
DEFSTR A-C, N
DEFINT I-J
DEFLNG K
DEFSNG P
DEFDBL X-Z
```

These statements are QB-only. Each range contains single ASCII letters, inclusively. `DEFSTR` makes subsequently auto-initialized unsuffixed names beginning with those letters strings. Numeric DEFtype forms reset the corresponding default to numeric; they do not implement distinct integer or floating-point storage. Existing values are unchanged. `DIM` without `AS` uses its own suffix/default logic and does not inherit `DEFSTR`. Reversed letter ranges currently have no effect.

### OPTION EXPLICIT (QB only)

```basic
OPTION EXPLICIT
DIM x
x = 10
```

When execution reaches this statement it enables declaration checks for subsequent operations throughout the interpreter. There is no `OFF` form. It is not a static check or a directive retroactively applied to preceding statements. Reads of existing bindings can succeed even if those bindings were not explicitly declared before the option was enabled.

Names count as declared through executed `DIM`, `REDIM`, `SHARED`, `STATIC`, `COMMON`, or `CONST`; through a procedure parameter or result binding; through a `FOR` variable; through any supported type suffix; or through an executed DEFtype letter range. Suffixes therefore bypass the need for DIM, including on an array name. Recognized special values and zero-argument user functions can still be read. Some specialized statements have weaker checks; this option is not a full declaration/type analysis. Scope details are in [Procedures](procedures.md).

### SWAP, CLEAR, and scope declarations

`SWAP a, b` exchanges two **bare variable names**. Array elements and record members are not accepted as targets. Values need not have matching types. Missing operands default to numeric `0` even if their names end in `$`; initialize strings before swapping them.

`CLEAR` removes variable values in the current environment and resets the DATA cursor to the start. It retains constants, declaration flags, procedure/type definitions, options, and array bounds metadata. It does not close files or fully recreate the interpreter. Compatibility arguments after CLEAR are parsed and ignored.

`SHARED name [, name ...]`, `DIM SHARED ...`, `STATIC ...`, and QB-only `COMMON [SHARED] ...` control procedure state. COMMON does not load or link modules; `CHAIN` is unsupported. See [Procedures](procedures.md) and [Multiple modules](multi-module.md) for exact sharing and persistence limitations.

## Expressions and operators

Parentheses group expressions. Named calls use `name(argument [, argument ...])`. Because the same syntax is used for arrays, a parenthesized name is resolved by the interpreter: special stateful function, registered builtin, user FUNCTION, then array element. Consequently an unknown function name can silently behave as an array access and return a default value. Existing builtin names cannot reliably be used for arrays or user functions.

Only specifically supported functions may be written without parentheses. In particular use `RND()`, `PI()`, and `MAXNUM()`; bare `RND`, `PI`, and `MAXNUM` are ordinary variables. Use bare `TIMER` and `FREEFILE`; the parser does not consume an empty parenthesis pair for them. See [Built-in functions](builtins.md) for all exact call forms, including aliases and special values.

Operands and ordinary argument lists are evaluated left to right. `AND`, `OR`, and `XOR` **do not short-circuit**, in either mode; `0 AND (1 / 0)` raises an error.

| Precedence, highest first | Operators | Association |
|---|---|---|
| Primary | Literals, references, calls, member access, parentheses | — |
| Power | `^` | Right |
| Unary sign | `+`, `-` | Prefix |
| Multiply/divide | `*`, `/` | Left |
| Modulus | `MOD` | Left |
| Add/subtract/concatenate | `+`, `-`, `&` | Left |
| Comparison | `=`, `<>`, `<`, `>`, `<=`, `>=` | Left |
| Logical/bitwise negation | `NOT` | Prefix |
| Conjunction | `AND` | Left |
| Disjunction | `OR` | Left |
| Exclusive OR | `XOR` | Left |

For example `-2^2 = -4`, `2^-2 = 0.25`, and `2^3^2 = 512`. Chained comparisons are evaluated as successive binary operations: `2 < 1 < 1` means `(2 < 1) < 1`, which is true. Use `(a < b) AND (b < c)` for a range test.

Arithmetic operators require numbers. `/` is real division; division by zero raises an error. `MOD` uses **`a - b * FLOOR(a / b)` in both modes**, including real operands. Examples: `-5 MOD 3 = 1`, `5 MOD -3 = -1`, and `5.5 MOD 2 = 1.5`. A zero divisor raises an error. Integer division `\`, `IMP`, and `EQV` are not implemented.

Comparison works on two numbers or two strings and returns the dialect's numeric true/false values. Mixed string/number expression comparison raises a type mismatch. Strings compare case-sensitively in lexicographic Unicode order; there is no locale collation or case-folding option. Ordinary operators do not compare records.

In QB, `AND`, `OR`, `XOR`, and `NOT` cast numbers to signed 64-bit integers, truncating fractions toward zero, perform the bitwise operation, and return an `f64`. These casts saturate out-of-range values and map NaN to zero; they do not implement QBasic 16-bit integer semantics or consistent overflow checking. ANSI logical operators treat every nonzero number as true and return `0` or `1`.

`&` requires two strings in both modes. QB `+` also concatenates two strings, but mixed strings/numbers are rejected. ANSI `+` accepts numbers only.

## Arrays

```basic
OPTION BASE 0
DIM a(10)                       ' Bounds 0 through 10, inclusive
DIM b(1 TO 10)                  ' Explicit bounds ignore OPTION BASE
DIM grid(-2 TO 2, 1 TO 3)
a(0) = 42
PRINT a(0)
PRINT LBOUND(grid, 1); UBOUND(grid, 2)
REDIM PRESERVE a(20)
ERASE a, grid
```

Dimensions are comma-separated `upper` or `lower TO upper` expressions. Their bounds are evaluated when DIM/REDIM executes and converted to signed 64-bit integers by truncation toward zero; nonfinite/unrepresentable numbers are rejected. Bounds are inclusive. A reversed range or overflowing range size is rejected. `OPTION BASE 0` or `OPTION BASE 1` changes the implicit lower bound for subsequent dimensions in that environment. The initial value is **1 in both dialects**; changing it does not change already recorded bounds.

`LBOUND(array [, dimension])` and `UBOUND(array [, dimension])` read dimension metadata. Dimensions are numbered from 1, with 1 the default. Missing array metadata or an invalid dimension raises an error. Use the bare array name as the first argument.

Rice arrays are currently sparse values stored under flattened names, separate from their bounds metadata:

- Ordinary element access does **not** enforce DIM bounds, the number of dimensions, or a requirement to DIM first when declaration checks permit the name.
- Indices truncate toward zero using checked signed 64-bit conversion. `a(1.9)` accesses `a(1)`.
- `a(1, 2)` is stored under the key `A_1_2`. This can collide with a scalar named `a_1_2` or an element `a_1(2)`. Avoid such scalar/array naming combinations.
- Scalar `a` and its elements coexist. A bare array name in an ordinary expression is the scalar slot, not a whole-array value.
- Missing primitive elements use the name/DEFSTR default, not the DIM `AS` clause. For example, an unsuffixed `DIM a(2) AS STRING` still yields numeric `0` for untouched `a(1)`; use `a$` for string arrays. UDT arrays separately retain record type metadata and initialize missing records lazily.
- `DIM` updates metadata and a scalar default but does not erase existing elements. Array assignment evaluates the right-hand value before its index expressions.

`REDIM [PRESERVE]` accepts the same declarations as DIM. Without PRESERVE, it removes existing keys beginning with the array name plus `_`; with PRESERVE, it keeps **all** such values, including values outside new bounds. It permits changes to any dimension instead of enforcing QBasic's last-dimension-only restriction. It resets the scalar slot even with PRESERVE. A failed REDIM can already have removed data. Type-changing REDIM of a UDT array is not a reliable conversion and may retain old record metadata.

`ERASE name [, name ...]` removes keys with that prefix and sets the scalar slot to numeric `0`. It retains dimension/type metadata and declaration state. Because removal uses a name prefix, ERASE and non-PRESERVE REDIM can also remove colliding scalar names or another array with that prefix. They do not implement a distinction between static and dynamic arrays. Array parameter/sharing limitations are in [Procedures](procedures.md); whole-matrix operations are in [MAT operations](mat-operations.md).

## Control flow

### IF

```basic
IF x > 0 THEN PRINT "positive" ELSE PRINT "zero or negative"

IF x > 0 THEN
    PRINT "positive"
ELSEIF x < 0 THEN
    PRINT "negative"
ELSE
    PRINT "zero"
END IF
```

THEN followed by a newline/comment starts a block IF; statements following THEN on the same line form a single-line IF. Single-line branches can contain colon-separated statements. ELSEIF clauses belong to the block form. Conditions are evaluated in order until one is true; only that branch executes. Write an explicit `GOTO` for a jump: the historical shorthand `IF condition THEN 100` is not implemented.

### FOR / NEXT

```basic
FOR i = 1 TO 3
    PRINT i
NEXT i

FOR i = 3 TO 1 STEP -1
    IF i = 2 THEN EXIT FOR
NEXT
```

Start, end, and step expressions are evaluated once on entry, in that order. STEP defaults to `1`. End bounds are inclusive. The loop variable is assigned even if the body runs zero times; it is also considered declared for OPTION EXPLICIT. Positive steps stop above the end and negative steps stop below it. **STEP 0 performs zero iterations**. Real steps are supported and subject to floating-point rounding. The current variable value is incremented after the body, so body assignments to it affect subsequent iterations. Normal completion leaves it at the first value beyond the limit; EXIT FOR leaves its current value.

NEXT may omit the variable or name the matching variable. A mismatched name is a parse error. `NEXT j, i` is not supported. EXIT FOR exits the enclosing active FOR; it is not a generic loop exit.

### WHILE and DO

```basic
WHILE x < 10
    x = x + 1
END WHILE

DO WHILE x < 20
    x = x + 1
LOOP

DO
    x = x - 1
LOOP UNTIL x = 0
```

WHILE checks before each iteration and accepts either `WEND` or `END WHILE` in both modes. There is no EXIT WHILE.

DO supports `DO WHILE condition ... LOOP`, `DO UNTIL condition ... LOOP`, `DO ... LOOP WHILE condition`, `DO ... LOOP UNTIL condition`, and unconditional `DO ... LOOP`. A bottom-tested loop executes at least once; a top-tested loop can execute zero times. Use only one test, at either the top or bottom. `EXIT DO` exits the enclosing active DO loop. `CONTINUE` is an exception-handler operation, not a loop-continue statement.

### SELECT CASE

```basic
SELECT CASE score
CASE 0
    PRINT "zero"
CASE 1 TO 9, 20
    PRINT "small or twenty"
CASE IS >= 90
    PRINT "high"
CASE ELSE
    PRINT "other"
END SELECT
```

The selector is evaluated once. Cases are tested in source order, with a comma-separated test list acting as alternatives. Ranges are inclusive. `CASE IS` accepts any comparison operator. The first matching branch executes, with no fallthrough. CASE ELSE must be last and is optional. String values and string ranges are supported; `EXIT SELECT` is not.

CASE comparisons use value equality/ordering directly, unlike expression comparisons: mixed types generally fail to match rather than raising a type mismatch, and `CASE IS <>` can match unequal mixed types. CASE equality can compare records structurally; record ordering is unavailable. Do not depend on these differences when porting a program.

### GOTO, GOSUB, and computed jumps

```basic
GOTO done
PRINT "skipped"
done:
PRINT "done"
```

GOTO accepts a named label or literal integer line number, not a computed expression. A GOTO resolves labels in the current block and then active enclosing blocks. It can jump outward, abandoning a loop or branch, but cannot enter an inactive/sibling block through ordinary GOTO. Duplicate labels are not rejected; resolution uses the first matching label in the searched block. Use unique labels within each program/procedure.

QB additionally supports:

```basic
GOSUB work
PRINT "back"
END
work:
PRINT "working"
RETURN
```

GOSUB executes from the target until RETURN and then resumes at the call site, preserving the caller's active blocks. Targets are collected within the current program or procedure, including nested blocks; they do not cross a procedure boundary. Unlike GOTO, GOSUB can reach a label in an inactive nested block. Falling off the target block without RETURN ends execution rather than acting as an implicit RETURN. RETURN has no label argument. RETURN without GOSUB is an error.

`ON expression GOTO label [, label ...]` and `ON expression GOSUB label [, label ...]` are QB-only. The selector is truncated toward zero to a checked signed 64-bit integer; 1 selects the first target. Zero, negative values, and values beyond the list continue with the next statement.

### Program termination and errors

`END`, `STOP`, `SYSTEM`, and `QUIT` terminate BASIC execution at top level and propagate through SUB calls. Current exception: FUNCTION evaluation discards non-error control-flow results, so these statements inside a FUNCTION stop that function body but do not terminate its caller. They do not provide pause/resume debugging. At the REPL prompt, system/quit handling is additionally specified in [Runtime and tooling](runtime.md).

`EXIT SUB` and `EXIT FUNCTION` return from the corresponding procedure. An unmatched EXIT, RETRY, CONTINUE, or other propagated control-flow operation can raise an error when it escapes its enclosing construct.

Both modes support `WHEN EXCEPTION IN ... USE ... END WHEN`, `RETRY`, `CONTINUE`, `EXTYPE`, and `EXTEXT$`. QB also supports `ON ERROR GOTO label`, `ON ERROR GOTO 0`, `ERROR expression`, `ERR`, `ERL`, and `RESUME [NEXT | label]`. Classic resume granularity follows the top-level statement/block model. See [Error handling](error-handling.md) for the complete behavior and error-code mappings.

## DATA / READ / RESTORE

```basic
READ title$, quantity, price
PRINT title$; quantity; price
RESTORE stock
READ title$
stock: DATA "Rice", 2, 3.5
```

`DATA [item [, item ...]]` accepts numeric literals (including a leading minus), quoted strings, or single unquoted identifiers. It does not evaluate expressions. Unquoted identifiers are uppercased, so `DATA Rice` stores `"RICE"`; keywords, strings containing spaces/commas, and case-sensitive text should be quoted. Leading plus and empty fields are not supported. An empty DATA statement and a trailing comma are accepted but add no extra item.

DATA is collected before execution in textual traversal order, including data in nested branches/loops whether or not the branch executes. The prescan does not enter SUB/FUNCTION bodies, and calls do not prescan them later: DATA inside a procedure is not added to the DATA stream. Keep DATA at module level. All READ operations use one interpreter DATA cursor.

`READ name [, name ...]` accepts bare variable names only. It consumes one item per target and assigns the actual item value without coercing it to a suffix or declared type. Reading past the end raises an error. Earlier target writes and consumed items are not rolled back if a later target fails; the current item's cursor advance also precedes its writable-target check.

`RESTORE` resets the cursor to the first item. `RESTORE label` resets to a label attached **directly to a DATA statement**; a label on a preceding standalone line does not mark that DATA. Unknown/non-DATA labels currently reset to the start silently. The cursor is also reset by CLEAR. Matrix DATA reading is documented in [MAT operations](mat-operations.md).

## Input and output

```basic
PRINT "Hello"; " "; "Rice"
PRINT 1, 2
PRINT TAB(20); "column 20"
PRINT SPC(3); "three spaces"
PRINT
WRITE "Rice", 2, 3.5
INPUT "Name"; name$
LINE INPUT "Description: "; description$
```

PRINT writes expressions consecutively. `;` adds no spacing; `,` advances to the next 16-column zone. A trailing `;` or `,` suppresses the newline; bare PRINT writes a newline. Positive numbers have no leading/trailing spaces, including in QB. `TAB(n)` advances with spaces to a 1-based column if it is ahead; it does not wrap or move backward. `SPC(n)` adds spaces. Both truncate numeric arguments toward zero and reject negative integers. They are PRINT helpers, not general expression functions. `LPRINT` is an alias for PRINT, with no printer device.

`PRINT USING format$; values` and its file form use the [PRINT USING](print-using.md) formatter. `WRITE` outputs comma-separated values with quoted strings and a final newline; console WRITE does not escape quotes inside strings. File WRITE has its own escaping behavior in [File I/O](file-io.md).

Console INPUT accepts an optional literal prompt followed by `;` or `,` and one or more bare variable names. **Both separators currently append `? ` to the prompt**; there is no comma form that suppresses the question mark. With no prompt it prints `? `. A single target receives the whole line; multiple targets split on commas and trim each field, with no quoted-CSV handling. A target whose current/default value is a string receives text; other targets require a finite numeric input. Incorrect field counts or numeric values print `? Redo from start` and retry without partial assignment. End-of-input raises error 62.

LINE INPUT takes an optional literal prompt and one bare target; it removes the input line ending and assigns the remaining text, even to an unsuffixed target. It prints the prompt verbatim without `? `. Array elements and record members are not supported INPUT/LINE INPUT targets. `INPUT$` and `INKEY$`, cursor statements, screen behavior, and output accounting are detailed in [Console](console.md) and [Built-in functions](builtins.md).

Files have dedicated `OPEN`, `CLOSE`, `RESET`, `PRINT #`, `WRITE #`, `INPUT #`, `LINE INPUT #`, `GET`, `PUT`, `FIELD`, `LSET`, `RSET`, `SEEK`, `SET #...: POINTER`, and `ASK #...: POINTER` forms. Their syntax, mode differences, numbering, encoding, and serialization form part of the specification in [File I/O](file-io.md).

## Procedures, records, strings, and matrices

These statement families are fully specified in their linked references:

| Family | Implemented forms |
|---|---|
| [Procedures](procedures.md) | `SUB ... END SUB`, `FUNCTION ... END FUNCTION`, `CALL`, implicit SUB calls, `DECLARE SUB/FUNCTION`, parameter `BYVAL/BYREF`, static procedures, `STATIC`, `SHARED`, `DIM SHARED`, QB `COMMON`, QB `DEF FN` |
| [User-defined types](user-defined-types.md) | `TYPE ... END TYPE`, nested fields, `DIM ... AS type`, record arrays, `record.field`, member assignment |
| [Strings](string-slicing.md) | `name$(start:end)`, slice assignment, QB `MID$(name$, start [, length]) = replacement$` |
| [Matrices](mat-operations.md) | `MAT PRINT`, `MAT INPUT`, `MAT READ`, copy, `ZER`, `CON`, `IDN`, `INV`, `TRN`, addition/subtraction/multiplication, scalar multiplication, `DET` |

## System and filesystem statements

The following statements are available in both modes unless marked QB-only. Paths are string expressions and follow host operating-system rules. Relative paths, including OPEN paths, resolve against the process working directory, not the source-file directory; see [File I/O](file-io.md).

| Statement | Implemented behavior |
|---|---|
| `RANDOMIZE [seed]` | Seed the interpreter RNG from the numeric seed's floating-point bit pattern, or system time. `RANDOMIZE TIMER` is a special spelling for time seeding. Use `RND()` to generate numbers; this is not QBasic's RNG sequence. |
| `SLEEP [seconds]` | Truncate to a checked integer; sleep for positive whole seconds. Missing, zero, and negative values return immediately. No wait-for-key behavior. |
| `NAME old$ AS new$` | Rename a file or directory using host filesystem behavior. |
| `KILL path$` | Remove one file; no wildcard expansion. |
| `MKDIR path$` | Create one directory; does not create missing parents recursively. |
| `RMDIR path$` | Remove one empty directory. |
| `CHDIR path$` | Change the process working directory. |
| `CHDRIVE drive$` | Change to the root formed from the first character, e.g. `C:\`; empty string is a no-op. This is Windows-shaped path behavior and is generally unsuitable on other hosts. |
| `FILES [path$]` | List directory entry names in filesystem order. Defaults to `.`. A directory path lists that directory; another path lists its parent. Wildcards are not filtered. |
| `SHELL [command$]` | Run `sh -c` on non-Windows or `cmd /c` on Windows, waiting for completion. Missing command is a no-op. Child output uses inherited process streams; a nonzero exit code is not a BASIC error. |
| QB `ENVIRON "name=value"` | Set an interpreter-local environment override visible to ENVIRON$ and SHELL child processes. It does not modify the parent process environment. |
| QB `DATE$ = value$`, `TIME$ = value$` | Set validated interpreter-local date/time overrides, not the host clock. See [Built-in functions](builtins.md). |

## Reserved words and compatibility boundaries

The lexer recognizes the following keyword spellings (some are syntax components or explicitly unsupported statements, not independent commands):

```text
ACCESS AND AS ASK BASE BEEP BYREF BYVAL CALL CASE CHAIN CHDIR CHDRIVE
CLEAR CLOSE CLS COLOR COMMON CONST CONTINUE DATA DECLARE DEF DEFDBL
DEFINT DEFLNG DEFSNG DEFSTR DIM DO DOUBLE ELSE ELSEIF END ERASE ERROR
EXCEPTION EXIT EXPLICIT FIELD FILES FOR FREEFILE FUNCTION GET GOSUB GOTO
IF INPUT INTEGER IS KEY KILL LEN LET LOCATE LONG LOOP LPRINT LSET MAT
MKDIR MOD NAME NEXT NOT OFF ON OPEN OPTION OR ORGANIZATION OUTIN OUTPUT
POINTER PRESERVE PRINT PUT QUIT RANDOMIZE READ REDIM REM RESET RESTORE
RESUME RETRY RMDIR RSET SEEK SELECT SEQUENTIAL SET SHARED SHELL SINGLE
SLEEP SPC STATIC STEP STOP STREAM STRING SUB SWAP SYSTEM TAB THEN TIMER
TO TYPE UNTIL USE USING VIEW WEND WHEN WHILE WIDTH WRITE XOR
```

Compound forms are listed in the source-text section. Identifiers such as `NUMERIC`, `DIALECT`, `BINARY`, and `RANDOM` are interpreted contextually rather than always reserved. Builtin names and aliases are listed in [Built-in functions](builtins.md).

There is no implemented graphics mode, event/timer/key handler, SOUND/PLAY music, memory-address access, CHAIN loader, integer division, or general module import/linking. Some unsupported words parse as ordinary implicit SUB calls and fail at runtime instead of producing a dedicated syntax error. See [Compatibility limits](compatibility.md) for verified quirks, standards coverage that remains unknown, and portability limits. Successful parsing alone does not establish historical compatibility.
