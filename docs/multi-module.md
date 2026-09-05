# Multi-Module Programming

Rice executes one source program at a time. It has no BASIC module loader, import
system, linker, `INCLUDE`/`$INCLUDE` processing, or `CHAIN` execution. This applies
to both QBasic mode (the default) and ANSI mode. `CHAIN` is recognized only to
report that it is unsupported. An include-looking directive in an apostrophe or
`REM` comment remains a comment; it does not load another file.

The command-line interpreter accepts one source filename. Its REPL can store and
run a numbered program, but provides no `LOAD`, `SAVE`, or `MERGE` module facility.
See [Quickstart](quickstart.md) for the actual command-line and REPL interfaces.

## COMMON in QBasic mode

```text
COMMON [SHARED] name [()] [AS type] [, name [()] [AS type] ...]
```

Both `COMMON` and `COMMON SHARED` mark the listed names as shared with the root
environment; there is no behavioral difference between them in Rice. The optional
array placeholder must be empty `()`, not dimension bounds. The optional `AS`
type is parsed but does not initialize or enforce a runtime type. Declare and
initialize values with `DIM` separately.

```basic
OPTION DIALECT "QB"
COMMON SHARED total, count
DIM total AS NUMERIC
DIM count AS NUMERIC

SUB Increment
    total = total + 5
    count = count + 1
END SUB

CALL Increment
PRINT total; ","; count             ' 5,1
```

These declarations provide state for procedures within the same interpreter.
They do not transfer values between files or operating-system processes. Named
COMMON blocks, external storage layouts, and cross-program COMMON matching are
not implemented. ANSI mode rejects `COMMON`.

The parser accepts `COMMON values() AS NUMERIC`, but marking an array's base name
shared does not make the runtime's flattened element keys shared. Do not rely on
this form for writes through procedure array access. See
[Procedures and Scope](procedures.md) for this and the array-parameter limitation.

## Organizing a program in either mode

Use `SUB` or `FUNCTION` to separate operations, and
[user-defined types](user-defined-types.md) to group data. Module-level
`DIM SHARED` and procedure-level `SHARED` are available in both modes for scalar
or record state. QBasic mode additionally supports `DEF` functions.

```basic
DIM SHARED total AS NUMERIC

SUB AddAmount(BYVAL amount)
    total = total + amount
END SUB

CALL AddAmount(12)
PRINT total                        ' 12 in either mode
```

Definitions are collected before execution, so separate procedure sections in one
file do not require forward declarations. `DECLARE SUB`/`DECLARE FUNCTION`
are optional syntax annotations, not external imports.

If source is assembled from multiple files by an external build step, Rice sees
only the resulting single source text. Such a build step must resolve filename
ordering, duplicate definitions, labels, and dialect options itself. There is no
language-level namespace boundary between concatenated sections.

## Embedded interpreter and state lifetime

Rust callers can reuse an `Interpreter` and call `run_source` or `run_file`
multiple times. Existing variables, procedure/type definitions, static values,
array metadata, and DATA state can persist. This is an embedding API behavior,
not a BASIC module or `CHAIN` specification. Redefined procedures replace their
bodies, while existing static values remain keyed by name.

A REPL `RUN` creates a fresh interpreter and does not preserve COMMON or static
values from the preceding run. `CLEAR` within a program clears variables in the
current environment and resets the DATA cursor; it does not provide a complete
interpreter/module reset. See [Runtime and Tooling](runtime.md).

Cross-file language semantics, external symbol resolution, and historical
QBasic/ANSI multi-module compatibility are not implemented or claimed.
