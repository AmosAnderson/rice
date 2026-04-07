# String Slicing

RICE BASIC uses ANSI Full BASIC colon-slicing syntax for substring operations, replacing the traditional LEFT$/MID$/RIGHT$ functions.

## Reading Substrings

Use `variable(start:end)` to extract a substring. Indices are 1-based:

```basic
LET A$ = "Hello, World!"
PRINT A$(1:5)          ! "Hello"
PRINT A$(8:13)         ! "World!"
PRINT A$(1:1)          ! "H"
```

## String Slice Assignment

Assign to a slice to modify part of a string in-place:

```basic
LET A$ = "Hello, World!"
LET A$(1:5) = "Howdy"
PRINT A$               ! "Howdy, World!"
```

The replacement string can be a different length than the slice:

```basic
LET A$ = "ABCDEF"
LET A$(3:4) = "XYZ"
PRINT A$               ! "ABXYZEF"
```

## String Concatenation

Use the `&` operator to concatenate strings:

```basic
LET first$ = "Hello"
LET second$ = "World"
LET result$ = first$ & ", " & second$ & "!"
PRINT result$          ! "Hello, World!"
```

The `+` operator is strictly arithmetic and does not concatenate strings.

## Comparison with Traditional BASIC

| Traditional BASIC       | ANSI Full BASIC          |
|------------------------|--------------------------|
| `LEFT$(A$, 5)`         | `A$(1:5)`               |
| `RIGHT$(A$, 5)`        | `A$(LEN(A$)-4:LEN(A$))` |
| `MID$(A$, 3, 4)`       | `A$(3:6)`               |
| `A$ + B$`              | `A$ & B$`               |
| `MID$(A$, 3, 4) = "X"` | `LET A$(3:6) = "X"`    |
