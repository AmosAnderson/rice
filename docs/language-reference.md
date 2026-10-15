# RICE BASIC language reference

This reference defines Rice BASIC and its runtime semantics.

The specification is split across this core reference and the following detailed references:

| Subject | Reference |
|---|---|
| Every builtin and its calling convention | [Built-in functions](builtins.md) |
| Declared types, numeric widths, fixed strings, buffers, conversions | [Data types and declarations](data-types.md) |
| SUB, FUNCTION, DEF FN, parameters, scope, STATIC | [Procedures](procedures.md) |
| TYPE records and field access | [User-defined types](user-defined-types.md) |
| String slices and substring assignment | [String slicing](string-slicing.md) |
| Matrix statements | [MAT operations](mat-operations.md) |
| Files, channels, records, FIELD, packed values | [File I/O](file-io.md) |
| Formatted printing | [PRINT USING](print-using.md) |
| Keyboard and text screen | [Console](console.md) |
| Structured and classic error handling | [Error handling](error-handling.md) |
| CLI, REPL, and language server | [Runtime and tooling](runtime.md) |
| Partial features and runtime constraints | [Implementation limits](compatibility.md) |

[SYNTAX.md](../SYNTAX.md) provides a compact syntax index. [Multiple modules](multi-module.md) explains the single-source execution model.

## Execution and defaults

Run a program with `rice program.bas`, or start the REPL with `rice`. See
[Runtime and tooling](runtime.md) for command-line arguments and REPL state.

Comparisons between NUMERIC values return `-1` for true and `0` for false; comparisons involving
native types return BOOL. Numeric logical operators are bitwise. Parameters
default to BYREF. Strings and byte buffers can be concatenated with `+` or `&`.

Arrays start at index 1 by default. Use `OPTION BASE 0` or explicit lower bounds for zero-based indexing. All declarations enforce their storage types: INTEGER is i16, LONG i32, SINGLE f32, NUMERIC/DOUBLE f64, INT64 i64, and BYTE u8. BOOL, BYTES, STRING, and records have their own conversion rules.

## Source text and lexical rules

Source files are UTF-8. Spaces and tabs separate tokens; LF, CRLF, and CR terminate lines. Keywords, labels, and identifiers are case-insensitive and identifiers are normalized to uppercase. String contents retain their case.

Identifiers match an ASCII letter or `_`, followed by ASCII letters, digits, or `_`, and optionally one suffix (`$`, `%`, `!`, `#`, or `&`). Examples: `total`, `_count`, `item2`, `name$`, and `count%`. A suffix is part of the name: `X`, `X%`, and `X#` are different variables. Dots are record member separators, not identifier characters. Unsuffixed keywords cannot be used as ordinary variable names; most builtin names are ordinary identifiers with special expression-call resolution. A suffix bypasses keyword recognition (`STRING$` and `NAME$` are identifiers).

Leave whitespace before a concatenation `&` following an unsuffixed identifier: `a & b`. `a&` is one suffixed identifier. Use the same spacing habit when mixing numeric literals with ampersands.

### Literals

| Form | Examples | Meaning |
|---|---|---|
| Decimal | `0`, `12`, `12.`, `.5`, `3.25` | An `f64` number |
| Exponent | `1E3`, `2.5e-2`, `1D+3` | Decimal exponent; `D` and `E` are equivalent |
| Hexadecimal | `&HFF`, `&HFFFF&` | Unsigned radix-16 integer converted to `f64` |
| Octal | `&O77`, `&O77&` | Unsigned radix-8 integer converted to `f64` |
| String | `"Hello"`, `""`, `"é"` | Text between double quotes, confined to one physical line |

Signs are unary operators, not part of a number token. Decimal numeric-literal suffixes such as `1%` or `1#`, binary literals, digit separators, and hexadecimal signed-width reinterpretation are not implemented. Radix literals must fit `u64`; large integers can lose precision when stored as `f64`.

Inside a string literal, two consecutive double quotes (`""`) represent one quote character. For example, `PRINT "She said ""hi"""` prints `She said "hi"`, and `""""` is a string containing one quote. This Rice extension works wherever a string literal is accepted, including DATA and input prompts. Backslashes remain literal: `\n` is two characters, not a newline. `CHR$(34)` also produces a quote. A string must close before the physical line ends; an unterminated string reports the opening quote's location.

This changes adjacent-quote behavior: `PRINT "a""b"` now prints `a"b`. To print two separate strings consecutively, write `PRINT "a"; "b"`.

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

A colon separates statements. Block constructs should put their headers, bodies, and terminators on separate lines. There is no underscore line-continuation syntax, multiline string literal, or `?` shorthand for PRINT. Comments consume the remainder of their line, including any colons. `!` is not a comment marker; it can be an identifier suffix, and a standalone `!` is a lexical error. Compound keywords use spaces/tabs on the same line: `END IF`, `END SUB`, `END FUNCTION`, `END SELECT`, `END TYPE`, `END DEF`, `END WHILE`, `END WHEN`, and `LINE INPUT`. Use `ELSEIF` as one word.

