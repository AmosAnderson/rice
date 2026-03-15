# Procedures and Scope

## SUB (Subroutines)

Subroutines are procedures that do not return a value.

### Definition

```basic
SUB name [(parameters)]
    ' body
END SUB
```

### Calling

```basic
CALL MySubroutine(arg1, arg2)
MySubroutine arg1, arg2         ' CALL keyword is optional
```

### Example

```basic
SUB PrintBanner (title AS STRING, width AS INTEGER)
    PRINT STRING$(width, "=")
    PRINT title
    PRINT STRING$(width, "=")
END SUB

CALL PrintBanner("Welcome", 20)
```

### EXIT SUB

Exit a subroutine early:

```basic
SUB CheckValue (x AS INTEGER)
    IF x < 0 THEN EXIT SUB
    PRINT "Value is: "; x
END SUB
```

---

## FUNCTION

Functions are procedures that return a value. Assign the return value by assigning to the function name.

### Definition

```basic
FUNCTION name [(parameters)] [AS type]
    ' body
    name = return_value
END FUNCTION
```

### Example

```basic
FUNCTION Factorial (n AS INTEGER) AS LONG
    IF n <= 1 THEN
        Factorial = 1
    ELSE
        Factorial = n * Factorial(n - 1)
    END IF
END FUNCTION

PRINT Factorial(5)    ' Prints 120
```

### EXIT FUNCTION

Exit a function early (returns whatever has been assigned so far, or the default value):

```basic
FUNCTION SafeDiv (a AS DOUBLE, b AS DOUBLE) AS DOUBLE
    IF b = 0 THEN
        SafeDiv = 0
        EXIT FUNCTION
    END IF
    SafeDiv = a / b
END FUNCTION
```

---

## DECLARE

Forward-declare procedures. This is optional in RICE BASIC but supported for compatibility:

```basic
DECLARE SUB MyProc (x AS INTEGER)
DECLARE FUNCTION MyFunc (x AS INTEGER) AS INTEGER
```

---

## DEF FN (Inline Functions)

Define simple functions using either single-line or multi-line syntax:

### Single-Line Form

```basic
DEF FNSquare(x) = x * x
DEF FNArea(r) = 3.14159 * r ^ 2
DEF FNCelsius(f) = (f - 32) * 5 / 9

PRINT FNSquare(5)       ' 25
PRINT FNArea(3)         ' 28.27...
PRINT FNCelsius(212)    ' 100
```

### Multi-Line Form

```basic
DEF FNMax(a, b)
    IF a > b THEN
        FNMax = a
    ELSE
        FNMax = b
    END IF
END DEF

PRINT FNMax(10, 20)     ' 20
```

DEF FN functions:
- Must have names starting with `FN`
- Can use single-line (`= expr`) or multi-line (`... END DEF`) syntax
- Can take multiple parameters
- Are scoped to the module where they are defined

---

## Parameters

### Pass by Reference (Default)

By default, parameters are passed by reference. Changes inside the procedure affect the original variable:

```basic
SUB Increment (x AS INTEGER)
    x = x + 1
END SUB

DIM n AS INTEGER
n = 10
CALL Increment(n)
PRINT n    ' 11
```

### Pass by Value (BYVAL)

Use `BYVAL` to pass a copy. Changes inside the procedure do not affect the original:

```basic
SUB TryIncrement (BYVAL x AS INTEGER)
    x = x + 1
    PRINT "Inside: "; x    ' 11
END SUB

DIM n AS INTEGER
n = 10
CALL TryIncrement(n)
PRINT "Outside: "; n        ' 10
```

### Array Parameters

Pass arrays by using empty parentheses:

```basic
SUB PrintArray (arr() AS INTEGER, size AS INTEGER)
    FOR i = 0 TO size - 1
        PRINT arr(i)
    NEXT i
END SUB
```

### Type Parameters

User-defined types can be passed to procedures:

```basic
TYPE PointType
    x AS SINGLE
    y AS SINGLE
END TYPE

SUB PrintPoint (p AS PointType)
    PRINT "(" ; p.x; ","; p.y; ")"
END SUB
```

---

## Scope Rules

### Local Scope

Variables declared within a SUB or FUNCTION are local by default. They are created when the procedure is entered and destroyed when it exits:

```basic
SUB MyProc
    DIM localVar AS INTEGER    ' Only exists inside MyProc
    localVar = 42
END SUB
' localVar does not exist here
```

### SHARED

Access global (module-level) variables from within a procedure:

```basic
DIM total AS INTEGER
total = 100

SUB AddToTotal (amount AS INTEGER)
    SHARED total
    total = total + amount
END SUB

CALL AddToTotal(50)
PRINT total    ' 150
```

### STATIC Variables

Variables declared `STATIC` retain their values between calls:

```basic
SUB Counter
    STATIC count AS INTEGER
    count = count + 1
    PRINT "Called "; count; " times"
END SUB

CALL Counter    ' Called 1 times
CALL Counter    ' Called 2 times
CALL Counter    ' Called 3 times
```

### STATIC SUB

Make all variables in a SUB static:

```basic
SUB Counter STATIC
    count = count + 1
    PRINT "Called "; count; " times"
END SUB
```

---

## GOSUB / RETURN

An older-style subroutine mechanism using labels:

```basic
GOSUB printHeader
PRINT "Main body"
GOSUB printFooter
END

printHeader:
    PRINT "=== Header ==="
    RETURN

printFooter:
    PRINT "=== Footer ==="
    RETURN
```

GOSUB/RETURN uses a return-address stack, so nested calls work correctly. Prefer SUB/FUNCTION for new code.

---

## Function Resolution Order

When RICE BASIC encounters `name(args)` in an expression, it resolves in this order:

1. **Built-in function** (e.g., `LEN`, `MID$`, `ABS`)
2. **User-defined FUNCTION** (defined with `FUNCTION...END FUNCTION`)
3. **Array access** (e.g., `myArray(index)`)

This means you cannot name a function or array the same as a built-in function.
