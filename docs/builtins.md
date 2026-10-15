# Built-in functions

This is the function reference for Rice BASIC 0.20.0. The [language reference](language-reference.md) defines expression operators and truth values.

## Calling conventions and types

Arguments are evaluated left to right. Numeric arguments must be numeric values; strings are not implicitly parsed as numbers. String arguments must be strings. Use `VAL` and `STR$` for legacy conversion; `CINT64`, `CBOOL`, `CBYTE`, and explicit byte encoders handle native types. Counts, positions, and dimensions generally truncate toward zero through a checked signed 64-bit conversion; NaN, infinity, and out-of-range values raise overflow. Native byte-buffer positions/counts require exact integers instead. Exceptions using direct casts are called out below.

| Form | Meaning |
|---|---|
| `ABS(x)`, `MID$(s$, start[, count])` | Parentheses required for functions with arguments; brackets in this reference mean optional syntax. |
| `PI()`, `MAXNUM()`, `RND()`, `CURDIR$()`, `COMMAND$()` | Parentheses required for these zero-argument calls. Bare names are ordinary variables and default to `0` or `""`. |
| `TIMER`, `FREEFILE` | Bare keyword forms only; `TIMER()` and `FREEFILE()` are parse errors. |
| `DATE$`, `TIME$`, `CSRLIN`, `INKEY$`, `DET`, `ERR`, `ERL`, `EXTYPE`, `EXTEXT$`, `TRUE`, `FALSE` | Bare or parenthesized zero-argument forms. An existing variable/constant of the same name can shadow a bare reference; parentheses force function lookup. DATE$/TIME$ assignment is special. |

Runtime dispatch checks special interpreter functions, the builtin registry, user functions, then arrays. An unrecognized `name(...)` may therefore read an implicit array element rather than report an unknown function. Builtins take precedence over user functions of the same name. `LBOUND`, `UBOUND`, and `DET(matrix)` examine their first argument as an array name rather than evaluating its value; DET requires a bare name or empty array parentheses.

The registry also tries adding `$` to an unmatched name, so `LEFT(...)` aliases `LEFT$(...)`, for example. Prefer canonical names: interpreter-specific overrides are dispatched by exact name, so `ENVIRON(...)`, `DATE()`, and `TIME()` bypass the overrides used by `ENVIRON$(...)`, `DATE$()`, and `TIME$()`. Missing or surplus arguments normally cause an arity error; `EXTYPE(...)` and `EXTEXT$(...)` currently evaluate but ignore supplied arguments.

## Mathematics and numeric conversion

Numeric functions can return NUMERIC/DOUBLE f64, INTEGER i16, LONG i32, SINGLE f32, or exact INT64/BYTE values. CINT, CLNG, and CSNG return their corresponding storage types. Native `CINT64` and `CBYTE` preserve exact integer representations. Legacy math functions generally require an exactly representable floating-point input; use `CDBL` explicitly when rounding a large integer is intended. Trigonometric angles are in radians.

