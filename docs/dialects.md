# Dialects and compatibility

Rice has two modes: **QBasic compatibility** (the default, also called QuickBasic or QB in the implementation) and **ANSI** (opt-in). These are Rice language profiles, not a claim of full QBasic 1.1 or ANSI X3.113-1991 conformance. The [language reference](language-reference.md) specifies the implemented language; the guides linked below describe its extensions and limits.

## Selecting a mode

```sh
rice program.bas                       # QBasic default
rice --dialect ansi program.bas
rice --dialect qb program.bas
rice --dialect qbasic program.bas
rice --dialect quickbasic program.bas
rice --compat program.bas              # QBasic
cargo run -- --dialect ansi program.bas
```

CLI mode names are case-insensitive. `--dialect "QBasic 1.1"` is not a CLI alias; use `qb` instead. A source directive can override the CLI setting:

```basic
OPTION DIALECT "ANSI"
```

Accepted source strings, case-insensitively, are `ANSI`, `QB`, `QBASIC`, `QBASIC 1.1`, `QBASIC1.1`, and `QUICKBASIC`. Detection happens before lexing and parsing the full source, including files, stored REPL programs run with `RUN`, and immediate REPL input. An immediate directive also selects the mode for subsequent input.

The **first recognized directive** in source order selects the mode for the entire source, even if it appears after executable statements or in a block that will never execute. It is not a runtime mode switch. Labels, whitespace, and trailing comments are allowed; directive-looking text inside comments or string literals is ignored. An unknown quoted value is currently parsed as a no-op and leaves the current mode unchanged. Put one recognized directive at the beginning of portable examples to make their interpretation explicit.

## Differences implemented by Rice

| Feature | QBasic mode | ANSI mode |
|---|---|---|
| Identifier suffixes | `%`, `&`, `!`, `#`, `$` distinguish names | Only `$` is a type suffix |
| Numeric storage | All scalar numeric types use `f64` in memory | Same |
| Comparisons | True is `-1`, false is `0` | True is `1`, false is `0` |
| Conditions | Zero false; nonzero true | Same |
| `AND`, `OR`, `XOR`, `NOT` | Bitwise operations using signed 64-bit integer casts | Boolean operations on zero/nonzero values |
| String concatenation | String `+` and `&` | String `&`; `+` requires numbers |
| Hex/octal literals | `&H...`, `&O...` | Not recognized as numeric literal syntax |
| Initial `OPTION BASE` | **1 in the current implementation** | 1 |
| Default procedure arguments | `BYREF` | `BYVAL` |
| `GOSUB`, `RETURN`, `ON expression GOTO/GOSUB` | Supported | Rejected |
| `ON ERROR GOTO`, `RESUME`, `ERROR` | Supported with limits below | Rejected |
| `DEFINT`, `DEFLNG`, `DEFSNG`, `DEFDBL`, `DEFSTR`; `DEF FN` | Supported | Rejected |
| `MID$` assignment | Supported | Rejected; use slice assignment |
| `OPTION EXPLICIT`, `COMMON [SHARED]` | Supported | Rejected |
| `FIELD`, `LSET`, `RSET` | Supported | Rejected |
| `ENVIRON` statement; `DATE$ =`, `TIME$ =` | Supported | Rejected |
| `GET`/`PUT` with a variable | Typed binary serialization | Raw text/byte-count operations, described in [file I/O](file-io.md) |

The QBasic initial array base of 1 differs from the usual expectation for older QB programs. Use `OPTION BASE 0` or explicit lower bounds where needed. Suffixes do not implement QB integer overflow, single-precision arithmetic, or different numeric storage classes; `A%`, `A!`, and `A#` are distinct `f64` variables. QB bitwise operands are truncated toward zero by integer casts, rather than rounded like `CINT`; out-of-range casts saturate and NaN casts to zero. There is no short-circuit evaluation in either mode: both operands of a binary operator are evaluated.

