# Procedures and Scope

This page specifies the procedures implemented by Rice BASIC. See the [language reference](language-reference.md) for core language rules.

## Definitions and calls

```text
SUB name [(parameter, ...)] [STATIC]
    statements
END SUB

FUNCTION name [(parameter, ...)] [AS type] [STATIC]
    statements
END FUNCTION

parameter := [BYVAL | BYREF] name [()] [AS type]
```

Parentheses in definitions are optional when there are no parameters. Names are
case-insensitive. A `SUB` returns no value; a `FUNCTION` returns the value assigned
to its own name. `EXIT SUB` and `EXIT FUNCTION` end the corresponding procedure
early. Parameters are positional, and the number of arguments must match exactly;
there are no optional, named, or variadic parameters.

```basic
SUB Banner(title$, span)
    PRINT STRING$(span, "=")
    PRINT title$
END SUB

FUNCTION Factorial(n) AS NUMERIC
    IF n <= 1 THEN
        Factorial = 1
    ELSE
        Factorial = n * Factorial(n - 1)
    END IF
END FUNCTION

CALL Banner("Rice", 12)
Banner "Hello", 12
PRINT Factorial(5)                  ' 120
```

| Call form | Behavior |
| --- | --- |
| `CALL P(x, y)` | Calls `SUB P`; a DB... builtin instead runs and discards its result |
| `P x, y` | Calls `SUB P` |
| `CALL P` or `P` | Calls a zero-argument `SUB` |
| `P(x, y)` | Not a parenthesized argument-list form; use `CALL` |
| `P (x)` | One parenthesized expression; forces value passing |
| `CALL P((x))` | Forces value passing for `x` |
| `F(x, y)` in an expression | Calls a function or accesses an array |

With `CALL`, arguments must be inside parentheses: `CALL P x` is not supported.
Without `CALL`, individual arguments may be parenthesized, as in
`P (x), (y)`, but a comma-separated list cannot be enclosed in one pair.

Zero-argument functions can be called with `F()` or, when no variable named `F`
already resolves, with bare `F`. Use `F()` to avoid the variable ambiguity.
`CALL` resolves database builtins first, then `SUB` definitions. `CALL DBEXEC(db, sql$)` and other DB... calls discard their result and evaluate arguments by value with no copy-back. Other expression functions cannot be invoked through CALL. See [SQLite](sqlite.md).

Definitions are collected before top-level execution, so calls may precede their
definitions. Definitions inside top-level `IF`, loop, `SELECT CASE`, and exception
blocks are also collected regardless of whether that block executes. A procedure
body is not recursively scanned for nested definitions: nested `SUB`, `FUNCTION`,
`TYPE`, and `DATA` declarations are not a supported way to define local entities.
Repeated procedure names replace earlier definitions. Recursion is supported, but
depth is limited by the host stack; there is no specified portable recursion limit.

## Function result types

`AS type` initializes and enforces the function-name result binding at each call. STRING starts empty; STRING * n starts with n spaces; numeric types start with their own zero, BOOL false, BYTES an empty buffer, and records recursively initialized fields. Explicit AS overrides suffix shorthand. A suffix without AS enforces its corresponding type. An unsuffixed result without AS starts at numeric zero and remains dynamic; DEFtype does not change that result default. EXIT FUNCTION returns the current result.

Result assignments obey the same conversion rules as variables, including integer range checks, fixed-string padding/truncation, and matching record TYPE names. The initial result is fresh even for recursive calls and STATIC functions. Undefined or recursive result TYPEs fail before the body runs. TYPE definitions are prescanned.

```basic
FUNCTION Greeting$(who$) AS STRING
    Greeting$ = "Hello, " & who$
END FUNCTION
PRINT Greeting$("world")
```

Record results can be populated directly through the function-name variable:

```basic
TYPE Point
    x AS NUMERIC
    y AS NUMERIC
END TYPE
FUNCTION Origin() AS Point
    Origin.x = 10
END FUNCTION
PRINT Origin().x; ","; Origin().y   ' 10,0
```

Use `EXIT FUNCTION` for an early return. `RETURN` belongs to `GOSUB` and is not a function-return statement. `END` inside a `SUB` stops the whole
program. A current implementation limitation is that `END` inside a `FUNCTION`
does not stop the caller; do not use it for program termination. Unmatched RETURN
and unresolved GOTO/RETURN targets at a function boundary report runtime errors.
An `EXIT WHILE` or `EXIT SELECT` must find a matching active construct within
the current procedure; it raises an error instead of exiting a caller's block.
Labels and `GOSUB` targets in a procedure are local to that invocation.
See [Error Handling](error-handling.md)
for the limitations of error handlers in procedures.

