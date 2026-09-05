# Built-in functions

This is the function reference for Rice BASIC 0.14.0. Unless stated otherwise, functions are available in **both** QBasic and ANSI modes. Names such as “ANSI math additions” in the source do not restrict a function to that dialect. The [language reference](language-reference.md) defines expression operators and the [dialect guide](dialects.md) defines truth values.

## Calling conventions and types

Arguments are evaluated left to right. Numeric arguments must be numeric values; strings are not implicitly parsed as numbers. String arguments must be strings. Use `VAL` and `STR$` for explicit conversion. Counts, positions, and dimensions generally truncate toward zero through a checked signed 64-bit conversion; NaN, infinity, and out-of-range values raise overflow. Exceptions using direct casts are called out below.

| Form | Meaning |
|---|---|
| `ABS(x)`, `MID$(s$, start[, count])` | Parentheses required for functions with arguments; brackets in this reference mean optional syntax. |
| `PI()`, `MAXNUM()`, `RND()`, `CURDIR$()`, `COMMAND$()` | Parentheses required for these zero-argument calls. Bare names are ordinary variables and default to `0` or `""`. |
| `TIMER`, `FREEFILE` | Bare keyword forms only; `TIMER()` and `FREEFILE()` are parse errors. |
| `DATE$`, `TIME$`, `CSRLIN`, `INKEY$`, `DET`, `ERR`, `ERL`, `EXTYPE`, `EXTEXT$` | Bare or parenthesized zero-argument forms. An existing variable/constant of the same name can shadow a bare reference; parentheses force function lookup. DATE$/TIME$ assignment is special. |

Runtime dispatch checks special interpreter functions, the builtin registry, user functions, then arrays. An unrecognized `name(...)` may therefore read an implicit array element rather than report an unknown function. Builtins take precedence over user functions of the same name. `LBOUND` and `UBOUND` examine their first argument as an array name rather than evaluating it.

The registry also tries adding `$` to an unmatched name, so `LEFT(...)` aliases `LEFT$(...)`, for example. Prefer canonical names: interpreter-specific overrides are dispatched by exact name, so `ENVIRON(...)`, `DATE()`, and `TIME()` bypass the overrides used by `ENVIRON$(...)`, `DATE$()`, and `TIME$()`. Missing or surplus arguments normally cause an arity error; `EXTYPE(...)` and `EXTEXT$(...)` currently evaluate but ignore supplied arguments.

## Mathematics and numeric conversion

All numeric results are stored as `f64`, including integer conversion results. Trigonometric angles are in radians.

| Function | Result and restrictions |
|---|---|
| `ABS(x)` | Absolute value. |
| `INT(x)` | Floor; `INT(-1.2)` is `-2`. |
| `FIX(x)` | Truncation toward zero; `FIX(-1.2)` is `-1`. |
| `SGN(x)` | `-1`, `0`, or `1` according to sign; independent of dialect truth values. NaN currently returns `0`. |
| `SQR(x)` | Square root; negative inputs raise illegal function call. |
| `SIN(x)`, `COS(x)`, `TAN(x)` | Sine, cosine, tangent. |
| `ATN(x)` | Arctangent. |
| `ASIN(x)`, `ACOS(x)` | Inverse sine/cosine; input must be in `[-1, 1]`. |
| `COT(x)`, `CSC(x)`, `SEC(x)` | Reciprocals of tangent, sine, cosine. Division by zero is checked using exact floating-point equality; a mathematically singular angle may instead yield a large finite result. |
| `ANGLE(x, y)` | `atan2(y, x)`, the angle of the vector `(x, y)`. |
| `EXP(x)` | Exponential with base e. |
| `LOG(x)` | Natural logarithm; inputs `<= 0` raise illegal function call. |
| `CEIL(x)` | Ceiling. |
| `ROUND(x[, places])` | Nearest value, ties away from zero. Default `places` is zero; explicitly supplied places truncate to an integer and must be `0..308`. Negative places are unsupported. |
| `TRUNCATE(x[, places])` | Truncation toward zero at the requested decimal places; same places rules as `ROUND`. |
| `REMAINDER(x, y)` | Floating-point remainder using a quotient truncated toward zero; sign follows the dividend. Unlike MOD for negative operands: `REMAINDER(-5, 3)` is `-2`, while `-5 MOD 3` is `1`. Zero divisor raises division by zero. |
| `CINT(x)` | Half-to-even rounding; original input must be within `-32768..32767` **before** rounding. |
| `CLNG(x)` | Half-to-even rounding; original input must be within `-2147483648..2147483647` before rounding. |
| `CSNG(x)` | Converts through IEEE single precision (`f32`), then stores the result as `f64`. |
| `CDBL(x)` | Returns the numeric value unchanged. |
| `MAXNUM()` | Largest finite `f64` value. |
| `PI()` | The `f64` approximation of pi. |

`ROUND` and `TRUNCATE` use binary floating-point scaling. If scaling overflows, they return the original value. Arithmetic and many math functions can produce infinity or NaN rather than a BASIC overflow exception; this is not a complete historical numeric error model. `CINT`/`CLNG` differ from the truncation used for most indices. For example, `ROUND(2.5)` is `3`, but `CINT(2.5)` is `2`.

