# Procedures and Scope

This page specifies the procedures implemented by Rice BASIC. Both the default
QBasic mode and ANSI mode support these forms unless a difference is stated.
The mode names describe Rice's compatibility behavior; they do not imply complete
QBasic 1.1 or ANSI Full BASIC conformance. See [Dialects](dialects.md).

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

| Call form | QBasic mode (default) | ANSI mode |
| --- | --- | --- |
| `CALL P(x, y)` | Calls `SUB P` | Calls `SUB P` |
| `P x, y` | Calls `SUB P` | Calls `SUB P` |
| `CALL P` or `P` | Calls a zero-argument `SUB` | Same |
| `P(x, y)` | Not a parenthesized argument-list form; use `CALL` | Accepted |
| `P (x)` | One parenthesized expression; forces value passing | Accepted as a one-argument call; explicit `BYREF` still writes back |
| `CALL P((x))` | Forces value passing for `x` | Forces value passing for `x` |
| `F(x, y)` in an expression | Calls a function or accesses an array | Same |

With `CALL`, arguments must be inside parentheses: `CALL P x` is not supported.
Without `CALL` in QBasic mode, individual arguments may be parenthesized, as in
`P (x), (y)`, but a comma-separated list cannot be enclosed in one pair.

Zero-argument functions can be called with `F()` or, when no variable named `F`
already resolves, with bare `F`. Use `F()` to avoid the variable ambiguity.
`CALL` resolves only `SUB` definitions; it does not discard a function's result.

Definitions are collected before top-level execution, so calls may precede their
definitions. Definitions inside top-level `IF`, loop, `SELECT CASE`, and exception
blocks are also collected regardless of whether that block executes. A procedure
body is not recursively scanned for nested definitions: nested `SUB`, `FUNCTION`,
`TYPE`, and `DATA` declarations are not a supported way to define local entities.
Repeated procedure names replace earlier definitions. Recursion is supported, but
depth is limited by the host stack; there is no specified portable recursion limit.

## Function result types

`AS type` is accepted on a `FUNCTION`, but the runtime does not enforce it or use
it to initialize the result. At entry, the function-name variable is `""` when the
name ends in `$`, and `0` otherwise. Assignment to that variable determines the
actual returned value. Consequently, an unassigned `FUNCTION Text() AS STRING`
returns numeric `0`, while an unassigned `FUNCTION Text$()` returns `""`.

```basic
FUNCTION Greeting$(who$) AS STRING
    Greeting$ = "Hello, " & who$
END FUNCTION
PRINT Greeting$("world")
```

Use `EXIT FUNCTION` for an early return. `RETURN` belongs to `GOSUB` in QBasic
mode and is not a function-return statement. `END` inside a `SUB` stops the whole
program. A current implementation limitation is that function evaluation discards
non-error control-flow results, so `END` inside a `FUNCTION` does not stop the
caller; do not use it for program termination. Labels and `GOSUB` targets in a
procedure are local to that invocation. See [Error Handling](error-handling.md)
for the limitations of error handlers in procedures.

## DEF functions

QBasic mode accepts single-expression and multiline `DEF` functions. ANSI mode
rejects `DEF`; use `FUNCTION` there.

```basic
OPTION DIALECT "QB"
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
DECLARE FUNCTION name [(parameter, ...)]
```

These declarations are optional and have no runtime effect. They do not validate
the eventual definition, supply missing parameters, or define an external
procedure. A return `AS type` clause after `DECLARE FUNCTION` is **not accepted**,
even though `FUNCTION` itself accepts it. Type clauses on parameters are accepted.

## Parameter passing

| Property | QBasic mode (default) | ANSI mode |
| --- | --- | --- |
| Unqualified parameter | `BYREF` | `BYVAL` |
| Explicit `BYREF` / `BYVAL` | Accepted | Accepted |
| Scalar `AS type` | Parsed, not enforced at runtime | Same |
| Whole record argument | Copied in; copied back for a plain `BYREF` variable | Copied in; copied back only with explicit `BYREF` |
| Array parameter `a()` | Syntax accepted; array binding is not implemented | Same |

Arguments are evaluated left to right in the caller, before entering the procedure.
Each value is copied into a parameter. At exit, parameters marked `BYREF` are
copied back only when the corresponding argument is a **plain variable**.
Parenthesized expressions, arithmetic expressions, array elements, member accesses,
and slices have no write-back destination, even with explicit `BYREF`.

