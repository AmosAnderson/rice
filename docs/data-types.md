# Data types and declarations

Every declaration establishes a runtime type. This includes scalar variables,
array elements, record fields, parameters, function results, STATIC variables,
typed constants, and typed SHARED/COMMON bindings. Valid assignments convert to
the declared representation; invalid assignments raise an error before replacing
the destination value.

| Type | Representation | Default | Assignment rule |
|---|---|---|---|
| `NUMERIC`, `DOUBLE` | IEEE 754 binary64 (`f64`); two names for the same type | `0` | Numeric values; an INT64 must be exactly representable unless explicitly converted with CDBL |
| `INTEGER` | Signed 16-bit integer, −32768 through 32767 | `0` | Exact integer in range |
| `LONG` | Signed 32-bit integer, −2147483648 through 2147483647 | `0` | Exact integer in range |
| `SINGLE` | IEEE 754 binary32 (`f32`) | `0` | Numeric values rounded to binary32; finite overflow is an error |
| `INT64` | Signed 64-bit integer, −9223372036854775808 through 9223372036854775807 | `0` | Exact integer in range |
| `BOOL`, `BOOLEAN` | Boolean | `FALSE` | Boolean or finite numeric value; zero is false, nonzero is true |
| `BYTE` | Unsigned 8-bit integer, 0 through 255 | `0` | Exact integer in range |
| `BYTES` | An owned sequence of bytes | Empty buffer | Byte buffer only; text needs explicit encoding |
| `STRING` | Unicode text | Empty string | String only |
| `STRING * n` | Exactly n Unicode scalar values | n spaces | String only; truncate excess characters or pad with spaces |
| A declared `TYPE` name | Record aggregate | Fresh fields with their declared defaults | A record with the same TYPE name |

Fractions, nonfinite values, and out-of-range values cannot be assigned to integer
types. Arithmetic may produce a wider numeric type; assignment checks the result
again. Fixed string widths count Unicode scalar values, not UTF-8 bytes or display
columns. A zero-length fixed string is always empty.

An explicit `AS` overrides a name suffix. Without `AS`, suffixes mean `%` INTEGER,
`&` LONG, `!` SINGLE, `#` DOUBLE, and `$` STRING; the suffix remains part of the
identifier. `DIM` and `STATIC` without AS or a suffix declare NUMERIC. A plain
unsuffixed implicit variable stays dynamic unless a DEFtype rule applies when it
is first initialized. DEFINT, DEFLNG, DEFSNG, DEFDBL, and DEFSTR select enforced
default types for subsequent new names; existing bindings keep their type.

`TYPEOF$(value)` reports INTEGER, LONG, SINGLE, NUMERIC, INT64, BOOL, BYTE, BYTES,
STRING, or the record type name. DOUBLE reports NUMERIC; BOOLEAN reports BOOL.
Fixed and variable strings both report STRING.

SWAP and BYREF copy-back validate every destination before writing any of them.
Static variables retain their declarations across calls, even when a later call
skips the declaration statement. CLEAR retains declarations and restores their
typed defaults. READ, INPUT, file input, and binary GET apply the same destination
conversion rules. Typed SHARED/COMMON convert an existing dynamic shared value without resetting it. An existing declared type must agree; conflicting declarations fail. Omitting AS preserves its existing type and value.

```basic
DIM identifier AS INT64
DIM enabled AS BOOLEAN
DIM marker AS BYTE
DIM packet AS BYTES
identifier = CINT64("9223372036854775807")
enabled = TRUE()
marker = 255
packet = BYTES(0, 127, 255)
PRINT identifier; " "; enabled; " "; HEXENCODE$(packet)
```

There is no new numeric-literal suffix. Numeric literals still begin as `f64`,
so use `CINT64("decimal digits")` for exact integers beyond the consecutive
integer range of floating point. CINT, CLNG, and CSNG return INTEGER, LONG, and SINGLE; CINT/CLNG round ties to even before checking their destination ranges. Converting a previously rounded numeric literal
cannot recover its original digits. `CINT64` trims surrounding whitespace and
accepts a signed decimal integer string; it does not parse hexadecimal strings,
fractional text, or scientific notation.

## Arithmetic and comparisons

Integer arithmetic involving `INTEGER`, `LONG`, `INT64`, or `BYTE` uses checked signed 64-bit
operations when the other numeric operand is integral. Overflow raises an error.
Integer widths are storage types: arithmetic may produce an `INT64`; assigning that result
back to INTEGER, LONG, INT64, or BYTE checks the destination range again. `MOD` retains Rice's floor-quotient rule,
including negative operands. `/` uses floating point. Native integer division
and integer logical/bitwise operations require integral operands; they reject
fractional values instead of falling back to a rounded floating-point result.

Mixing an exact integer with fractional floating point requires the integer to
be exactly representable as `f64`. Otherwise, explicitly choose the rounding with
`CDBL(integer)`. The same precision check applies to math functions that
need floating-point input. Numeric comparisons compare exact integers against
floating-point values without first rounding the integer.