## Random numbers

| Form | Behavior |
|---|---|
| `RND()` or `RND(positive)` | Advances the generator and returns a number in `[0, 1)`. The positive argument's magnitude is ignored. |
| `RND(0)` | Returns the last generated value, initially `0`. |
| `RND(negative)` | Reseeds from that number's `f64` bit pattern and returns the first generated value; repeating the same negative value repeats the result. |
| `RANDOMIZE` or `RANDOMIZE TIMER` | Seeds from the system clock nanoseconds; the TIMER spelling is a special parser case, not evaluation of local seconds since midnight. |
| `RANDOMIZE expression` | Seeds from the numeric expression's `f64` bit pattern. `RANDOMIZE (TIMER)` evaluates TIMER as an expression, unlike the bare TIMER special case. |

The implementation uses a 64-bit linear congruential generator. It does not reproduce QBasic's random sequence. `RANDOMIZE` does not change the cached value returned by `RND(0)` until another value is generated. The initial generator state is clock-seeded; use an explicit seed for reproducibility.

## Strings

Text uses Unicode strings. `LEN`, positions, substrings, and most string counts use Unicode scalar values, not UTF-8 bytes, grapheme clusters, or terminal display columns. `CHR$` and packed binary functions have separate byte rules.

| Function | Result and restrictions |
|---|---|
| `LEN(s$)` | Number of Unicode scalar values; does not accept a numeric value or UDT record. |
| `LEFT$(s$, count)` | First `count` characters, clipped at the string end; count must be nonnegative. |
| `RIGHT$(s$, count)` | Last `count` characters, clipped to the string length; count must be nonnegative. |
| `MID$(s$, start[, count])` | Substring beginning at 1-based `start`; omitted count means through the end. Start must be at least 1, count nonnegative. Starting past the end returns `""`. See [slice and MID$ assignment](string-slicing.md). |
| `INSTR(s$, needle$)` | 1-based first match, or `0` if absent. Empty needle returns `1`, including in an empty string. |
| `INSTR(start, s$, needle$)` | Searches at or after 1-based start; start must be at least 1. Starting past the last character returns `0`, even for an empty needle. |
| `LTRIM$(s$)`, `RTRIM$(s$)` | Remove leading/trailing Unicode whitespace, including tabs and newlines, not just spaces. |
| `UCASE$(s$)`, `LCASE$(s$)` | Unicode case conversion; can change the character count. |
| `SPACE$(count)` | Repeats a space; nonnegative count. |
| `STRING$(count, character)` | Repeats the first character of a string, or a numeric byte code. Empty string means a space. Numeric character is converted to a checked integer, then wraps modulo 256; count must be nonnegative. |
| `CHR$(code)` | Character U+0000..U+00FF; integer-converted code must be `0..255`. |
| `ASC(s$)` | Unicode scalar value of the first character, possibly above 255. Empty input raises illegal function call. |
| `STR$(x)` | Numeric text with no leading/trailing positive-number space. Uses integer text for integral magnitudes below `1e15`, otherwise Rust floating-point formatting. |
| `VAL(s$)` | Trims surrounding whitespace; parses a number, or a leading decimal prefix, or leading `&H` hex/`&O` octal digits. Empty/invalid input returns `0`. |
| `HEX$(x)`, `OCT$(x)` | Uppercase hexadecimal / octal text after direct truncating signed 64-bit cast. Out-of-range inputs saturate and NaN becomes zero. Negative values use unsigned 64-bit two's-complement representation; this is not QBasic's 16/32-bit formatting. |

`VAL` first tries to parse the whole decimal string, including an exponent. Its fallback prefix scan accepts digits, decimal points, and an initial sign, but not an exponent: `VAL("1E2")` is `100`, while `VAL("1E2tail")` is `1`. Hex/octal overflow returns `0`; a leading minus before `&H`/`&O` is not supported. Unlike source literals, VAL does not recognize D exponents (`VAL("1D2")` is `1`) and uses signed `i64` for radix conversion (`VAL("&HFFFFFFFFFFFFFFFF")` is `0`). Whole-string host float parsing also accepts `"NaN"` and `"inf"`. This is permissive parsing, not a full QBasic numeric-literal parser.

Large string counts can exhaust host memory; there is no documented BASIC string-size limit. Unicode behavior is not a DOS code-page emulation. See [compatibility notes](compatibility.md).

## Packed binary conversion

These functions work in both modes. Binary strings map each byte to a character U+0000..U+00FF. On conversion back, characters above U+00FF become byte 255. Use binary/record I/O for these strings; UTF-8 text I/O does not preserve their byte representation.

| Encode | Decode | Layout |
|---|---|---|
| `MKI$(x)` | `CVI(s$)` | 2-byte little-endian signed integer. |
| `MKL$(x)` | `CVL(s$)` | 4-byte little-endian signed integer. |
| `MKS$(x)` | `CVS(s$)` | 4-byte little-endian IEEE single precision. |
| `MKD$(x)` | `CVD(s$)` | 8-byte little-endian IEEE double precision. |

