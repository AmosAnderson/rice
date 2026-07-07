# Dialects

RICE BASIC has two dialect modes:

- **QBasic 1.1 compatibility mode** is the default. It accepts a focused set of QuickBasic/QBasic syntax and semantics for older programs.
- **ANSI mode** follows ANSI X3.113-1991 Full BASIC where the current interpreter supports it.

Both modes use the same runtime value model: numbers are stored as `f64`, strings are stored as variable-length strings, identifiers are case-insensitive, and undefined variables auto-initialize to `0` or `""`.

## Selecting a Dialect

QBasic 1.1 compatibility mode is used unless ANSI mode is requested:

```bash
rice --dialect ansi program.bas
```

Inside a source file, use:

```basic
OPTION DIALECT "ANSI"
```

The default QBasic-compatible mode can also be requested explicitly:

```bash
rice --dialect qb program.bas
rice --dialect qbasic program.bas
rice --compat program.bas
cargo run -- --dialect qb program.bas
cargo run -- --compat program.bas
```

or in a source file:

```basic
OPTION DIALECT "QB"
```

`OPTION DIALECT "QBASIC"`, `OPTION DIALECT "QBasic 1.1"`, and `OPTION DIALECT "QUICKBASIC"` are also accepted. Source-level dialect detection is applied when running a complete file or a stored REPL program with `RUN`.

## ANSI Mode

ANSI mode is opt-in and remains the reference for ANSI-specific behavior.

| Area | ANSI behavior |
|------|---------------|
| Variable names | `$` marks string variables; numeric suffixes such as `%`, `!`, `#`, and `&` are not supported. |
| Value types | `NUMERIC` and `STRING`; all numeric values are stored as `f64`. |
| Truth values | Comparisons return `1` for true and `0` for false. |
| String concatenation | Use `&`; `+` is arithmetic. |
| Logical operators | `AND`, `OR`, `NOT`, and `XOR` operate on truth values, not bits. |
| Arrays | `OPTION BASE` defaults to `1`. |
| Procedure parameters | Parameters are `BYVAL` by default; `BYREF` can be requested explicitly. |
| Subroutine jumps | `GOSUB`/`RETURN` and `ON ... GOTO`/`ON ... GOSUB` are not supported. |
| File I/O | ANSI `OPEN #n: NAME file$, ACCESS mode, ORGANIZATION org` syntax. |

ANSI mode supports structured control flow, `SUB` and `FUNCTION`, `WHEN EXCEPTION IN ... USE ... END WHEN`, ANSI string slicing with `s$(start:end)`, MAT operations, text console control, and ANSI file pointer operations.

## QBasic 1.1 Compatibility Mode

QBasic/QuickBasic mode is intended for compatibility with common QuickBasic/QBasic program shapes. It is not a complete clone of the QuickBasic runtime, compiler, or memory model.

| Area | QuickBasic mode behavior |
|------|--------------------------|
| Variable names | Type suffixes `%`, `!`, `#`, `&`, and `$` are accepted as part of variable names, so `X%`, `X!`, and `X#` are distinct variables. Numeric suffixes still store `f64` values internally. |
| Truth values | Comparisons return `-1` for true and `0` for false. |
| String concatenation | `+` concatenates strings; `&` also concatenates strings. |
| Logical operators | `AND`, `OR`, `NOT`, and `XOR` perform bitwise operations on numeric values. |
| Literals | Hexadecimal `&H...` and octal `&O...` numeric literals are accepted. |
| Procedure parameters | Parameters are `BYREF` by default. `BYVAL` can be requested explicitly. Passing a parenthesized argument to an unparenthesized call, such as `ChangeMe (x)`, evaluates it as an expression and passes it by value. |
| Subroutine jumps | `GOSUB`/`RETURN`, `ON ... GOTO`, and `ON ... GOSUB` are supported. |
| Classic error handling | `ON ERROR GOTO`, `RESUME`, `RESUME NEXT`, `RESUME label`, `ERROR`, `ERR`, and `ERL` are supported for top-level handlers. |
| File I/O | QuickBasic-style `OPEN file$ FOR mode AS #n` is supported for `INPUT`, `OUTPUT`, `APPEND`, `BINARY`, and `RANDOM`. |

QuickBasic mode also supports `GET` and `PUT` for structured binary record I/O, including recursive serialization of user-defined type fields. `FIELD`, `LSET`, and `RSET` support classic random-access record buffers. `MKI$`/`MKL$`/`MKS$`/`MKD$` and `CVI`/`CVL`/`CVS`/`CVD` convert between numbers and packed binary strings.

`COMMON` is supported as a single-source shared-variable declaration. `CHAIN` is not supported.

Classic `ON ERROR` handling is exact for top-level statements. Errors that escape from deeply nested blocks can transfer to the handler, but `RESUME` targets are limited by the interpreter's current block-level control-flow model.

## Shared Semantics

These behaviors are the same in both modes unless noted above:

- Identifiers are normalized case-insensitively.
- Undefined numeric variables read as `0`; undefined string variables read as `""`.
- Statement-level `=` performs assignment; expression-level `=` performs comparison.
- `MOD` works on real numbers.
- `PRINT` does not add a leading space before positive numbers and uses 16-character comma zones.
- Arrays are stored with flattened keys internally.
- Graphics and sound APIs are not implemented; RICE BASIC is text-mode only.

## Not Implemented

The following common BASIC-family features are not currently implemented or are intentionally limited:

- `CHAIN`.
- `TIMER ON`/`OFF`/`STOP` and `KEY ON`/`OFF`/`STOP`.
- Full QuickBasic integer, long, single, and double storage semantics; suffixes distinguish variables but numeric values are stored as `f64`.
- Nested-block `RESUME` behavior for classic `ON ERROR` handlers is limited; top-level handlers are supported.
- A graceful REPL break that returns to `Ok` while a program is running. `END` and `STOP` end the BASIC program normally; `Ctrl+D` exits at the REPL prompt; `Ctrl+C` while a program runs may terminate the host process.
