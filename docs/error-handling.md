# Error Handling

RICE BASIC provides structured error handling through `ON ERROR GOTO`, `RESUME`, and the `ERR`/`ERL` functions.

## Setting Up an Error Handler

### ON ERROR GOTO

Establish an error handler by specifying a label to jump to when a runtime error occurs:

```basic
ON ERROR GOTO errorHandler

' Code that might cause errors
OPEN "nonexistent.txt" FOR INPUT AS #1
PRINT "This line runs if no error"
END

errorHandler:
PRINT "Error"; ERR; "occurred at line"; ERL
RESUME NEXT
```

### ON ERROR GOTO 0

Disable the current error handler. Any subsequent errors will terminate the program:

```basic
ON ERROR GOTO handler
' Protected code...
ON ERROR GOTO 0
' Unprotected code - errors are fatal here
```

---

## Resuming After an Error

Within an error handler, use `RESUME` to continue execution:

### RESUME

Retry the statement that caused the error:

```basic
ON ERROR GOTO retryHandler
OPEN filename$ FOR INPUT AS #1
END

retryHandler:
filename$ = "backup.txt"
RESUME    ' Try the OPEN statement again with the new filename
```

### RESUME NEXT

Skip the statement that caused the error and continue with the next one:

```basic
ON ERROR GOTO skipHandler

x = 1 / 0         ' Division by zero - will be skipped
PRINT "Continued"  ' This runs after RESUME NEXT
END

skipHandler:
PRINT "Skipping error"; ERR
RESUME NEXT
```

### RESUME label

Jump to a specific label after handling the error:

```basic
ON ERROR GOTO handler

x = 1 / 0
PRINT "Skipped"

safePoint:
PRINT "Resumed at safe point"
END

handler:
RESUME safePoint
```

---

## Error Information

### ERR

Returns the error code of the most recent error. Returns `0` when no error has occurred.

### ERL

Returns the line number where the error occurred. Returns `0` if no error has occurred.

### Example

```basic
ON ERROR GOTO handler
10 x = 1 / 0
20 PRINT "OK"
END

handler:
PRINT "Error code:"; ERR
PRINT "Error line:"; ERL
RESUME NEXT
```

---

## Error Codes

RICE BASIC uses QBasic-compatible error codes:

| Code | Description              |
|------|--------------------------|
| 1    | NEXT without FOR         |
| 3    | RETURN without GOSUB     |
| 5    | Illegal function call    |
| 6    | Overflow                 |
| 8    | Undefined label          |
| 9    | Subscript out of range   |
| 10   | Duplicate definition     |
| 11   | Division by zero         |
| 13   | Type mismatch            |
| 20   | RESUME without error     |

---

## Error Handling Patterns

### Graceful File Open

```basic
ON ERROR GOTO fileError
OPEN "config.txt" FOR INPUT AS #1
ON ERROR GOTO 0    ' Disable handler after successful open

' Process file...
LINE INPUT #1, setting$
CLOSE #1
END

fileError:
PRINT "Could not open config.txt, using defaults"
RESUME NEXT
```

### Error Logging

```basic
ON ERROR GOTO logError

' Various operations...
x = SQR(-1)
PRINT "After error"
END

logError:
PRINT "ERROR"; ERR; "at line"; ERL
RESUME NEXT
```

### Retry Logic

```basic
DIM attempts AS INTEGER
ON ERROR GOTO retryOpen

tryOpen:
attempts = attempts + 1
OPEN "data.txt" FOR INPUT AS #1
PRINT "File opened successfully"
END

retryOpen:
IF attempts < 3 THEN
    PRINT "Attempt"; attempts; "failed, retrying..."
    RESUME tryOpen
ELSE
    PRINT "Failed after 3 attempts"
    END
END IF
```