## DEF functions

Rice accepts single-expression and multiline `DEF` functions.

```basic
DEF FNSquare(x) = x * x

DEF FNAbsolute(x)
    IF x < 0 THEN
        FNAbsolute = -x
    ELSE
        FNAbsolute = x
    END IF
END DEF

PRINT FNSquare(4); ","; FNAbsolute(-3)   ' 16,3
```

The `FN` prefix is conventional, not required by the parser. Parameter syntax and
passing rules are the same as for `FUNCTION`. `DEF` has no return `AS` clause or
procedure-wide `STATIC` modifier. Its body returns by assignment to its name and
may use `EXIT FUNCTION`; `EXIT DEF` is not recognized.

## DECLARE

```text
DECLARE SUB name [(parameter, ...)]
DECLARE FUNCTION name [(parameter, ...)] [AS type]
```

DECLARE signatures are checked against matching definitions after source prescan and before execution. The procedure kind, parameter count, passing modes, array markers, parameter types, and function result type must agree; parameter names may differ. NUMERIC and DOUBLE are equivalent. A declaration without a definition can remain available for a later REPL submission, but cannot supply executable code. Runtime conversion follows the actual definition.

If signature validation fails, the submission does not execute or retain its prescanned definitions, declarations, TYPE definitions, or DATA. Previously accepted definitions and DATA remain available, so a rejected declaration or redefinition does not prevent later REPL commands from running. Errors raised after validation during execution retain the usual runtime side effects.

## Parameter passing

| Property | Behavior |
| --- | --- |
| Unqualified parameter | `BYREF` |
| Explicit `BYREF` / `BYVAL` | Accepted |
| Explicit `AS type` or suffix shorthand | Converts arguments and enforces assignments, including record identity and fixed strings |
| Unsuffixed parameter without AS | Keeps dynamic argument values |
| Whole record argument | Copied in; copied back for a `BYREF` variable or array element |
| Array parameter `a()` | Syntax accepted; array binding is not implemented |

Arguments are evaluated left to right in the caller, before entering the procedure.
Each value is copied into a parameter. At exit, parameters marked `BYREF` are
copied back when the corresponding argument is a **variable or array element**,
including a string or record-valued element and multidimensional subscripts.
Each index is evaluated exactly once during that argument's evaluation. The
destination is then fixed: a later argument or the procedure body changing an
index variable does not redirect the write-back.

Parenthesized expressions, arithmetic expressions, function results, member
accesses, and slices have no write-back destination, even with explicit `BYREF`.
For ambiguous `name(args)` expressions, builtin and user-function lookup still
take precedence over arrays. Constants, including constant aliases of flattened
element keys and constant array base names, are passed without write-back.
Index conversion, declaration checks, defaults, and scope lookup follow ordinary
array reads; this does not add bounds/rank enforcement or whole-array binding.

```basic
SUB Change(BYREF x)
    x = 42
END SUB

n = 10
CALL Change(n)
PRINT n                            ' 42
n = 10
CALL Change((n))
PRINT n                            ' 10

DIM a(1 TO 2)
a(1) = 7
CALL Change(a(1))
PRINT a(1)                         ' 42
CALL Change((a(1)))                ' Explicit expression: no write-back
```

This is copy-in/copy-out, not a live alias. Aliased parameters begin with independent values; later parameters win on write-back. Copy-back also occurs on a runtime-error exit. Every explicit parameter type and suffix shorthand converts arguments and assignments. All caller destination conversions are validated before any copy-back write; a conversion failure preserves the caller destinations. Invalid index evaluation prevents entry, while earlier argument side effects remain. These rules apply to SUB, FUNCTION, and DEF calls.

Copy-back follows ordinary assignment scope and type rules. If an argument reads
an inherited value but copy-back creates a local binding, the current DEFtype
applies to that new binding. Failed conversion does not create a local declaration
that would hide the inherited value.

### Array parameter limitation

The parser accepts `SUB P(a() AS NUMERIC)` and calls such as `CALL P(values())`.
The runtime does not associate the array's elements, type, or bounds with `a`.
`values()` is evaluated as an ordinary zero-index expression, and only that scalar
value is bound to the parameter. Array parameters are excluded from write-back.
Renaming a caller's array through an array parameter therefore does not work.

Use a scalar/record argument when possible. Reading an array under its original
name may reach the caller's elements through scope lookup, but that is not array
parameter passing and does not make writes shared. The same restrictions apply to
arrays of records. See [User-Defined Types](user-defined-types.md).

## Scope and shared state

Each call creates a child environment of the **caller**, not just the global
environment. `DIM` and ordinary assignments create local values by default, and
those values disappear after return. However, an unbound read falls through to
the caller and then its ancestors. This dynamic read lookup differs from strict
local/module scoping:

