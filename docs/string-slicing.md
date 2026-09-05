# String Slicing

Rice BASIC supports colon slicing in both the default QBasic mode and ANSI mode.
It is an ANSI-style feature, not a claim that QBasic 1.1 recognizes this syntax.
The `LEFT$`, `MID$`, and `RIGHT$` functions are also available in both modes.
See [Dialects](dialects.md) and [Built-in Functions](builtins.md).

## Extraction syntax

```text
name(start_expression : end_expression)
```

Both bounds are required, positions start at 1, and the end position is inclusive.
The named value must be a string. The syntax applies to a **plain variable name**,
not a literal, arbitrary expression, array element, or record field; assign such
a value to a temporary string before slicing it.

```basic
s$ = "Hello, World!"
PRINT s$(1:5)                      ' Hello
PRINT s$(8:12)                     ' World
PRINT s$(1:1)                      ' H
```

The `$` suffix is conventional and gives an undefined scalar its string default.
An explicitly declared `DIM s AS STRING` also works. An existing numeric value
cannot be sliced and is not implicitly converted to text.

| Bounds | Extraction behavior |
| --- | --- |
| Finite bound below `1` | Runtime error |
| Positive fractional bound | Truncated toward zero after the lower-bound check |
| End beyond string length | Clamped to the string's end |
| Start beyond string length | Empty string |
| End before start after truncation | Empty string |
| Omitted bound, such as `s$(:3)` or `s$(3:)` | Parse error |

For example, `"ABCDE"` stored in `s$` gives `"BC"` for `s$(2.9:3.9)`.
A bound of `0.9` is rejected rather than becoming zero. Non-finite bounds are
not separately validated by slicing; no portable behavior is promised for them.

Indices count Unicode scalar values, as do `LEN`, `LEFT$`, `MID$`, `RIGHT$`,
and `INSTR`. They do not count UTF-8 bytes or grapheme clusters. Thus `"é"` is one
position, while a letter followed by a separate combining accent occupies two.
This differs from historical byte/code-page string behavior.

```basic
s$ = "éx"
PRINT LEN(s$)                      ' 2
PRINT s$(1:1)                      ' é
```

## Slice assignment

```text
[LET] name(start_expression : end_expression) = replacement_expression
```

The replacement must be a string. The selected range is removed and the complete
replacement is inserted. Its length may differ from the old slice, so the total
string can grow or shrink.

```basic
s$ = "ABCDEF"
s$(3:4) = "XYZ"
PRINT s$                           ' ABXYZEF
s$(3:5) = ""
PRINT s$                           ' ABEF
```

The same bound checks and truncation rules apply as for extraction. An end beyond
the string length replaces through the end. A valid range starting beyond the
string length appends the replacement without padding. A reversed range normally
makes no change; the current empty-string edge case still inserts the replacement
because both byte endpoints collapse to zero.

```basic
s$ = "ABC"
s$(10:12) = "!"
PRINT s$                           ' ABC!
s$(3:1) = "ignored"
PRINT s$                           ' ABC!
```

An undefined slice target is treated as `""` unless QBasic's `OPTION EXPLICIT` rejects the
name. This slice-specific fallback can also apply to an unsuffixed name that has
not been read or initialized as a numeric variable. Existing non-string targets
still fail. Use string declarations or `$` names for predictable defaults.

Slicing accepts numeric expressions for each bound, but it does not provide a
writable reference argument. Passing `s$(1:3)` to a `BYREF` procedure passes a
temporary value; see [Procedures and Scope](procedures.md).

## Relation to LEFT$, MID$, and RIGHT$

For positive, in-range integer positions these expressions select the same text:

| Function | Slice |
| --- | --- |
| `LEFT$(s$, n)` | `s$(1:n)` |
| `RIGHT$(s$, n)` | `s$(LEN(s$)-n+1:LEN(s$))` |
| `MID$(s$, start, count)` | `s$(start:start+count-1)` |
| `MID$(s$, start)` | `s$(start:LEN(s$))` |

These are not general substitutions at the boundaries. `LEFT$(s$, 0)` and
`RIGHT$(s$, 0)` return `""`, while a slice end of `0` is invalid.
`RIGHT$(s$, n)` accepts an oversized `n` and returns the whole string; its
formula above would produce an invalid lower slice bound. Empty strings and
zero-length `MID$` results also require care.

## MID$ assignment in QBasic mode

This separate statement is accepted only in QBasic mode:

```text
MID$(name, start [, count]) = replacement
```

The target must be a plain name. `start` is truncated to an integer and must be
at least `1`; an explicit count is truncated and must be non-negative.
The statement overwrites at most the minimum of the requested count, replacement
length, and characters remaining in the original string. Without a count, only
replacement length and remaining characters limit it.

It never grows or shrinks the original string; a start past its end makes no
change. A short replacement leaves the rest of the selected area intact.

```basic
OPTION DIALECT "QB"
s$ = "ABCDEF"
MID$(s$, 3, 2) = "XYZ"
PRINT s$                           ' ABXYEF
MID$(s$, 3, 3) = "Q"
PRINT s$                           ' ABQYEF
```

The `MID$` **function** remains available in ANSI mode; only this assignment
statement is rejected there.

## Concatenation

`&` concatenates two string values in both modes. It does not automatically convert
numbers to strings; use `STR$` or separate `PRINT` items. QBasic additionally
permits string `+`; ANSI treats `+` as arithmetic and rejects string operands.
In QBasic mode, an `&` immediately following an identifier is its numeric suffix;
put spaces around the concatenation operator to avoid that ambiguity.

```basic
first$ = "Hello"
last$ = "world"
PRINT first$ & ", " & last$ & "!"
```

## Example: reversing scalar-value order

```basic
FUNCTION ReverseStr$(BYVAL s$) AS STRING
    DIM result$
    FOR i = LEN(s$) TO 1 STEP -1
        result$ = result$ & s$(i:i)
    NEXT i
    ReverseStr$ = result$
END FUNCTION
PRINT ReverseStr$("Hello")         ' olleH
```

This reverses Unicode scalar values, not displayed grapheme clusters. Colon
slicing and Unicode indexing are exercised by the integration suite; exact
historical byte-string compatibility is not claimed.