## Values, variables, and assignment

Runtime values include NUMERIC/DOUBLE (`f64`), INTEGER (`i16`), LONG (`i32`), SINGLE (`f32`), INT64 (`i64`), BOOL, BYTE (`u8`), BYTES, STRING, and TYPE records. Floating-point calculations can produce infinity or NaN; integer arithmetic checks overflow. See [Data types and declarations](data-types.md).

A missing scalar is initialized on first read: `""` for a `$` name and `0` for other names, subject to `DEFSTR` rules and `OPTION EXPLICIT`. Array defaults are discussed below. Conditions accept numeric values and BOOL: zero is false; every nonzero numeric value is true. Strings, byte buffers, and records used as conditions cause a type mismatch.

### Declarations

```basic
DIM count AS NUMERIC
DIM title AS STRING
DIM name$                        ' String default from suffix
DIM samples(1 TO 10) AS DOUBLE
DIM code AS STRING * 8
DIM SHARED total AS NUMERIC
CONST limit = 10, doubled = limit * 2
CONST greeting$ = "Hello", farewell$ = "Goodbye"
```

`DIM [SHARED] declaration [, declaration ...]` accepts scalar names or dimensions, each with an optional `AS type`. Types are `NUMERIC`, `INTEGER`, `LONG`, `SINGLE`, `DOUBLE`, `INT64`, `BOOL`/`BOOLEAN`, `BYTE`, `BYTES`, `STRING`, `STRING * length`, or a declared TYPE name. `length` is a nonnegative integer numeric literal, not a variable or expression. Numeric types initialize to zero in their respective representations; BOOL initializes to false, BYTES to an empty buffer, STRING to `""`, and STRING * n to n spaces. A scalar record initializes its fields recursively.

Declarations execute when reached. Repeating DIM resets the scalar value and retains array elements, validating and converting retained elements before applying the new declaration. AS overrides a suffix. Without AS, DIM uses the suffix type or NUMERIC. STRING * n starts with n spaces and always pads or truncates assignments to n Unicode characters, including scalar, array, field, parameter, and result destinations.

`CONST name [AS type] = expression [, name [AS type] = expression ...]` creates constants in the current scope. Initializers run left to right, so later expressions can use earlier constants. Explicit AS or suffix shorthand converts the initializer to that type before storage. An unsuffixed constant without AS keeps the expression value. Expressions may call functions; compile-time literals are not required. Trailing commas are rejected.

Duplicate definition in the same scope raises an error. If an expression or definition fails, earlier constants and expression side effects remain, and later declarations are skipped; the list is not rolled back. The failing name is marked declared before its expression runs, following the single-declaration behavior. An initializer runs before its duplicate-name check, so its side effects also persist if that check fails. The whole list is one statement for exception-handler CONTINUE/RESUME behavior.

Assignments and declarations cannot overwrite a constant; constants remain after CLEAR. A DIM that would convert an existing array element colliding with a scalar constant fails before replacing the array metadata or values. CONST is enforced at runtime.

### Assignment and type enforcement

```basic
LET count = 5
count = count + 1
name$ = "Rice"
samples(2) = 3.5
```

`LET` is optional. Statement-level `=` assigns; `=` inside an expression compares. Array and record member assignments, slices, and `MID$` assignment have separate forms in this reference and the linked topic guides. Assignments are not expressions and cannot be chained: `a = b = c` assigns the result of the comparison `b = c` to `a`.

All declared types validate assignments to scalars, array elements, record fields, parameters, and function results. Integral destinations require exact in-range integers; SINGLE rounds to f32 and rejects finite overflow. STRING requires text; STRING * n pads or truncates characters. Records require a matching TYPE name. READ, INPUT, SWAP, BYREF copy-back, and binary GET apply destination conversion too.

Suffixes enforce INTEGER (`%`), LONG (`&`), SINGLE (`!`), DOUBLE (`#`), and STRING (`$`) unless an explicit AS overrides them. Plain unsuffixed implicit variables remain dynamic unless DEFtype applies. Arithmetic does not parse numeric strings, and concatenation does not automatically format numbers. See [Data types and declarations](data-types.md) and [Built-in functions](builtins.md).

### Default types

```basic
DEFSTR A-C, N
DEFINT I-J
DEFLNG K
DEFSNG P
DEFDBL X-Z
```

Each range contains single ASCII letters, inclusively. DEFINT, DEFLNG, DEFSNG, DEFDBL, and DEFSTR establish INTEGER, LONG, SINGLE, DOUBLE, and STRING types for subsequently initialized unsuffixed implicit names. Those bindings enforce the selected type thereafter; existing bindings are unchanged. DIM/STATIC without AS use their suffix or NUMERIC, independently of DEFtype. Reversed letter ranges currently have no effect.

### OPTION EXPLICIT

```basic
OPTION EXPLICIT
DIM x
x = 10
```

