# PRINT USING Formatting

`PRINT USING` provides formatted output using a format string that controls how values are displayed.

## Syntax

```basic
PRINT USING format$; expr1; expr2; ...
PRINT USING format$; expr1, expr2, ...
```

The format string contains format specifiers that are applied to each expression in order. If there are more expressions than specifiers, the format string repeats from the beginning.

---

## Numeric Format Specifiers

### Digit Positions (`#`)

Each `#` represents one digit position. Numbers are right-justified within the field:

```basic
PRINT USING "###"; 5         '   5
PRINT USING "###"; 42        '  42
PRINT USING "###"; 123       ' 123
```

### Decimal Point (`.`)

Place a decimal point to specify fixed-point formatting:

```basic
PRINT USING "###.##"; 3.14159     '   3.14
PRINT USING "#.####"; 0.12345     ' 0.1235
PRINT USING "##.#"; 42.678        ' 42.7
```

### Leading Sign (`+`)

A `+` at the beginning prints `+` for positive numbers and `-` for negative numbers:

```basic
PRINT USING "+###.##"; 42.5      ' + 42.50
PRINT USING "+###.##"; -42.5     ' - 42.50
```

### Trailing Sign (`+` or `-`)

A `+` at the end prints the sign after the number. A `-` at the end prints `-` for negative or a space for positive:

```basic
PRINT USING "###.##+" ; 42.5     '  42.50+
PRINT USING "###.##+" ; -42.5    '  42.50-
PRINT USING "###.##-" ; 42.5     '  42.50
PRINT USING "###.##-" ; -42.5    '  42.50-
```

### Dollar Sign (`$$`)

A floating dollar sign appears immediately before the first digit:

```basic
PRINT USING "$$###.##"; 42.5     '  $42.50
PRINT USING "$$###.##"; 1.5      '   $1.50
```

### Asterisk Fill (`**`)

Fill leading spaces with asterisks:

```basic
PRINT USING "**###.##"; 42.5     ' ***42.50
PRINT USING "**###.##"; 1.5      ' ****1.50
```

### Asterisk Fill with Dollar (`**$`)

Combine asterisk fill with a dollar sign:

```basic
PRINT USING "**$###.##"; 42.5    ' ****$42.50
PRINT USING "**$###.##"; 1.5     ' *****$1.50
```

### Thousands Separator (`,`)

Place a comma among `#` signs (before the decimal) to enable thousands grouping:

```basic
PRINT USING "##,###.##"; 1234.56     '  1,234.56
PRINT USING "###,###"; 1000000       ' 1,000,000
```

### Scientific Notation (`^^^^`)

Four carets produce scientific notation (E+nn format):

```basic
PRINT USING "##.##^^^^"; 1234.5      '  1.23E+03
PRINT USING "##.##^^^^"; 0.00456     '  4.56E-03
PRINT USING "#.##^^^^"; 1            ' 1.00E+00
```

### Overflow Indicator (`%`)

When a number is too large for the format, a `%` prefix is added:

```basic
PRINT USING "##"; 123     ' %123
PRINT USING "#.#"; 99.9   ' %99.9
```

---

## String Format Specifiers

### First Character (`!`)

Print only the first character of the string:

```basic
PRINT USING "!"; "Hello"     ' H
PRINT USING "!"; "World"     ' W
```

### Entire String (`&`)

Print the entire string as-is:

```basic
PRINT USING "&"; "Hello"     ' Hello
```

### Fixed-Width Field (`\ \`)

The width is the total number of characters from the opening backslash through the closing backslash, including both backslashes:

```basic
PRINT USING "\  \"; "Hello"       ' Hel   (3 chars wide)
PRINT USING "\    \"; "Hi"        ' Hi     (5 chars wide, padded)
PRINT USING "\         \"; "Hello" ' Hello      (10 chars wide)
```

---

## Escape Character (`_`)

Use underscore to include the next character as a literal in the output:

```basic
PRINT USING "_!###"; 42      ' ! 42
PRINT USING "###_#"; 42      '  42#
```

---

## Literal Text

Characters in the format string that are not part of a format specifier are output as-is:

```basic
PRINT USING "Total: $$###.##"; 42.50    ' Total:  $42.50
```

---

## Multiple Values

When multiple values are printed, each uses the next format specifier. If the format string runs out of specifiers, it wraps around:

```basic
PRINT USING "###  "; 1; 2; 3
' Output: 1    2    3
```

---

## Complete Examples

### Financial Report

```basic
PRINT USING "Item: \          \  $$##,###.##"; "Widget", 1234.50
PRINT USING "Item: \          \  $$##,###.##"; "Gadget", 567.89
PRINT USING "Item: \          \  $$##,###.##"; "Thingamajig", 42.00
```

Output:
```
Item: Widget       $1,234.50
Item: Gadget         $567.89
Item: Thingamaji      $42.00
```

### Scientific Data

```basic
PRINT USING "Value: +#.####^^^^"; 6.022e23
PRINT USING "Value: +#.####^^^^"; -1.602e-19
PRINT USING "Value: +#.####^^^^"; 3.14159
```

### Table Formatting

```basic
PRINT USING "! \         \ ###.##"; "A", "Alice", 95.5
PRINT USING "! \         \ ###.##"; "B", "Bob", 87.3
PRINT USING "! \         \ ###.##"; "C", "Carol", 92.1
```