`MKI$`/`MKL$` use direct Rust integer casts: fractional values truncate; out-of-range values saturate; NaN becomes zero. They do not call `CINT`/`CLNG`. Decoders require at least the listed byte count and ignore extra bytes. IEEE encodings are not Microsoft Binary Format; there are no `MKSMBF$`, `MKDMBF$`, `CVSMBF`, or `CVDMBF` functions. See [binary record layouts](file-io.md).

## Interpreter state, files, arrays, and console

| Function | Behavior / detailed reference |
|---|---|
| `LBOUND(array[, dimension])`, `UBOUND(array[, dimension])` | Bounds recorded by DIM/REDIM. Dimension defaults to 1; invalid dimension or undimensioned array raises illegal function call. Bare `array`, `array()`, and an indexed form are accepted as the name; indices in that first argument are not evaluated. Bounds do not enforce ordinary array access. See [arrays](language-reference.md). |
| `FREEFILE` | Lowest unused handle in `1..255`; returns `0` if none is free. |
| `EOF(handle)` | End-of-file status using dialect truth values. |
| `LOF(handle)` | File length in bytes. |
| `LOC(handle)` | Current zero-based byte offset in every mode, including RANDOM; prefers the reader if present, otherwise the writer, and returns `0` on position-query failure. |
| `SEEK(handle)` | One-based byte position of the next operation, including RANDOM files. See [file I/O](file-io.md) for all file functions. |
| `CSRLIN` or `CSRLIN()` | Tracked 1-based console row. |
| `POS(argument)` | Tracked 1-based column; the argument is evaluated but ignored. |
| `INKEY$` or `INKEY$()` | Nonblocking key input, empty string if no key; extended keys have a NUL prefix. |
| `INPUT$(count[, handle])` | Reads up to the positive integer count of bytes from console or file, stopping on EOF/read error; decodes as lossy UTF-8. `#` before the file argument is optional. See [console](console.md) and [file I/O](file-io.md) for byte and EOF behavior. |
| `SCREEN(row, column[, flag])` | Character byte from Rice's screen buffer; the optional flag is evaluated but ignored, so attributes are not returned. |
| `DET` or `DET()` | Determinant saved by the latest successful `MAT ... = INV(...)`; initially zero. It is not `DET(matrix)`. See [MAT](mat-operations.md). |
| `ERR`, `ERL` (or zero-argument calls) | Last classic error code and numbered BASIC line, initially zero. Queries exist in both modes, although classic handler statements require QB. |
| `EXTYPE`, `EXTEXT$` (or zero-argument calls) | Structured exception code and description, initially zero/empty and cleared on normal handler completion. Early control transfers can leave stale state. See [errors](error-handling.md). |

`TAB(column)` and `SPC(count)` are **PRINT items**, not general expression functions. See [console output](console.md).

## Clock and host environment

| Function | Behavior |
|---|---|
| `TIMER` | Local seconds since midnight. Windows includes milliseconds; the non-Windows implementation has whole-second resolution. |
| `DATE$` or `DATE$()` | Local date as `MM-DD-YYYY`, unless overridden by assignment. |
| `TIME$` or `TIME$()` | Local time as `HH:MM:SS`, unless overridden by assignment. |
| `ENVIRON$(name$)` | Interpreter override, if present, otherwise host environment value; absent/unreadable value gives `""`. Numeric environment-index lookup is unsupported. Names are case-sensitive on Unix; override names are uppercased on Windows. |
| `CURDIR$()` | Host process working directory; `""` if it cannot be obtained. No drive argument. |
| `COMMAND$()` | Joins raw host process arguments starting at index 2 with spaces. The CLI accepts no extra BASIC program arguments, so this may be empty or include dialect options/source path depending on argument order. It is not a reliable program-argument API. |

**QB-only setters:** `DATE$ = expression` accepts `MM-DD-YYYY` or `MM/DD/YYYY` after trimming for validation. It checks month `1..12`, day `1..31`, and year `0..9999`, but not month lengths or leap years. `TIME$ = expression` checks `HH:MM:SS`, hours `0..23`, minutes/seconds `0..59`. Both store the original string, including surrounding whitespace; they override subsequent reads without changing the OS clock. The time override does not tick and does not change `TIMER`.

The **QB-only** `ENVIRON "name=value"` statement stores an interpreter-local override, inherited by commands started through `SHELL`. Empty names, missing `=`, or NUL characters are errors; empty values are allowed. It does not modify the parent process environment. See [runtime and host statements](runtime.md).

## Implementation sources

The registry and pure functions are in [`src/builtins.rs`](../src/builtins.rs); stateful dispatch is in [`src/interpreter.rs`](../src/interpreter.rs), expression spelling in [`src/parser.rs`](../src/parser.rs), and integer/binary conversions in [`src/value.rs`](../src/value.rs). These implementation rules describe Rice; compatibility with external dialects beyond the tested cases remains unverified.
