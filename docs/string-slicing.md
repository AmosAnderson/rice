# String Slicing

RICE BASIC supports ANSI X3.113-1991 string slicing with colon notation to extract and replace substrings. QBasic-compatible LEFT$, MID$, and RIGHT$ functions are also available as an alternative.

## Colon Slicing Syntax

Extract a substring by specifying a range of character positions (1-based):

```basic
s = "Hello, World!"

PRINT s(1:5)       ' "Hello"    - characters 1 through 5
PRINT s(8:12)      ' "World"    - characters 8 through 12
PRINT s(1:1)       ' "H"        - single character
```

### Open-Ended Slices

Omit one end of the range to slice from the beginning or to the end:

```basic
s = "Hello, World!"

PRINT s(8:)        ' "World!"   - from position 8 to the end
PRINT s(:5)        ' "Hello"    - from the beginning through position 5
```

### Equivalence to Traditional Functions

| Traditional Function     | ANSI Slice Equivalent     |
|--------------------------|---------------------------|
| `LEFT$(s, n)`            | `s(1:n)`                  |
| `RIGHT$(s, n)`           | `s(LEN(s)-n+1:)`          |
| `MID$(s, start, length)` | `s(start:start+length-1)` |
| `MID$(s, start)`         | `s(start:)`               |

---

## String Concatenation with &

Use the `&` operator to concatenate strings:

```basic
first = "Hello"
second = "World"
result = first & ", " & second & "!"
PRINT result    ' "Hello, World!"
```

The `&` operator always performs string concatenation, making it unambiguous (unlike `+` which could mean addition or concatenation depending on context).

---

## Slice Assignment

Assign to a slice to replace part of a string:

```basic
s = "Hello, World!"
s(8:12) = "BASIC"
PRINT s    ' "Hello, BASIC!"
```

The replacement string can be a different length than the slice:

```basic
s = "ABCDEF"
s(3:4) = "XYZ"
PRINT s    ' "ABXYZEF"
```

### Replacing from the Beginning

```basic
s = "Hello, World!"
s(1:5) = "Greet"
PRINT s    ' "Greet, World!"
```

### Replacing to the End

```basic
s = "Hello, World!"
s(8:) = "Everyone!"
PRINT s    ' "Hello, Everyone!"
```

---

## Examples

### Extracting Initials

```basic
full_name = "John Doe"
' Find the space
FOR i = 1 TO LEN(full_name)
    IF full_name(i:i) = " " THEN
        space_pos = i
        EXIT FOR
    END IF
NEXT i

first_initial = full_name(1:1)
last_initial = full_name(space_pos + 1 : space_pos + 1)
PRINT first_initial & "." & last_initial & "."    ' "J.D."
```

### Building a String Character by Character

```basic
result = ""
FOR i = 65 TO 90
    result = result & CHR(i)
NEXT i
PRINT result    ' "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
```

### Reversing a String

```basic
FUNCTION ReverseStr (s AS STRING) AS STRING
    DIM result AS STRING
    result = ""
    FOR i = LEN(s) TO 1 STEP -1
        result = result & s(i:i)
    NEXT i
    ReverseStr = result
END FUNCTION

PRINT ReverseStr("Hello")    ' "olleH"
```