| Function | Result and restrictions |
|---|---|
| `ABS(x)` | Absolute value; preserves INTEGER/LONG/INT64/BYTE. A signed integer minimum raises overflow. |
| `INT(x)` | Floor; `INT(-1.2)` is `-2`. Integer input is unchanged; SINGLE input retains SINGLE. |
| `FIX(x)` | Truncation toward zero; `FIX(-1.2)` is `-1`. Integer input is unchanged; SINGLE input retains SINGLE. |
| `SGN(x)` | `-1`, `0`, or `1` according to sign; independent of numeric truth values. INT64/BYTE input produces INT64; legacy input produces NUMERIC. NaN returns `0`. |
| `SQR(x)` | Square root; negative inputs raise illegal function call. |
| `SIN(x)`, `COS(x)`, `TAN(x)` | Sine, cosine, tangent. |
| `ATN(x)` | Arctangent. |
| `ASIN(x)`, `ACOS(x)` | Inverse sine/cosine; input must be in `[-1, 1]`. |
| `COT(x)`, `CSC(x)`, `SEC(x)` | Reciprocals of tangent, sine, cosine. Division by zero is checked using exact floating-point equality; a mathematically singular angle may instead yield a large finite result. |
| `ANGLE(x, y)` | `atan2(y, x)`, the angle of the vector `(x, y)`. |
| `EXP(x)` | Exponential with base e. |
| `LOG(x)` | Natural logarithm; inputs `<= 0` raise illegal function call. |
| `CEIL(x)` | Ceiling. |
| `ROUND(x[, places])` | Nearest value, ties away from zero. Default `places` is zero; explicitly supplied places truncate to an integer and must be `-308..308`. Negative places round to tens, hundreds, and larger powers of ten. |
| `TRUNCATE(x[, places])` | Truncation toward zero at the requested decimal places; same places rules as `ROUND`. |
| `REMAINDER(x, y)` | Floating-point remainder using a quotient truncated toward zero; sign follows the dividend. Unlike MOD for negative operands: `REMAINDER(-5, 3)` is `-2`, while `-5 MOD 3` is `1`. Zero divisor raises division by zero. |
| `MIN(x[, y, ...])`, `MAX(x[, y, ...])` | Smallest / largest of one or more numeric arguments. The selected argument retains its numeric type, with exact mixed integer/float comparisons. A single argument is returned unchanged; no arguments raises an arity error. Strings, BOOL, BYTES, and records raise a type mismatch. Infinities compare normally; any NaN produces NaN, while remaining arguments are still type-checked. |
| `CINT(x)` | Returns INTEGER after half-to-even rounding and a -32768..32767 range check on the rounded value. BOOL explicitly converts to 1/0. |
| `CLNG(x)` | Returns LONG after half-to-even rounding and a -2147483648..2147483647 range check on the rounded value. BOOL explicitly converts to 1/0. |
| `CSNG(x)` | Returns SINGLE f32, rounding numeric input. Finite overflow raises an error; existing NaN/infinity remain nonfinite. BOOL converts explicitly to 1/0. |
| `CDBL(x)` | Explicit `f64` conversion; may round INT64. Converts native BOOL to `1` or `0`. Existing NUMERIC input is unchanged. |
| `MAXNUM()` | Largest finite `f64` value. |
| `PI()` | The `f64` approximation of pi. |

`ROUND` and `TRUNCATE` use binary floating-point scaling. With nonnegative places, scaling overflow returns the original value. Negative places divide by the corresponding decimal power, round or truncate, then multiply back: `ROUND(150, -2)` is `200`, `ROUND(-150, -2)` is `-200`, and `TRUNCATE(-1499, -2)` is `-1400`. A finite input whose negative-place result exceeds the numeric range raises overflow (BASIC error 6). NaN and infinity inputs remain unchanged, but invalid places still raise an error. Fractional places truncate toward zero, so `-1.9` selects tens and `-0.9` selects units. These functions provide binary floating-point results, not exact decimal arithmetic.

Arithmetic and many math functions can produce infinity or NaN rather than a BASIC overflow exception. `CINT`/`CLNG` differ from the truncation used for most indices. For example, `ROUND(2.5)` is `3`, but `CINT(2.5)` is `2`.

`MIN` and `MAX` accept individual numeric expressions, including indexed array elements and nested calls; they do not reduce a whole array. Arguments are evaluated eagerly from left to right, following normal function-call rules.

```basic
PRINT MIN(7, -2, 4.5)        ' -2
PRINT MAX(-8, -3, -12)       ' -3
PRINT MIN(10, MAX(0, 12))    ' 10 (clamp 12 to the range 0..10)
```

## Native conversions and byte buffers

See [Native data
types](data-types.md) for declaration, arithmetic, precision, and file rules.

