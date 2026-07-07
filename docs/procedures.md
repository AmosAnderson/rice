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
SUB PrintBanner (title AS STRING, width AS NUMERIC)
    PRINT STRING$(width, "=")
    PRINT title
    PRINT STRING$(width, "=")
END SUB

CALL PrintBanner("Welcome", 20)
```

### EXIT SUB

Exit a subroutine early:

```basic
SUB CheckValue (x AS NUMERIC)
    IF x < 0 THEN EXIT SUB
    PRINT "Value is: "; x
END SUB
```

---

## FUNCTION

Functions are procedures that return a value. Assign the return value by assigning to the function name.

### Definition

```basic
FUNCTION name [(parameters)] AS type
    ' body
    name = return_value
END FUNCTION
```

### Example

```basic
FUNCTION Factorial (n AS NUMERIC) AS NUMERIC
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
FUNCTION SafeDiv (a AS NUMERIC, b AS NUMERIC) AS NUMERIC
    IF b = 0 THEN
        SafeDiv = 0
        EXIT FUNCTION
    END IF
    SafeDiv = a / b
END FUNCTION
```

---

## DECLARE

Forward-declare procedures. This is optional in RICE BASIC but supported for clarity:

```basic
DECLARE SUB MyProc (x AS NUMERIC)
DECLARE FUNCTION MyFunc (x AS NUMERIC) AS NUMERIC
```

---

## Parameters

### Pass By Reference By Default

QBasic mode is the default. Parameters are passed by reference unless `BYVAL` is explicit:

```basic
SUB Increment (x AS NUMERIC)
    x = x + 1
END SUB

DIM n AS NUMERIC
n = 10
CALL Increment(n)
PRINT n        ' 11
```

Passing a parenthesized argument to an unparenthesized call forces that argument to be evaluated as an expression and passed by value:

```basic
SUB ChangeMe (x)
    x = 42
END SUB

n = 10
ChangeMe (n)
PRINT n        ' 10
```

### Pass By Value In ANSI Mode

In ANSI Full BASIC, parameters are passed by value by default. Changes inside the procedure do not affect the original variable:

```basic
SUB TryIncrement (x AS NUMERIC)
    x = x + 1
    PRINT "Inside: "; x    ' 11
END SUB

DIM n AS NUMERIC
n = 10
CALL TryIncrement(n)
PRINT "Outside: "; n        ' 10
```

Use `BYREF` to request write-back to the caller:

```basic
SUB Increment (BYREF x AS NUMERIC)
    x = x + 1
END SUB
```

### Array Parameters

Pass arrays by using empty parentheses:

```basic
SUB PrintArray (arr() AS NUMERIC, size AS NUMERIC)
    FOR i = 1 TO size
        PRINT arr(i)
    NEXT i
END SUB
```

### Type Parameters

User-defined types can be passed to procedures:

```basic
TYPE PointType
    x AS NUMERIC
    y AS NUMERIC
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
    DIM localVar AS NUMERIC    ' Only exists inside MyProc
    localVar = 42
END SUB
' localVar does not exist here
```

### SHARED

Access global (module-level) variables from within a procedure:

```basic
DIM total AS NUMERIC
total = 100

SUB AddToTotal (amount AS NUMERIC)
    SHARED total
    total = total + amount
END SUB

CALL AddToTotal(50)
PRINT total    ' 150
```

### COMMON

Available in the default QBasic compatibility mode and unavailable in ANSI mode. Declare module-level variables that are automatically accessible inside procedures without needing `SHARED` in each sub or function:

```basic
COMMON SHARED total, count
DIM total AS NUMERIC
DIM count AS NUMERIC

SUB Increment
    total = total + 1
    count = count + 1
END SUB
```

`CHAIN` is not supported; `COMMON` is provided for single-source shared-state declarations.

### STATIC Variables

Variables declared `STATIC` retain their values between calls:

```basic
SUB Counter
    STATIC count AS NUMERIC
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

## Function Resolution Order

When RICE BASIC encounters `name(args)` in an expression, it resolves in this order:

1. **Built-in function** (e.g., `LEN`, `ABS`, `SQR`)
2. **User-defined FUNCTION** (defined with `FUNCTION...END FUNCTION`)
3. **Array access** (e.g., `myArray(index)`)

This means you cannot name a function or array the same as a built-in function.