When execution reaches this statement it enables declaration checks for subsequent operations throughout the interpreter. There is no `OFF` form. It is not a static check or a directive retroactively applied to preceding statements. Reads of existing bindings can succeed even if those bindings were not explicitly declared before the option was enabled.

Names count as declared through executed `DIM`, `REDIM`, `SHARED`, `STATIC`, `COMMON`, or `CONST`; through a procedure parameter or result binding; through a `FOR` variable; through any supported type suffix; or through an executed DEFtype letter range. Suffixes therefore bypass the need for DIM, including on an array name. Recognized special values and zero-argument user functions can still be read. Some specialized statements have weaker checks; this option is not a full declaration/type analysis. Scope details are in [Procedures](procedures.md).

### SWAP, CLEAR, and scope declarations

`SWAP target, target` exchanges scalar variables or array elements. Each destination validates conversion of the other value before either write. Record members and slices are unsupported. Missing values use their declared defaults; untyped names use their implicit defaults. A failed conversion leaves both values unchanged.

Both destinations are resolved from left to right, evaluating each index exactly once, before either value is read or written. Thus `SWAP i, items(i)` selects the element using the old `i`; swapping a location with itself leaves its value unchanged. A target error leaves both destinations unwritten, although side effects from evaluating indices remain. Constants and `OPTION EXPLICIT` follow the same checks as INPUT targets. Array targets retain the usual flattened storage and do not add bounds or rank checks.

`CLEAR` removes variable values in the current environment and resets the DATA cursor to the start. It retains constants, declaration flags, procedure/type definitions, options, and array bounds/default-type metadata. It does not close files or fully recreate the interpreter. Optional arguments after CLEAR are parsed and ignored.

`SHARED name[()] [AS type] [, ...]`, `DIM SHARED ...`, `STATIC ...`, and `COMMON [SHARED] ...` control procedure state. COMMON does not load or link modules; `CHAIN` is unsupported. See [Procedures](procedures.md) and [Multiple modules](multi-module.md) for exact sharing and persistence limitations.

## Expressions and operators

Parentheses group expressions. Named calls use `name(argument [, argument ...])`. Because the same syntax is used for arrays, a parenthesized name is resolved by the interpreter: special stateful function, registered builtin, user FUNCTION, then array element. Consequently an unknown function name can silently behave as an array access and return a default value. Existing builtin names cannot reliably be used for arrays or user functions.

Only specifically supported functions may be written without parentheses. In particular use `RND()`, `PI()`, and `MAXNUM()`; bare `RND`, `PI`, and `MAXNUM` are ordinary variables. Use bare `TIMER` and `FREEFILE`; the parser does not consume an empty parenthesis pair for them. See [Built-in functions](builtins.md) for all exact call forms, including aliases and special values.

Operands and ordinary argument lists are evaluated left to right. `AND`, `OR`, and `XOR` **do not short-circuit**; `0 AND (1 / 0)` raises an error. `EQV` and `IMP` also evaluate both operands, so `0 IMP (1 / 0)` raises an error.