| Function | Result and restrictions |
|---|---|
| `CINT64(value)` | Exact signed 64-bit integer. Accepts an integral numeric value, BOOL (`1`/`0`), or a trimmed signed decimal integer string. Fractions, nonfinite numbers, and out-of-range values fail. Use a string to preserve large integer digits. |
| `CBOOL(value)` | Native BOOL from a boolean or finite number; zero is false, nonzero true. Strings and byte buffers fail. |
| `CBYTE(value)` | Native BYTE from an exact integer in `0..255`, or BOOL (`1`/`0`). |
| `TRUE()`, `FALSE()` | Native BOOL constants; bare forms work when not shadowed by a variable or constant. |
| `TYPEOF$(value)` | `NUMERIC`, `INTEGER`, `LONG`, `SINGLE`, `INT64`, `BOOL`, `BYTE`, `BYTES`, `STRING`, or the record type name. DOUBLE reports NUMERIC; fixed strings report STRING. |
| `BYTES([value, ...])` | New byte buffer. Each value uses CBYTE conversion; no arguments creates an empty buffer. |
| `BYTEBUFFER(length[, fill])` | New buffer with an exact nonnegative length; optional byte fill defaults to zero. |
| `BYTELEN(buffer)` | Byte count, also available through LEN(buffer). |
| `BYTEAT(buffer, position)` | Native BYTE at a 1-based position. |
| `BYTESET(buffer, position, value)` | New copy with one byte replaced; the original is unchanged. |
| `BYTESLICE(buffer, start[, count])` | New slice; omitted count reaches the end and oversized count is clipped. Start may equal length plus one for an empty result. |
| `UTF8ENCODE(text$)` | BYTES containing the text's UTF-8 encoding. |
| `UTF8DECODE$(buffer)` | STRING decoded from valid UTF-8; invalid bytes raise an error. |
| `HEXENCODE$(buffer)` | Uppercase hexadecimal STRING without a prefix. |
| `HEXDECODE(text$)` | BYTES decoded from an even number of hexadecimal digits; either case is accepted, prefixes/whitespace/separators are rejected. |

Byte positions are independent of OPTION BASE and must be exact integers.
BYTEAT/BYTESET reject positions outside existing elements. BYTESLICE rejects
negative counts and starts outside `1..LEN(buffer)+1`. Buffers are distinct from
text and from the older MK*/CV* packed-string representation.

ABS, INT, and FIX preserve native integer representations; ABS of the minimum
INT64 raises overflow. SGN of INT64/BYTE returns INT64. MIN/MAX compare mixed
numeric values exactly and retain the selected value's type. STR$ preserves all
INT64 digits and formats BOOL as `TRUE`/`FALSE`; HEX$/OCT$ preserve native integer
bits. Other floating-point math functions may require explicit CDBL when an
integer cannot be represented exactly.

## Random numbers

| Form | Behavior |
|---|---|
| `RND()` or `RND(positive)` | Advances the generator and returns a number in `[0, 1)`. The positive argument's magnitude is ignored. |
| `RND(0)` | Returns the last generated value, initially `0`. |
| `RND(negative)` | Reseeds from that number's `f64` bit pattern and returns the first generated value; repeating the same negative value repeats the result. |
| `RANDOMIZE` or `RANDOMIZE TIMER` | Seeds from the system clock nanoseconds; the TIMER spelling is a special parser case, not evaluation of local seconds since midnight. |
| `RANDOMIZE expression` | Seeds from the numeric expression's `f64` bit pattern. `RANDOMIZE (TIMER)` evaluates TIMER as an expression, unlike the bare TIMER special case. |

The implementation uses a 64-bit linear congruential generator. `RANDOMIZE` does not change the cached value returned by `RND(0)` until another value is generated. The initial generator state is clock-seeded; use an explicit seed for reproducibility.

## Strings

Text uses Unicode strings. `LEN(string)`, positions, substrings, and most string counts use Unicode scalar values, not UTF-8 bytes, grapheme clusters, or terminal display columns. `LEN(record)`, `CHR$`, and packed binary functions have separate byte rules.