```basic
x = 10
SUB ShowAndChange
    PRINT x                        ' Reads the outer 10
    x = 20                         ' Creates a local value
    PRINT x                        ' 20
END SUB
CALL ShowAndChange
PRINT x                            ' Outer value remains 10
```

`DIM x` inside the procedure initializes a local default, which shadows an outer
value. Constants are visible through parent environments and cannot be shadowed
by a normal assignment. Avoid using parameter names that collide with shared
variables or constants: parameter binding uses the same environment rules.

```text
SHARED name [()] [AS type] [, ...]
DIM SHARED declaration [, declaration ...]
COMMON [SHARED] name [()] [AS type] [, ...]
```

SHARED sends reads and writes of a name to the root environment. SHARED and COMMON accept optional empty array markers and AS types. An explicit AS validates/converts an existing dynamic shared value without resetting it, or initializes a missing value to its declared default. A conflicting existing declaration is rejected; NUMERIC/DOUBLE aliases agree. Without AS, an existing binding keeps its type/value; a missing binding uses suffix/DEFtype defaults. DIM SHARED initializes and declares a shared variable. COMMON and COMMON SHARED have identical single-program sharing behavior; empty array markers do not allocate dimensions or create array parameters.

```basic
DIM SHARED total AS NUMERIC
SUB AddTotal(amount)
    total = total + amount
END SUB
CALL AddTotal(5)
PRINT total                        ' 5
```

Sharing an array's base name routes its element reads and writes to the root
environment. `DIM SHARED a(...)`, `SHARED a`, and `COMMON a()` share element
values and enforce the root array's declared type. A local declaration can
shadow an inherited shared array. Scalar records share as whole values, so
`SHARED state` also permits updating `state.field`.

`OPTION EXPLICIT` recognizes local declarations, parameters, the function-result
name, constants, and shared names. Ordinary undeclared reads that already find a
value in an ancestor can still succeed; declaration checking is not a complete
static scope check. Array bounds/type metadata is interpreter-wide rather than
local to each call, so declaring the same array name in different procedures can
affect later `LBOUND`, `UBOUND`, `MAT`, and primitive/record array defaults. This
includes explicit `AS STRING` and numeric array declarations; returning from a
procedure does not restore a previous same-named array's metadata.

See [Multi-Module Programming](multi-module.md) for the single-source restriction.

## Static storage

`STATIC declaration [, declaration ...]` preserves local scalar, record, and array
values with their declared types between procedure calls. It accepts the same
type and dimension forms as DIM. Initialization uses local storage even when a
parent scope has an identically named value, unless the name is explicitly SHARED.

```basic
SUB Counter
    STATIC count AS INTEGER
    count = count + 1
    PRINT count
END SUB
CALL Counter                       ' 1
CALL Counter                       ' 2
```

`SUB name(...) STATIC` and `FUNCTION name(...) [AS type] STATIC` preserve all local
values and types except parameters and the function result. Explicit STATIC
bindings and array elements remain persistent even when a later invocation skips
the declaration; updates on that invocation are saved too.
Implicit arrays also retain their initialization state, including arrays first
used only in a read, so later DEFtype statements do not change existing bindings.

CLEAR within a procedure resets its current local values to their declared
defaults. It also closes every database connection and frees every query result
in the interpreter, including handles held in caller or shared bindings. It retains static type metadata even when no values remain to save, so a
later call that skips STATIC still enforces the type. The next invocation resumes
from the values saved when the previous call exited. DIM still executes and resets
its scalar/record slot whenever reached, including inside a STATIC procedure.
CLEAR removes implicit array initialization state, allowing a later DEFtype to
apply when an undeclared array is next initialized.

Static state is keyed by procedure name and is retained when a procedure is
redefined in the same interpreter. A fresh interpreter, including REPL RUN,
starts with empty static storage. Values are saved on runtime-error exits too.
Recursive calls load and save snapshots rather than sharing one live static cell.
STATIC arrays retain element values and types, and their declarations record
bounds; array dimension metadata still has interpreter-wide lifetime.

## Function and array resolution

For `name(args)` in an expression, Rice resolves stateful built-ins first, then
registered built-ins, then user functions, then array access. A function or array
cannot override a built-in through this syntax. Undefined function-looking names
may silently act as arrays and return a default value; `OPTION EXPLICIT` catches
some, but suffixes and other declarations can still make them legal array names.

The implementation behavior above is covered by the procedure, sharing, static,
record, and BYREF integration tests in [`tests/integration.rs`](../tests/integration.rs)
and direct runtime checks. Rice does not implement an external-linking ABI.