| Precedence, highest first | Operators | Association |
|---|---|---|
| Primary | Literals, references, calls, member access, parentheses | — |
| Power | `^` | Right |
| Unary sign | `+`, `-` | Prefix |
| Multiply/divide | `*`, `/` | Left |
| Integer divide | `\` | Left |
| Modulus | `MOD` | Left |
| Add/subtract/concatenate | `+`, `-`, `&` | Left |
| Comparison | `=`, `<>`, `<`, `>`, `<=`, `>=`, `LIKE` | Left |
| Logical/bitwise negation | `NOT` | Prefix |
| Conjunction | `AND` | Left |
| Disjunction | `OR` | Left |
| Exclusive OR | `XOR` | Left |
| Equivalence | `EQV` | Left |
| Implication | `IMP` | Left |

For example `-2^2 = -4`, `2^-2 = 0.25`, and `2^3^2 = 512`. Chained comparisons are evaluated as successive binary operations: `2 < 1 < 1` means `(2 < 1) < 1`, which is true. Use `(a < b) AND (b < c)` for a range test.

Arithmetic operators require numbers. `/` is real division; division by zero raises an error. `MOD` uses **`a - b * FLOOR(a / b)`**, including real operands. Examples: `-5 MOD 3 = 1`, `5 MOD -3 = -1`, and `5.5 MOD 2 = 1.5`. A zero divisor raises an error.

Rice also supports integer division, `a \ b`. With legacy numeric operands, both round to the nearest integer, with exact halfway values rounding to the even integer. The rounded operands must fit signed 64-bit integers; division then truncates the quotient toward zero and returns an `f64`. For example, `7 \ 2 = 3`, `-7 \ 2 = -3`, `7.5 \ 2 = 4`, and `6.5 \ 2 = 3`. `*` and `/` bind more tightly than `\`, which binds more tightly than `MOD`: `20 \ 2 * 3 = 3` and `20 \ 3 MOD 2 = 0`. Repeated integer divisions associate left to right.

A divisor that rounds to zero (including `0.5` or `-0.5`) raises division by zero (`ERR = 11`). Nonfinite or out-of-range rounded operands, and `(-2 ^ 63) \ -1`, raise overflow (`ERR = 6`). These errors also work with structured exception handlers. Large results from legacy numeric division can lose precision when converted back to `f64`.

`MIN(number [, number ...])` and `MAX(number [, number ...])` select numeric extrema. They require at least one numeric argument; see [Built-in functions](builtins.md#mathematics-and-numeric-conversion) for evaluation and nonfinite-value behavior.

Ordering and equality comparison work on two numbers, two strings, two native booleans, or two byte buffers. Legacy numeric/string comparisons return `-1` for true and `0` for false; comparisons involving native types return BOOL. Mixed integer/floating comparisons retain exact integer ordering. Mixed string/number expression comparison raises a type mismatch. Strings compare case-sensitively in lexicographic Unicode order. The separate `STRCOMP(a$, b$[, compare])` builtin can compare after Unicode lowercasing; see [builtins](builtins.md#strings). Ordinary operators do not compare records. `LIKE` accepts strings only, as described below.

For legacy numeric operands, `AND`, `OR`, `XOR`, `NOT`, `EQV`, and `IMP` cast numbers to signed 64-bit integers, truncating fractions toward zero, perform the bitwise operation, and return an `f64`. These casts saturate out-of-range values and map NaN to zero; these legacy casts do not report overflow. Large integer results may lose precision when stored as `f64`.

`EQV` complements the bitwise XOR of the converted operands. `IMP` combines the bitwise complement of its left operand with its right operand using OR. For example, `5 EQV 3` returns `-7`, and `5 IMP 3` returns `-5`. Comparisons bind more tightly, so use `(5 EQV 3) = -7` when testing the result. For truth values:

| Left | Right | `EQV` | `IMP` |
|---|---|---|---|
| `-1` | `-1` | `-1` | `-1` |
| `-1` | `0` | `0` | `0` |
| `0` | `-1` | `0` | `-1` |
| `0` | `0` | `-1` | `-1` |

`XOR` binds more tightly than `EQV`, which binds more tightly than `IMP`. Repeated operators associate left to right: `1 IMP 2 IMP 4` gives `5`. Strings and records are rejected.

`&` concatenates two strings or two BYTES buffers. `+` also concatenates strings or buffers. Mixing strings, buffers, or numbers in concatenation raises a type mismatch. Native BOOL logical operations use boolean results. See [Data types and declarations](data-types.md) for checked integer arithmetic and explicit numeric/boolean conversion.

### Wildcard matching

`text$ LIKE pattern$` matches the **entire string**, case-sensitively. It is a Rice extension. A match returns `-1`; a nonmatch returns `0`. LIKE has comparison precedence and associates left to right, so concatenation binds more tightly and NOT/AND/OR bind less tightly. Both operands evaluate exactly once, left to right, and must be strings; other values raise type mismatch (error 13).

| Pattern | Meaning |
|---|---|
| `*` | Any sequence of zero or more Unicode scalar values. |
| `?` | Exactly one Unicode scalar value. |
| `#` | Exactly one ASCII digit, `0` through `9`. |
| `[abc]` | One character from a set. |
| `[a-z]` | One character in an inclusive range, ordered by Unicode scalar value. Sets and ranges can be combined. |
| `[!abc]`, `[!a-z]` | One character outside the set/range. `!` negates only at the start of a class. |
| `[*]`, `[?]`, `[#]`, `[[]` | Literal `*`, `?`, `#`, or `[`. A `]` outside a class is already literal. |

A leading or trailing `-` inside a class is literal. Backslash has no escape meaning. Other characters match literally. Empty classes (`[]`, `[!]`), unclosed classes, and reversed ranges such as `[z-a]` raise illegal function call (error 5), even when another part of the pattern would not match. The empty pattern matches only the empty string; `*` also matches an empty string. Matching uses an iterative algorithm with no recursive backtracking. There is no locale collation or Unicode normalization; a combining accent counts as a separate scalar value.

```basic
PRINT "notes.bas" LIKE "*.bas"       ' True
PRINT "item7" LIKE "item#"           ' True
PRINT "é" LIKE "?"                  ' True
PRINT "A" LIKE "[a-z]"              ' False
IF "report.txt" LIKE "*.txt" THEN PRINT "text file"
```

LIKE is now a reserved unsuffixed keyword; use a different name for an existing variable named `like` (`like$` remains an identifier).

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

Dimensions are comma-separated `upper` or `lower TO upper` expressions. Their bounds are evaluated when DIM/REDIM executes and converted to signed 64-bit integers by truncation toward zero; nonfinite/unrepresentable numbers are rejected. Bounds are inclusive. A reversed range or overflowing range size is rejected. `OPTION BASE 0` or `OPTION BASE 1` changes the implicit lower bound for subsequent dimensions in that environment. The initial value is **1**; changing it does not change already recorded bounds.

