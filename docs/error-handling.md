# Error Handling

RICE BASIC provides structured error handling through the ANSI X3.113-1991 `WHEN EXCEPTION` construct.

## WHEN EXCEPTION

The `WHEN EXCEPTION` block establishes a protected region of code with an associated exception handler:

```basic
WHEN EXCEPTION IN
    ' Code that might cause errors
    OPEN #1: NAME "nonexistent.txt", ACCESS INPUT
    PRINT "This line runs if no error"
USE
    PRINT "Error occurred: "; EXTEXT$
END WHEN
```

If any statement in the `WHEN EXCEPTION IN` block raises an error, control transfers immediately to the `USE` block. If no error occurs, the `USE` block is skipped entirely.

---

## Exception Information

### EXTYPE

Returns the numeric exception type code for the most recent exception. Returns `0` when no exception has occurred.

```basic
WHEN EXCEPTION IN
    x = 1 / 0
USE
    PRINT "Exception type:"; EXTYPE
END WHEN
```

### EXTEXT$

Returns a descriptive text string for the most recent exception:

```basic
WHEN EXCEPTION IN
    OPEN #1: NAME "missing.txt", ACCESS INPUT
USE
    PRINT "Error: "; EXTEXT$
END WHEN
```

---

## RETRY

Within a `USE` block, `RETRY` re-executes the statement that caused the exception. This is useful when the handler can correct the condition that caused the error:

```basic
DIM filename AS STRING
filename = "primary.txt"

WHEN EXCEPTION IN
    OPEN #1: NAME filename, ACCESS INPUT
    PRINT "File opened successfully"
USE
    IF filename = "primary.txt" THEN
        filename = "backup.txt"
        RETRY
    ELSE
        PRINT "Could not open any file"
    END IF
END WHEN
```

---

## CONTINUE

Within a `USE` block, `CONTINUE` skips the statement that caused the exception and resumes execution with the next statement in the protected block:

```basic
WHEN EXCEPTION IN
    x = 1 / 0         ' Division by zero - will be skipped
    PRINT "Continued"  ' This runs after CONTINUE
USE
    PRINT "Skipping error: "; EXTEXT$
    CONTINUE
END WHEN
```

---

## Nested Exception Handling

`WHEN EXCEPTION` blocks can be nested. Each block has its own handler:

```basic
WHEN EXCEPTION IN
    PRINT "Outer protected block"
    WHEN EXCEPTION IN
        x = SQR(-1)
    USE
        PRINT "Inner handler caught: "; EXTEXT$
    END WHEN
    PRINT "Back in outer block"
USE
    PRINT "Outer handler caught: "; EXTEXT$
END WHEN
```

---

## Error Handling Patterns

### Graceful File Open

```basic
WHEN EXCEPTION IN
    OPEN #1: NAME "config.txt", ACCESS INPUT

    ' Process file...
    LINE INPUT #1: line
    CLOSE #1
USE
    PRINT "Could not open config.txt, using defaults"
END WHEN
```

### Retry Logic

```basic
DIM attempts AS NUMERIC
attempts = 0

WHEN EXCEPTION IN
    attempts = attempts + 1
    OPEN #1: NAME "data.txt", ACCESS INPUT
    PRINT "File opened successfully"
USE
    IF attempts < 3 THEN
        PRINT "Attempt"; attempts; "failed, retrying..."
        RETRY
    ELSE
        PRINT "Failed after 3 attempts"
    END IF
END WHEN
```

### Logging Errors

```basic
WHEN EXCEPTION IN
    x = SQR(-1)
    PRINT "After error"
USE
    PRINT "Exception type:"; EXTYPE; " - "; EXTEXT$
    CONTINUE
END WHEN
```
