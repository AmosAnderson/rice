# PRINT USING

`PRINT USING` uses the same formatting engine in QBasic and ANSI modes, for console and file output:

```basic
PRINT USING format$; value [, value ...]
PRINT #channel, USING format$; value [, value ...]
```

A semicolon is required after the format expression. Values may be separated by commas or semicolons. The format expression must be a string; numeric fields require numeric values and string fields require strings. There is no implicit conversion between these value types.

## Applying a format

Fields consume values in order. If values remain at the end of the format, the **entire** format repeats, including literal text. Once no values remain, literal text continues until the next value field or the end of the format; remaining unfilled fields and everything after them are omitted.

```basic
PRINT USING "[#]"; 1, 2       ' [1][2]
PRINT USING "#/# END"; 1      ' 1/
PRINT USING "###  "; 1, 2, 3 ' two spaces before each digit and after each field
```

Commas between values do not insert ordinary print zones while formatting. A **trailing** comma advances to the next 16-column zone after the result and suppresses the newline. A trailing semicolon suppresses only the newline. Otherwise the statement ends with LF. `TAB(...)` and `SPC(...)` print items inside a `PRINT USING` statement are currently ignored, including their expressions; put spacing in the format string instead.

If values are supplied but the format has no consuming fields, the statement raises an illegal-function-call error instead of repeating forever. With no values, a literal-only format can print its literals, while a format starting with a value field prints nothing.

## Numeric fields

| Mark | Meaning |
|---|---|
| `#` | One integer or fractional digit position |
| `.` | Decimal point; following `#` marks set the number of fractional places |
| Leading `+` | Always print sign at the beginning, before field padding |
| Trailing `+` | Print `+` or `-` after the number |
| Trailing `-` | Print `-` for negative values, space for nonnegative values |
| `$$` | Floating dollar marker, with one extra integer digit position |
| `**` | Asterisk fill, with two extra integer digit positions |
| `**$` | Asterisk fill with floating dollar marker |
| Comma before decimal | Enable thousands grouping |
| `^^^^` after digit/decimal portion | Scientific notation |

Unmarked integer padding uses spaces. Fixed-point numbers round the absolute value to the requested precision, using halfway-away-from-zero rounding, then apply the sign. A field without a decimal point rounds to an integer. Floating-point representation can affect decimal ties. A negative value without an explicit sign marker replaces the final available leading padding character with `-`; if there is no room, it overflows.

Examples below show exact output inside backticks; spaces inside the backticks are significant.

| Format | Value | Result |
|---|---:|---|
| `###` | 5 | `  5` |
| `###.##` | 1.5 | `  1.50` |
| `###.##` | -1.5 | ` -1.50` |
| `###.##` | 123.456 | `123.46` |
| `+###` | 5 | `+  5` |
| `+###` | -5 | `-  5` |
| `###-` | 5 | `  5 ` |
| `###-` | -5 | `  5-` |
| `###.##+` | 42.5 | ` 42.50+` |
| `$$###.##` | 1.5 | `  $1.50` |
| `$$###.##` | 42.5 | ` $42.50` |
| `**###.##` | 1.5 | `****1.50` |
| `**$###.##` | 42.5 | `**$42.50` |
| `#,###.##` | 1234.56 | `1,234.56` |
| `##` | 123 | `%123` |
| `##` | -12 | `%-12` |

### Overflow and width quirks

If the ungrouped integer digits exceed the allocated positions, the formatter prefixes `%` to the value instead of truncating or raising an exception. Fixed-point overflow output keeps a negative sign, grouping, and fractional digits, but does not preserve explicit positive signs, dollar markers, or normal fill.

Grouping is determined by the total count of integer digit positions; commas in the format enable grouping rather than defining independent literal-width slots. The output width includes the grouping commas required for that count. `$` markers float by replacing fill just before the first digit/sign, or by prepending `$` if no fill is available. Consequently total output width is not always the literal number of characters in the format.

A decimal point can start a numeric field, but `.##` allocates no integer positions: even `0.5` formats as `%0.50`. Use `#.##` for a leading zero. Integer-only formatting internally casts to `i64`; huge/non-finite values do not have a validated QB-compatible display contract. Negative zero is not treated as negative for sign selection.

### Scientific notation

Four consecutive carets immediately after the numeric field select scientific formatting. The number of integer digit positions determines the mantissa width:

```basic
PRINT USING "##.##^^^^"; 1234.5   ' 12.35E+02
PRINT USING "##.##^^^^"; 0.00456  ' 45.60E-04
PRINT USING "#.##^^^^"; 1         ' 1.00E+00
```

Exponents have `E`, an explicit sign, and at least two digits. Leading/trailing sign markers work. Scientific formatting does not renormalize a mantissa that rounds past its allocated width: `PRINT USING "#.##^^^^"; 9.999` produces `10.00E+00`. It does not use the fixed-point `%` overflow mechanism, and dollar/grouping flags are not applied as they are for fixed-point formatting. Very small, very large, and non-finite scientific values have not been established as compatible with historical BASIC output.

## String fields

| Field | Meaning |
|---|---|
| `!` | First Unicode character; an empty string becomes one space |
| `&` | Entire string |
| `\ ... \` | Fixed character width, including both backslashes; truncate or right-pad with spaces |

```basic
PRINT USING "!"; "Hello"        ' H
PRINT USING "&"; "Hello"        ' Hello
PRINT USING "\  \"; "Hello"     ' Hell (4 characters)
PRINT USING "\    \"; "Hi"      ' Hi followed by 4 spaces (6 characters total)
```

Width is measured in Unicode characters, not bytes or terminal display cells. A backslash field currently treats **any** intervening characters as field width, not just spaces: `"\ab\"` is a four-character field. If no closing backslash appears, the field runs to the end of the format. These permissive malformed-format behaviors are implementation quirks rather than a portable formatting convention.

## Escapes and literal text

Underscore escapes the following character so that it prints literally. A final underscore with no following character emits nothing.

```basic
PRINT USING "_!###"; 42      ' ! 42
PRINT USING "###_#"; 42      ' 42# (one leading space)
PRINT USING "Total: $$###.##"; 42.5
```

Other characters are literal unless they start a recognized field. `#` and `.` always start numeric fields; `+` starts one when followed by `#`, `.`, `+`, `$`, or `*`; `**` and `$$` start their respective numeric fields. A single `$` or `*`, or carets outside the numeric suffix position, is literal. Escape punctuation explicitly when its literal meaning could be ambiguous.

## Example: repeated rows

```basic
PRINT USING "Item: \          \  $$##,###.##"; "Widget", 1234.5
PRINT USING "Item: \          \  $$##,###.##"; "Gadget", 567.89
PRINT USING "! \         \ ###.##"; "A", "Alice", 95.5
```

Each statement starts from the beginning of its format. Formatting emits a string; console cursor/buffer tracking and file column tracking then follow their normal rules. See [console](console.md) and [file I/O](file-io.md).