| Function | Result and restrictions |
|---|---|
| `LEN(s$)` | Number of Unicode scalar values. Numeric arguments are rejected. |
| `LEN(buffer)`, `BYTELEN(buffer)` | Number of bytes in a BYTES buffer. |
| `LEN(record)` | Byte size of the record's current binary `PUT` payload, including nested fields and variable-string headers. Available; see [record sizes](user-defined-types.md#record-byte-length). |
| `LEFT$(s$, count)` | First `count` characters, clipped at the string end; count must be nonnegative. |
| `RIGHT$(s$, count)` | Last `count` characters, clipped to the string length; count must be nonnegative. |
| `MID$(s$, start[, count])` | Substring beginning at 1-based `start`; omitted count means through the end. Start must be at least 1, count nonnegative. Starting past the end returns `""`. See [slice and MID$ assignment](string-slicing.md). |
| `INSTR(s$, needle$)` | 1-based first match, or `0` if absent. Empty needle returns `1`, including in an empty string. |
| `INSTR(start, s$, needle$)` | Searches at or after 1-based start; start must be at least 1. Starting past the last character returns `0`, even for an empty needle. |
| `INSTRREV(s$, needle$[, start])` | 1-based last match whose final character is at or before `start`, or `0` if absent. Omitted start or `-1` searches the entire string. |
| `STRCOMP(a$, b$[, compare])` | Returns `-1`, `0`, or `1` for less than, equal to, or greater than. Mode `0` (default) compares case-sensitive Unicode scalar order; `1` compares after lowercasing both strings. |
| `REPLACE$(s$, find$, replacement$[, start[, count]])` | Replaces nonoverlapping matches in the suffix beginning at 1-based `start` (default 1). Returns that suffix, omitting any preceding text. Count defaults to `-1` for all matches. |
| `LTRIM$(s$)`, `RTRIM$(s$)` | Remove leading/trailing Unicode whitespace, including tabs and newlines, not just spaces. |
| `UCASE$(s$)`, `LCASE$(s$)` | Unicode case conversion; can change the character count. |
| `SPACE$(count)` | Repeats a space; nonnegative count. |
| `STRING$(count, character)` | Repeats the first character of a string, or a numeric byte code. Empty string means a space. Numeric character is converted to a checked integer, then wraps modulo 256; count must be nonnegative. |
| `CHR$(code)` | Character U+0000..U+00FF; integer-converted code must be `0..255`. |
| `ASC(s$)` | Unicode scalar value of the first character, possibly above 255. Empty input raises illegal function call. |
| `STR$(x)` | Numeric text with no leading/trailing positive-number space. INT64/BYTE preserve exact decimal digits and BOOL prints TRUE/FALSE. Legacy NUMERIC uses integer text for integral magnitudes below `1e15`, otherwise Rust floating-point formatting. |
| `VAL(s$)` | Trims surrounding whitespace; parses a leading decimal number with optional E/D exponent, or leading `&H` hex/`&O` octal digits. Empty/invalid input returns `0`. |
| `HEX$(x)`, `OCT$(x)` | Uppercase hexadecimal / octal text; native integers preserve their bits, while legacy NUMERIC uses a direct truncating signed 64-bit cast. Out-of-range inputs saturate and NaN becomes zero. Negative values use unsigned 64-bit two's-complement representation. |