```basic
SUB Change(BYREF x)
    x = 42
END SUB

n = 10
CALL Change(n)
PRINT n                            ' 42 in both modes
n = 10
CALL Change((n))
PRINT n                            ' 10 in both modes

DIM a(1 TO 2)
a(1) = 7
CALL Change(a(1))
PRINT a(1)                         ' 7: array elements are not written back
```

This is copy-in/copy-out, not a live alias. When one caller variable is passed to
two parameters, they start with independent values, and later parameters win on
write-back. Changes are also copied back when a procedure exits with a runtime
error. Parameter `AS` declarations do not coerce values or reject mismatched types.

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
SHARED name [, name ...]
DIM SHARED declaration [, declaration ...]
COMMON [SHARED] name [()] [AS type] [, ...]     QBasic mode only
```

`SHARED` inside a procedure sends reads and writes of that exact name to the root
environment. It accepts plain names only, without `AS` clauses or array
parentheses. Module-level `DIM SHARED` initializes a variable and makes it shared
in subsequent procedure calls. `COMMON` and `COMMON SHARED` are identical in
Rice: both mark names shared, without initializing their values. The optional
`COMMON` type and array placeholders are parsed but have no type/allocation effect.

```basic
DIM SHARED total AS NUMERIC
SUB AddTotal(amount)
    total = total + amount
END SUB
CALL AddTotal(5)
PRINT total                        ' 5 in both modes
```

Sharing an array's base name does not share the flattened element names used by
the current runtime. `DIM SHARED a(...)`, `SHARED a`, and `COMMON a()` do not make
element writes propagate to the caller. Scalar records do share as whole values,
so `SHARED state` also permits updating `state.field`.

In QBasic mode, `OPTION EXPLICIT` recognizes local declarations, parameters, the function-result
name, constants, and shared names. Ordinary undeclared reads that already find a
value in an ancestor can still succeed; declaration checking is not a complete
static scope check. Array bounds/type metadata is interpreter-wide rather than
local to each call, so declaring the same array name in different procedures can
affect later `LBOUND`, `UBOUND`, `MAT`, and record-array initialization.

See [Multi-Module Programming](multi-module.md) for the single-source restriction.

## Static storage

`STATIC declaration [, declaration ...]` preserves named scalar or record values
between calls to a procedure. A declaration may include `AS type`.

```basic
SUB Counter
    STATIC count AS NUMERIC
    count = count + 1
    PRINT count
END SUB
CALL Counter                       ' 1
CALL Counter                       ' 2
```

`SUB name(...) STATIC` and `FUNCTION name(...) [AS type] STATIC` preserve all local
values except parameters and the function result. Static state is keyed by
procedure name, persists through `CLEAR`, and is not discarded when a procedure
is redefined in the same interpreter. A fresh interpreter, including a fresh REPL
`RUN`, starts with empty static storage.

Current limitations:

- A `DIM` statement still runs every call and resets its scalar/record value, even
  in a procedure declared `STATIC`. Prefer `STATIC count` for an explicit counter.
- `STATIC` initializes only when no value resolves, so an identically named outer
  variable can affect initialization. Choose distinct names when that matters.
- `STATIC a(bounds)` is parsed, but it does not create dimension metadata or retain
  individual array elements. A procedure-wide `STATIC` saves local element values,
  but array metadata still has the shared-scope limitation described above.
- Static values are saved even when execution returns a runtime error. Recursive
  calls do not provide a single live shared static cell; each call loads and saves
  a snapshot. Recursive static behavior is not claimed to match QBasic.

## Function and array resolution

For `name(args)` in an expression, Rice resolves stateful built-ins first, then
registered built-ins, then user functions, then array access. A function or array
cannot override a built-in through this syntax. Undefined function-looking names
may silently act as arrays and return a default value; `OPTION EXPLICIT` catches
some, but suffixes and other declarations can still make them legal array names.

The implementation behavior above is covered by the procedure, sharing, static,
record, and BYREF integration tests in [`tests/integration.rs`](../tests/integration.rs)
and direct runtime checks. Full historical ABI, stack-depth, recursive-static, and
external-procedure compatibility remain unverified; no external-linking ABI is
implemented.