Comparisons involving INTEGER, LONG, SINGLE, INT64, BOOL, BYTE, or BYTES return BOOL. Comparisons between NUMERIC/DOUBLE values or strings retain numeric truth values: `-1` for true, with
`0` for false. Booleans compare with booleans; byte buffers compare
lexicographically by unsigned bytes. A boolean does not implicitly become a
number in arithmetic. Use CINT, CLNG, CSNG, CINT64, CBYTE, or CDBL explicitly to obtain
`1` or `0`. `CBOOL(number)` converts finite numeric values
using zero/nonzero truth.

`TRUE()` and `FALSE()` return native booleans. Bare `TRUE` and `FALSE` also work
when the name has no existing variable or constant binding. Native boolean
`AND`, `OR`, `XOR`, and `NOT` operate logically. Numeric logical operators perform bitwise operations. Operands are still evaluated
eagerly; these operators do not short-circuit.

Plain `PRINT` preserves every decimal digit of an `INT64`, prints booleans as
`TRUE`/`FALSE`, and prints byte buffers as hexadecimal with an `&H` prefix.
`PRINT USING` fixed numeric formats preserve full-range integer digits,
including signs, grouping, and zero decimal places. Scientific formats use a
checked floating-point conversion; use `CDBL(integer)` when rounding a large
integer for scientific notation is intended.

## Byte buffers and text

`BYTES` is separate from Unicode `STRING` and from the older packed binary-string
convention. Buffers can contain all byte values, including NUL and invalid UTF-8.
They are copied by assignment. `&` and `+` concatenate two buffers. Concatenation does not mix buffers with strings.

| Function | Behavior |
|---|---|
| `BYTES([byte, ...])` | Build a buffer from exact byte values; `BYTES()` is empty |
| `BYTEBUFFER(length[, fill])` | Allocate a buffer; nonnegative exact length, default fill `0` |
| `LEN(buffer)`, `BYTELEN(buffer)` | Byte count |
| `BYTEAT(buffer, position)` | Read a `BYTE` at a 1-based position |
| `BYTESET(buffer, position, byte)` | Return an updated copy; assign it to keep the change |
| `BYTESLICE(buffer, start[, count])` | Copy a slice; omitted count reaches the end, an oversized count is clipped |
| `UTF8ENCODE(text$)` | Encode Unicode text into UTF-8 bytes |
| `UTF8DECODE$(buffer)` | Decode valid UTF-8; invalid input raises an error |
| `HEXENCODE$(buffer)` | Uppercase hexadecimal text without a prefix |
| `HEXDECODE(text$)` | Decode pairs of hexadecimal digits; no prefix or separators |

Buffer positions always begin at **1**, independent of `OPTION BASE`. Positions
and counts must be exact integers. `BYTEAT` and `BYTESET` require an existing
element. `BYTESLICE` allows `start = LEN(buffer) + 1` to return an empty buffer;
other out-of-range starts and negative counts are errors.

```basic
DIM original AS BYTES
DIM changed AS BYTES
original = HEXDECODE("0041FF")
changed = BYTESET(original, 2, 66)
PRINT HEXENCODE$(original)        ' 0041FF
PRINT HEXENCODE$(changed)         ' 0042FF
PRINT UTF8DECODE$(UTF8ENCODE("café"))
```

## Files, records, and procedures

Top-level `GET`/`PUT` on a `BYTES` scalar or array element transfers raw bytes. Allocate the requested read size first with `BYTEBUFFER`; a short
read returns only the bytes read. There is no length header on a top-level buffer
transfer. `INT64`, `BOOL`, and `BYTE` scalar/element transfers use their native
binary layouts. Explicit declarations take precedence over suffixes.

In structured record I/O, an `INT64` field occupies eight little-endian bytes,
a `BOOL` uses one byte (`0` or `1`), and a `BYTE` uses one byte. A `BYTES` field
has a four-byte unsigned little-endian length followed by its payload. This
record framing differs from raw top-level buffer transfers and from older
two-byte-length binary strings. `LEN(record)` includes these fields and headers.
See [File I/O](file-io.md) for positioning and transfer limits.

```basic
FUNCTION NextIdentifier(BYVAL previous AS INT64) AS INT64
    NextIdentifier = previous + 1
END FUNCTION
PRINT NextIdentifier(CINT64("9007199254740993"))
```

Procedure conversion follows the actual SUB/FUNCTION definition. DECLARE signatures, when present, must match that definition before execution: kind, arity, passing modes, array markers, parameter types, and result type. An unsuffixed parameter/result without AS remains dynamic. Existing argument evaluation, copies, parenthesized BYVAL arguments, and unsupported whole-array binding rules apply. See [Procedures](procedures.md).

## Matrix operations

MAT calculations use floating point. Native numeric inputs must be exactly
representable as `f64`; an integer that would lose precision raises an error
instead of becoming a rounded value or zero. MAT writes to a declared array
validate all output elements before replacing the destination. Use ordinary
integer expressions and loops for exact full-range INT64 arithmetic.