`STRCOMP` is a Rice extension. Its result is an ordering, independent of the numeric true value: test `STRCOMP(a$, b$, 1) = 0` for equality. Both operands must be strings. The optional comparison mode must be exactly numeric `0` or `1`; fractions, other numbers, NaN, and infinity raise illegal function call (error 5). Arguments evaluate once from left to right. Mode 1 uses Unicode lowercase conversion, which can expand characters, but does not perform locale collation, full case folding, or Unicode normalization. Thus `STRCOMP("É", "é", 1)` is `0`, while `"ß"` and `"SS"`, or composed and decomposed accents, remain distinct. Ordinary comparison operators are unchanged. Use the [LIKE operator](language-reference.md#wildcard-matching) for wildcard matching.

`INSTRREV` and `REPLACE$` are Rice extensions. Matches are case-sensitive and positions count Unicode scalar values. Optional positions and counts use the checked, truncating integer conversion described above. Arguments are evaluated once, from left to right; even no-op calls validate their argument types and ranges.

For `INSTRREV`, explicit start must be `-1` or at least 1. A start beyond the string's length returns `0`, including for an empty needle. An empty source also returns `0`. An empty needle in a nonempty source returns the effective start position. Overlapping matches are considered: `INSTRREV("aaa", "aa")` returns `2`. The limit applies to the **end** of the match: `INSTRREV("banana", "ana", 5)` returns `2`, while an unrestricted search returns `4`.

For `REPLACE$`, start must be at least 1, and count must be `-1` or nonnegative. Starting beyond the final character returns `""`. An empty find string or zero count returns the selected suffix unchanged; an empty replacement deletes matches. Replacement proceeds left to right without searching newly inserted text. For example, `REPLACE$("prefix abc abc", "abc", "X", 8, 1)` returns `"X abc"`; `REPLACE$("abc", "", "X", 2)` returns `"bc"`. Neither function accepts a comparison-mode argument or modifies its source string.

`VAL` accepts an optional sign, digits with at most one decimal point, and an
optional `E`, `e`, `D`, or `d` exponent with optional sign and at least one digit.
At least one mantissa digit is required. A second decimal point or other text
ends the numeric prefix: `VAL("1.2.3")` returns `1.2`. Scientific prefixes work
before trailing text: `VAL("1E2tail")` is `100`, and `VAL("-.5D+2kg")` is `-50`.
An incomplete exponent is excluded from the prefix, so `VAL("2D-")` returns `2`.
Internal whitespace ends the prefix (`VAL("1 2")` is `1`); only surrounding
whitespace is trimmed.

Radix behavior is unchanged: `&H`/`&O` digits use signed `i64` conversion, overflow
returns `0`, and a leading sign before the radix prefix is unsupported.
For example, `VAL("&HFFFFFFFFFFFFFFFF")` and `VAL("-&H10")` are both `0`.
Whole-string host float parsing also accepts `"NaN"` and `"inf"`; decimal
overflow/underflow can produce infinity or zero. Numeric suffixes do not impose
declared types: trailing text ends the prefix instead. This is Rice's permissive
conversion behavior; numeric literals in source are parsed separately.

Large string counts can exhaust host memory; there is no documented BASIC string-size limit. Text uses Unicode. See [implementation limits](compatibility.md).

## Packed binary conversion

These functions work. Binary strings map each byte to a character U+0000..U+00FF. On conversion back, characters above U+00FF become byte 255. Use binary/record I/O for these strings; UTF-8 text I/O does not preserve their byte representation.

| Encode | Decode | Layout |
|---|---|---|
| `MKI$(x)` | `CVI(s$)` | 2-byte little-endian signed integer. |
| `MKL$(x)` | `CVL(s$)` | 4-byte little-endian signed integer. |
| `MKS$(x)` | `CVS(s$)` | 4-byte little-endian IEEE single precision. |
| `MKD$(x)` | `CVD(s$)` | 8-byte little-endian IEEE double precision. |

MKI$/MKL$ require exact integers within their signed width; fractions, nonfinite values, and overflow fail. MKS$ rounds to SINGLE and rejects finite overflow. CVI/CVL/CVS return INTEGER/LONG/SINGLE; CVD returns NUMERIC. Decoders require at least the listed byte count and ignore extra bytes. IEEE encodings are not Microsoft Binary Format; there are no `MKSMBF$`, `MKDMBF$`, `CVSMBF`, or `CVDMBF` functions. See [binary record layouts](file-io.md).

## Interpreter state, files, arrays, and console

| Function | Behavior / detailed reference |
|---|---|
| `LBOUND(array[, dimension])`, `UBOUND(array[, dimension])` | Bounds recorded by DIM/REDIM. Dimension defaults to 1; invalid dimension or undimensioned array raises illegal function call. Bare `array`, `array()`, and an indexed form are accepted as the name; indices in that first argument are not evaluated. Bounds do not enforce ordinary array access. See [arrays](language-reference.md). |
| `FREEFILE` | Lowest unused handle in `1..255`; returns `0` if none is free. |
| `EOF(handle)` | `-1` at end of file, otherwise `0`. |
| `LOF(handle)` | File length in bytes. |
| `LOC(handle)` | Current zero-based byte offset, including RANDOM; prefers the reader if present, otherwise the writer, and returns `0` on position-query failure. |
| `SEEK(handle)` | One-based byte position of the next operation, including RANDOM files. See [file I/O](file-io.md) for all file functions. |
| `CSRLIN` or `CSRLIN()` | Tracked 1-based console row. |
| `POS(argument)` | Tracked 1-based column; the argument is evaluated but ignored. |
| `INKEY$` or `INKEY$()` | Nonblocking key input, empty string if no key; extended keys have a NUL prefix. |
| `INPUT$(count[, handle])` | Reads up to the positive integer count of bytes from console or file, stopping on EOF/read error; decodes as lossy UTF-8. `#` before the file argument is optional. See [console](console.md) and [file I/O](file-io.md) for byte and EOF behavior. |
| `SCREEN(row, column[, flag])` | Character byte from Rice's screen buffer; the optional flag is evaluated but ignored, so attributes are not returned. |
| `DET` or `DET()` | Determinant saved by the latest successful `MAT ... = INV(...)`; initially zero. See [MAT](mat-operations.md). |
| `DET(matrix)` or `DET(matrix())` | Independently computes a nonempty square matrix's determinant, returning zero for an exactly singular matrix. Accepts a bare array name or empty array parentheses, with no index expressions or value evaluation; leaves the saved `DET()` unchanged. See [MAT](mat-operations.md#determinants). |
| `ERR`, `ERL` (or zero-argument calls) | Last classic error code and numbered BASIC line, initially zero. |
| `EXTYPE`, `EXTEXT$` (or zero-argument calls) | Structured exception code and description, initially zero/empty and cleared on normal handler completion. Early control transfers can leave stale state. See [errors](error-handling.md). |

`TAB(column)` and `SPC(count)` are **PRINT items**, not general expression functions. See [console output](console.md).

## Clock and host environment

| Function | Behavior |
|---|---|
| `TIMER` | Local seconds since midnight. Windows includes milliseconds; the non-Windows implementation has whole-second resolution. |
| `DATE$` or `DATE$()` | Local date as `MM-DD-YYYY`, unless overridden by assignment. |
| `TIME$` or `TIME$()` | Local time as `HH:MM:SS`, unless overridden by assignment. |
| `ENVIRON$(name$)` | Interpreter override, if present, otherwise host environment value; absent/unreadable value gives `""`. Names are case-sensitive on Unix; override names are uppercased on Windows. |
| `ENVIRON$(index)` | The 1-based environment entry as `NAME=value`, with interpreter overrides merged into host entries and sorted by name. An index past the end returns `""`. See ordering and index rules below. |
| `CURDIR$()` | Host process working directory; `""` if it cannot be obtained. No drive argument. |
| `COMMAND$()` or `COMMAND()` | Program arguments joined by single spaces, excluding the source filename and interpreter options. No arguments produce `""`. Case, Unicode, internal whitespace, and empty argument fields are preserved; shell quoting and original argument boundaries are not reconstructed. Parentheses are required; bare COMMAND$ is an ordinary variable. |

Pass arguments as `rice program.bas First "two words"`; use `rice program.bas -- --flag` for dash-leading arguments. Interpreter flags remain active before `--`, even after the filename. For example, `rice program.bas First Second` gives COMMAND$() the text `First Second`. See the [CLI contract](runtime.md#build-and-command-line).

REPL and library interpreters start with empty program arguments instead of reading host process arguments. Library callers can use `interpreter.set_command_line_args(["First", "two words"])` to replace them; they persist across execution and CLEAR. A standalone builtin registry returns an empty string. Both aliases require zero arguments and are available.

Numeric `ENVIRON$` indices use the usual checked integer conversion: fractional
values truncate toward zero, values below 1 raise illegal function call, and
NaN/infinity or values outside signed 64-bit range raise overflow. Each call
rebuilds the current entry list. Host entries whose names or values cannot be
represented as Unicode are skipped; interpreter overrides replace matching host
names and can add entries. Sorting is lexical by name, not the host's original
environment order or a locale-specific order. Names are case-sensitive on Unix;
Windows enumeration uppercases names before merging and sorting. Changing an
override during enumeration can change subsequent positions.

The numeric form works and does not mutate the host environment.
An empty value returns `"NAME="`, distinct from the `""` end-of-list result.
As with the existing string form, the alias `ENVIRON(...)` and standalone builtin
registry calls read only the host environment; use `ENVIRON$(...)` to include
interpreter-local overrides.

```basic
i = 1
DO
    entry$ = ENVIRON$(i)
    IF entry$ = "" THEN EXIT DO
    PRINT entry$
    i = i + 1
LOOP
```

**Setters:** `DATE$ = expression` accepts `MM-DD-YYYY` or `MM/DD/YYYY` after trimming for validation. It checks month `1..12`, day `1..31`, and year `0..9999`, but not month lengths or leap years. `TIME$ = expression` checks `HH:MM:SS`, hours `0..23`, minutes/seconds `0..59`. Both store the original string, including surrounding whitespace; they override subsequent reads without changing the OS clock. The time override does not tick and does not change `TIMER`.

The `ENVIRON "name=value"` statement stores an interpreter-local override, inherited by commands started through `SHELL`. Empty names, missing `=`, or NUL characters are errors; empty values are allowed. It does not modify the parent process environment. See [runtime and host statements](runtime.md).

## Implementation sources

The registry and pure functions are in [`src/builtins.rs`](../src/builtins.rs); stateful dispatch is in [`src/interpreter.rs`](../src/interpreter.rs), expression spelling in [`src/parser.rs`](../src/parser.rs), and integer/binary conversions in [`src/value.rs`](../src/value.rs).