`LBOUND(array [, dimension])` and `UBOUND(array [, dimension])` read dimension metadata. Dimensions are numbered from 1, with 1 the default. Missing array metadata or an invalid dimension raises an error. Use the bare array name as the first argument.

Rice arrays are currently sparse values stored under flattened names, separate from their bounds metadata:

- Ordinary element access does **not** enforce DIM bounds, the number of dimensions, or a requirement to DIM first when declaration checks permit the name.
- Indices truncate toward zero using checked signed 64-bit conversion. `a(1.9)` accesses `a(1)`.
- `a(1, 2)` is stored under the key `A_1_2`. This can collide with a scalar named `a_1_2` or an element `a_1(2)`. Avoid such scalar/array naming combinations.
- Scalar `a` and its elements coexist. A bare array name in an ordinary expression is the scalar slot, not a whole-array value.
- Missing elements use an explicit `DIM`/`REDIM` `AS` type: `STRING` yields `""`, `STRING * n` yields n spaces, numeric types yield their own zero, BOOL yields false, BYTES yields an empty buffer, and a TYPE name yields a recursively initialized record. `AS` takes precedence over the name suffix and DEFSTR; for example, `DIM a(2) AS STRING` makes untouched `a(1)` a string, while `DIM a$(2) AS NUMERIC` makes it numeric. Without AS, DIM/REDIM select the suffix type or NUMERIC. Later DEFtype changes do not retype existing declarations. UDT records initialize lazily on ordinary reads.
- `DIM` updates metadata and a scalar default but does not erase existing elements. Each declaration validates and converts retained elements before applying the declaration. Array assignment evaluates the right-hand value before its index expressions.