`BYREF` operates through the interpreter's procedure argument/copy-back rules, not a general memory reference model. In QB, a parenthesized expression in an unparenthesized call, such as `ChangeMe (x)`, forces value passing. See [procedures](procedures.md) for argument forms and limits.

## Features shared by both profiles

Unless a row above explicitly rejects a construct, do not infer that its historical origin restricts its use. Both modes accept:

- Structured control flow, numeric/named labels and `GOTO`, `SUB`/`FUNCTION`, `DIM`, `CONST`, `SHARED`, `STATIC`, and user-defined records.
- `WHEN EXCEPTION IN ... USE ... END WHEN`, `RETRY`, and `CONTINUE`; `EXTYPE`/`EXTEXT$` provide structured exception information. `ERR` and `ERL` can be read in either mode, though only QB installs classic handlers.
- ANSI-style string slicing, MAT operations, and `PRINT USING`.
- **Both** `OPEN #n: NAME ...` and `OPEN path$ FOR ... AS #n` syntaxes. The parser does not restrict these by dialect. `INPUT`, `OUTPUT`, `APPEND`, `BINARY`, and `RANDOM` mode words are accepted by the latter syntax in either profile, but their record and `GET`/`PUT` behavior differs.
- `SEEK`, `SET #n: POINTER`, `ASK #n: POINTER`, text file I/O, filesystem statements, and text console control.
- The shared builtin registry, including QBasic-style string function names, `INPUT$`, `MKI$`/`MKL$`/`MKS$`/`MKD$`, `CVI`/`CVL`/`CVS`/`CVD`, and `ENVIRON$` reads. See the complete [builtin catalog](builtins.md).

Identifiers are case-insensitive; string contents are case-sensitive. Undefined values initialize to `0` or `""` unless declaration enforcement applies. `=` assigns at statement level and compares inside expressions. `MOD` is `a - b * FLOOR(a / b)` in both profiles, including real operands and negative values. Plain `PRINT` adds neither leading nor trailing spaces to numbers and uses 16-column comma zones. Numeric and string operands do not implicitly convert for arithmetic or concatenation.

## Compatibility limits and open questions

- This interpreter has no graphics, audio synthesis, event-driven `ON TIMER`/`ON KEY`, `TIMER ON/OFF/STOP`, `KEY ON/OFF/STOP`, `CHAIN`, or compiled execution. `BEEP` only emits a terminal bell. `SCREEN()` reads a simulated text buffer; it is not a graphics mode statement.
- Classic error handlers are reliable for supported top-level statement flows. Errors escaping nested constructs are associated with the enclosing statement, so `RESUME` does not reconstruct the inner execution point. Handler scope is interpreter-wide, not independently stacked for each procedure. See [error handling](error-handling.md).
- Line numbers on block terminators such as `NEXT` and `END IF` are not accepted; leave terminators unnumbered.
- `END` inside a function evaluated as an expression does not currently terminate its caller. End the program from the calling statement.
- Binary scalar widths use suffixes, rather than retaining every `AS` declaration. An unsuffixed `DIM n AS INTEGER` uses default numeric binary storage; use `n%` or an `INTEGER` field inside a `TYPE`. Ordinary fixed-length string declarations do not enforce storage length in memory.
- Text files use UTF-8, while QB binary strings use one byte per character in U+0000–U+00FF; characters above U+00FF clamp to byte 255. `INPUT$` performs lossy UTF-8 decoding and is not a substitute for arbitrary packed-byte reads. See [file I/O](file-io.md).
- Console behavior is a limited terminal emulation: no tracked wrapping/scrolling, no color attributes in `SCREEN`, and no physical resize from `WIDTH`. See [console](console.md).
- `END` and `STOP` end a running program; neither implements a resumable debugger break. `Ctrl+D` exits at the REPL prompt. `Ctrl+C` during execution may terminate the host process rather than return to `Ok`.
- Complete external conformance, interoperability with every historical QB binary file layout, and all host terminal/Windows behaviors have not been established. A construct being accepted does not establish standard conformance. Malformed-format edge cases and Unicode display widths are also not compatibility guarantees; see [PRINT USING](print-using.md).