Element types are retained and used by ordinary reads, empty DATA items, console/file INPUT, SWAP, and GET/PUT. Every declared element assignment is converted and checked. Retained AS metadata selects binary widths, overriding suffix shorthand; see [File I/O](file-io.md#typed-binary-serialization).

`REDIM [PRESERVE]` accepts the same declarations as DIM. Without PRESERVE, it removes existing keys beginning with the array name plus `_`; with PRESERVE, it keeps **all** such values, including values outside new bounds. It permits changes to any dimension. It resets the scalar slot even with PRESERVE. Bounds, type constraints, and default construction are validated before replacing the current declaration or clearing its elements. Earlier successful declarations and bound-expression side effects remain if a later declaration in the same statement fails. A successful array DIM or REDIM replaces the explicit default type. Omitting AS in DIM or non-PRESERVE REDIM selects the suffix type or NUMERIC; omitting AS in REDIM PRESERVE retains an existing declared type. REDIM PRESERVE rejects an element-type change; NUMERIC and DOUBLE are aliases. Scalar DIM leaves a same-named array's metadata intact; REDIM without dimensions removes it.

`ERASE name [, name ...]` removes keys with that prefix and resets a declared scalar slot to its typed default; untyped scalar slots reset to numeric `0`. It retains dimension/type metadata and declaration state. Because removal uses a name prefix, ERASE and non-PRESERVE REDIM can also remove colliding scalar names or another array with that prefix. They do not implement a distinction between static and dynamic arrays. Array parameter/sharing limitations are in [Procedures](procedures.md); whole-matrix operations are in [MAT operations](mat-operations.md).

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

THEN followed by a newline/comment starts a block IF; statements following THEN on the same line form a single-line IF. Single-line branches can contain colon-separated statements. ELSEIF clauses belong to the block form. Conditions are evaluated in order until one is true; only that branch executes.

Rice also permits a literal line number at the start of either single-line branch as shorthand for `GOTO`:

```basic
10 x = -1
20 IF x > 0 THEN 100 ELSE 200
100 PRINT "positive": END
200 PRINT "zero or negative": END
```

Either branch can mix with ordinary statement syntax, such as `IF ready THEN 100 ELSE PRINT "waiting"` or `IF ready THEN PRINT "ready" ELSE 200`. The numeric target must be a nonnegative integral literal in the same range as a GOTO line number (`0..4294967295`). Computed expressions are not valid targets; named labels require explicit `GOTO`. A following colon introduces more statements in that branch, though an executed jump transfers control before reaching them. Nested single-line IF statements associate ELSE with the nearest unmatched IF. Targets use the existing GOTO label scope; an executed jump to an undefined target raises an error.

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

NEXT may omit the variable or name the matching variable. A comma-separated list closes nested FOR loops from the innermost outward: `NEXT j, i` is equivalent to `NEXT j` followed by `NEXT i`. Every listed variable must match its loop, and each comma requires names on both sides. A list can close some or all enclosing FOR loops, but cannot cross an unclosed IF, DO, WHILE, SELECT, exception, or procedure block. Existing restrictions on labels before block terminators still apply. EXIT FOR exits the enclosing active FOR; it is not a generic loop exit.

```basic
FOR row = 1 TO 2
    FOR column = 1 TO 3
        PRINT row; ","; column
NEXT column, row
```

Comments may follow the list, and a colon starts the next statement after all listed loops finish. A mismatched, missing, or excess loop variable is a parse error; lists cannot continue onto another physical line.

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

WHILE checks before each iteration and accepts either `WEND` or `END WHILE`. `EXIT WHILE` immediately leaves the nearest active WHILE loop and resumes after its terminator, without checking the condition again. It can appear inside an IF, SELECT, DO, FOR, or exception block nested within that WHILE. An inner WHILE consumes its own exit, leaving outer loops active. EXIT WHILE is a Rice extension; it cannot leave a caller's loop from inside a SUB or FUNCTION. Executing it without an active WHILE in the current procedure or top-level execution raises an error.

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

The selector is evaluated once. Cases are tested in source order, with a comma-separated test list acting as alternatives. Ranges are inclusive. `CASE IS` accepts any comparison operator. The first matching branch executes, with no fallthrough. CASE ELSE must be last and is optional. String values and string ranges are supported.

`EXIT SELECT` immediately leaves the nearest active SELECT CASE and resumes after END SELECT, including when used in CASE ELSE or a nested IF, loop, or exception block. It skips the remainder of the branch; no later cases are tested. An inner SELECT consumes its own exit, leaving the outer SELECT active. EXIT SELECT is a Rice extension. It cannot leave a caller's SELECT from inside a SUB or FUNCTION, and executing it without an active SELECT in the current procedure or top-level execution raises an error.

CASE comparisons use value equality/ordering directly, unlike expression comparisons: mixed types generally fail to match rather than raising a type mismatch, and `CASE IS <>` can match unequal mixed types. CASE equality can compare records structurally; record ordering is unavailable.

### GOTO, GOSUB, and computed jumps

```basic
GOTO done
PRINT "skipped"
done:
PRINT "done"
```

GOTO accepts a named label or literal integer line number, not a computed expression. A GOTO resolves labels in the current block and then active enclosing blocks. It can jump outward, abandoning a loop or branch, but cannot enter an inactive/sibling block through ordinary GOTO. Duplicate labels are not rejected; resolution uses the first matching label in the searched block. Use unique labels within each program/procedure.

Rice also supports:

```basic
GOSUB work
PRINT "back"
END
work:
PRINT "working"
RETURN
```

GOSUB executes from the target until RETURN and then resumes at the call site, preserving the caller's active blocks. Targets are collected within the current program or procedure, including nested blocks; they do not cross a procedure boundary. Unlike GOTO, GOSUB can reach a label in an inactive nested block. Falling off the target block without RETURN ends execution rather than acting as an implicit RETURN.

`RETURN [label]` ends exactly the most recent active GOSUB. With no target it resumes at the following statement. With a named label or literal line number it instead transfers control using that caller's GOTO resolution: active enclosing blocks can receive the jump, and an outward jump abandons intervening blocks. If the caller itself is inside another GOSUB, that still-active GOSUB can also resolve the jump through its collected label targets; otherwise inactive/sibling blocks cannot be entered. The target uses the same literal-number rules as GOTO; expressions are rejected. An unknown or unreachable target raises an undefined-label error. Both forms require an active GOSUB in the current program or procedure; RETURN without GOSUB is an error, including inside SUB and FUNCTION.

```basic
GOSUB work
PRINT "skipped"
done: PRINT "done"
END
work: RETURN done
```

Rice also permits targeted RETURN inside a SUB or FUNCTION, using that procedure's local labels. It cannot transfer into the module or another procedure. RETURN does not replace EXIT SUB or EXIT FUNCTION.

`ON expression GOTO label [, label ...]` and `ON expression GOSUB label [, label ...]` select a branch from a list. The selector is truncated toward zero to a checked signed 64-bit integer; 1 selects the first target. Zero, negative values, and values beyond the list continue with the next statement.

### Program termination and errors

`END`, `STOP`, `SYSTEM`, and `QUIT` terminate BASIC execution at top level and propagate through SUB calls. Current exception: FUNCTION evaluation discards non-error control-flow results, so these statements inside a FUNCTION stop that function body but do not terminate its caller. They do not provide pause/resume debugging. At the REPL prompt, system/quit handling is additionally specified in [Runtime and tooling](runtime.md).

`EXIT SUB` and `EXIT FUNCTION` return from the corresponding procedure. An unmatched EXIT, RETRY, CONTINUE, or other propagated control-flow operation can raise an error when it escapes its enclosing construct.

Rice supports `WHEN EXCEPTION IN ... USE ... END WHEN`, `RETRY`, `CONTINUE`, `EXTYPE`, and `EXTEXT$`. Rice also supports `ON ERROR GOTO label`, `ON ERROR GOTO 0`, `ERROR expression`, `ERR`, `ERL`, and `RESUME [NEXT | label]`. Classic resume granularity follows the top-level statement/block model. See [Error handling](error-handling.md) for the complete behavior and error-code mappings.

## DATA / READ / RESTORE

```basic
READ title$, quantity, price
PRINT title$; quantity; price
RESTORE stock
READ title$
stock: DATA "Rice", 2, 3.5
```

`DATA [item [, item ...]]` accepts numeric literals with an optional leading `+` or `-`, quoted strings, or single unquoted identifiers. It does not evaluate expressions or named constants. Unquoted identifiers are uppercased, so `DATA Rice` stores `"RICE"`; keywords, strings containing spaces/commas, and case-sensitive text should be quoted. Items require commas between them. Signs apply only to a numeric literal: `DATA +2, -3.5E1` is valid, while `DATA 1 + 2` and `DATA +name` are rejected.

Missing comma-separated fields are empty DATA items. Leading, consecutive, and trailing commas all count: `DATA ,7,,` stores four items (empty, `7`, empty, empty), and `DATA ,` stores two empty items. A bare DATA statement stores no items. A trailing comma therefore adds an item, unlike earlier Rice versions, which discarded it. Comments and statement-ending colons do not remove a preceding empty field.

An empty DATA item supplies the destination's declared default: numeric zero, false, empty buffer, empty variable string, or spaces for STRING * n. Dynamic destinations use their current value's type. Records reject empty items without consuming that item. Quoted DATA text is parsed according to a declared numeric, BOOL, or BYTES destination; string destinations retain text. MAT READ treats an empty item as numeric zero.

DATA is collected before execution in textual traversal order, including data in nested branches/loops whether or not the branch executes. The prescan does not enter SUB/FUNCTION bodies, and calls do not prescan them later: DATA inside a procedure is not added to the DATA stream. Keep DATA at module level. All READ operations use one interpreter DATA cursor.

READ accepts scalar names and array elements. Each destination resolves and assigns in order, so READ count, values(count) uses the new count. Index expressions evaluate once and truncate through checked integer conversion. Declared destinations convert and validate each value. Quoted integer text parses directly with range checks, avoiding an f64 intermediate; BOOL accepts TRUE/FALSE/1/0 and BYTES hexadecimal pairs with optional &H. Earlier assignments and consumed items remain on later failure. A destination-resolution error leaves that target's item unconsumed.

Array destinations use the existing flattened storage and declaration rules: the base name is checked under `OPTION EXPLICIT`, including the usual suffix/DEFtype exceptions, and bounds/rank are not enforced. Constants cannot be overwritten, including a scalar constant that aliases a flattened array key. Empty array subscripts, string slices, and record-member destinations are rejected. These rules apply to READ and console/file INPUT and LINE INPUT.

```basic
DIM values(3), count
READ count, values(count)
PRINT values(2)                 ' 42
DATA 2, 42
```

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

PRINT writes expressions consecutively. `;` adds no spacing; `,` advances to the next 16-column zone. A trailing `;` or `,` suppresses the newline; bare PRINT writes a newline. Positive numbers have no leading/trailing spaces, including. `TAB(n)` advances with spaces to a 1-based column if it is ahead; it does not wrap or move backward. `SPC(n)` adds spaces. Both truncate numeric arguments toward zero and reject negative integers. They are PRINT helpers, not general expression functions. `LPRINT` is an alias for PRINT, with no printer device.

`PRINT USING format$; values` and its file form use the [PRINT USING](print-using.md) formatter. `WRITE` outputs comma-separated values with quoted strings and a final newline; console WRITE does not escape quotes inside strings. File WRITE has its own escaping behavior in [File I/O](file-io.md).

Console INPUT accepts an optional literal prompt followed by `;` or `,` and one or more scalar or array-element targets. **Both separators currently append `? ` to the prompt**; there is no comma form that suppresses the question mark. With no prompt it prints `? `. A single target receives the whole line; multiple targets split on commas and trim each field, with no quoted-CSV handling. A target whose current/default value is a string receives text; other targets require a finite numeric input. Incorrect field counts or numeric values print `? Redo from start` and retry without partial assignment. End-of-input raises error 62.

INPUT resolves all destination indices once, left to right, before prompting or consuming file data. Console retries keep those destinations. Values are assigned only after the whole input list succeeds, so `INPUT i, values(i)` uses the value of `i` from before input; this differs from sequential READ. Failed destination resolution consumes no input, though side effects from index expressions are not rolled back. Primitive array defaults follow ordinary element reads, including explicit AS STRING/STRING * n declarations for untouched string elements.

LINE INPUT takes an optional literal prompt and one scalar or array-element destination, resolves it once, and removes the input line ending. Text is assigned through its declared type; declared numeric destinations reject it. Dynamic implicit targets can receive text. LINE INPUT # requires a string destination before reading. Members and slices remain unsupported. See [Console](console.md).

Files have dedicated `OPEN`, `CLOSE`, `RESET`, `PRINT #`, `WRITE #`, `INPUT #`, `LINE INPUT #`, `GET`, `PUT`, `FIELD`, `LSET`, `RSET`, `SEEK`, `SET #...: POINTER`, and `ASK #...: POINTER` forms. Their syntax, numbering, encoding, and serialization form part of the specification in [File I/O](file-io.md).

## Procedures, records, strings, and matrices

These statement families are fully specified in their linked references:

| Family | Implemented forms |
|---|---|
| [Procedures](procedures.md) | `SUB ... END SUB`, `FUNCTION ... END FUNCTION`, `CALL`, implicit SUB calls, `DECLARE SUB/FUNCTION`, parameter `BYVAL/BYREF`, static procedures, `STATIC`, `SHARED`, `DIM SHARED`, `COMMON`, `DEF FN` |
| [User-defined types](user-defined-types.md) | `TYPE ... END TYPE`, nested fields, `DIM ... AS type`, record arrays, `record.field`, member assignment |
| [Strings](string-slicing.md) | `name$(start:end)`, slice assignment, `MID$(target, start [, length]) = replacement$` for scalar or array-element targets |
| [Matrices](mat-operations.md) | `MAT PRINT`, `MAT INPUT`, `MAT READ`, copy, `ZER`, `CON`, `IDN`, `INV`, `TRN`, addition/subtraction/multiplication, scalar multiplication, `DET` |

## System and filesystem statements

The following host statements are available. Paths are string expressions and follow host operating-system rules. Relative paths, including OPEN paths, resolve against the process working directory, not the source-file directory; see [File I/O](file-io.md).

| Statement | Implemented behavior |
|---|---|
| `RANDOMIZE [seed]` | Seed the interpreter RNG from the numeric seed's floating-point bit pattern, or system time. `RANDOMIZE TIMER` is a special spelling for time seeding. Use `RND()` to generate numbers. |
| `SLEEP [seconds]` | Truncate to a checked integer; sleep for positive whole seconds. Missing, zero, and negative values return immediately. No wait-for-key behavior. |
| `NAME old$ AS new$` | Rename a file or directory using host filesystem behavior. |
| `KILL path$` | Remove one file; no wildcard expansion. |
| `MKDIR path$` | Create one directory; does not create missing parents recursively. |
| `RMDIR path$` | Remove one empty directory. |
| `CHDIR path$` | Change the process working directory. |
| `CHDRIVE drive$` | Change to the root formed from the first character, e.g. `C:\`; empty string is a no-op. This is Windows-shaped path behavior and is generally unsuitable on other hosts. |
| `FILES [path$]` | List directory entry names in filesystem order. Defaults to `.`. A directory path lists that directory; another path lists its parent. Wildcards are not filtered. |
| `SHELL [command$]` | Run `sh -c` on non-Windows or `cmd /c` on Windows, waiting for completion. Missing command is a no-op. Child output uses inherited process streams; a nonzero exit code is not a BASIC error. |
| `ENVIRON "name=value"` | Set an interpreter-local environment override visible to ENVIRON$ and SHELL child processes. It does not modify the parent process environment. |
| `DATE$ = value$`, `TIME$ = value$` | Set validated interpreter-local date/time overrides, not the host clock. See [Built-in functions](builtins.md). |

## Reserved words and implementation limits

The lexer recognizes the following keyword spellings (some are syntax components or explicitly unsupported statements, not independent commands):

```text
ACCESS AND AS ASK BASE BEEP BYREF BYVAL CALL CASE CHAIN CHDIR CHDRIVE
CLEAR CLOSE CLS COLOR COMMON CONST CONTINUE DATA DECLARE DEF DEFDBL
DEFINT DEFLNG DEFSNG DEFSTR DIM DO DOUBLE ELSE ELSEIF END EQV ERASE ERROR
EXCEPTION EXIT EXPLICIT FIELD FILES FOR FREEFILE FUNCTION GET GOSUB GOTO
IF IMP INPUT INTEGER IS KEY KILL LEN LET LIKE LOCATE LONG LOOP LPRINT LSET MAT
MKDIR MOD NAME NEXT NOT OFF ON OPEN OPTION OR ORGANIZATION OUTIN OUTPUT
POINTER PRESERVE PRINT PUT QUIT RANDOMIZE READ REDIM REM RESET RESTORE
RESUME RETRY RMDIR RSET SEEK SELECT SEQUENTIAL SET SHARED SHELL SINGLE
SLEEP SPC STATIC STEP STOP STREAM STRING SUB SWAP SYSTEM TAB THEN TIMER
TO TYPE UNTIL USE USING VIEW WEND WHEN WHILE WIDTH WRITE XOR
```

Compound forms are listed in the source-text section. Identifiers such as `NUMERIC`, `BINARY`, and `RANDOM` are interpreted contextually rather than always reserved. Builtin names and aliases are listed in [Built-in functions](builtins.md).

There is no implemented graphics mode, event/timer/key handler, SOUND/PLAY music, memory-address access, CHAIN loader, or general module import/linking. Some unsupported words parse as ordinary implicit SUB calls and fail at runtime instead of producing a dedicated syntax error. See [Implementation limits](compatibility.md) for partial features, runtime constraints, and verification gaps. Successful parsing alone does not establish runtime support.
