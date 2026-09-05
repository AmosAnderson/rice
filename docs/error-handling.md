# Error handling

Rice supports structured `WHEN EXCEPTION` handling in **both dialects** and classic `ON ERROR GOTO` handling in **QBasic mode only**. These catch runtime errors during execution, not lexer/parser errors, failures loading the main source, or errors from the definition prescan. An unhandled error ends file execution with a diagnostic and a failing process status; the REPL reports it.

## Structured handlers: both modes

```basic
WHEN EXCEPTION IN
    value = 1 / 0
    PRINT "remaining protected statement"
USE
    PRINT EXTYPE; ": "; EXTEXT$
END WHEN
PRINT "after the protected block"
```

The protected body runs until a runtime error occurs. Control then enters `USE`; if the handler finishes normally, the rest of the protected body is skipped and execution continues after `END WHEN`. If the body succeeds, `USE` is skipped. Errors raised by a handler propagate outward, where an enclosing structured handler or an eligible classic handler may catch them.

`RETRY` restarts the protected body from its beginning. It does not undo assignments, output, consumed input, or other side effects. Fix the cause or bound retries to avoid an endless loop:

```basic
divisor = 0
WHEN EXCEPTION IN
    PRINT 10 / divisor
USE
    divisor = 2
    RETRY
END WHEN
```

`CONTINUE` skips the failing **direct statement of the protected body**, then resumes the remainder under the same handler:

```basic
WHEN EXCEPTION IN
    PRINT 1 / 0
    PRINT "continued"
USE
    PRINT EXTEXT$
    CONTINUE
END WHEN
```

If the failure occurs inside a loop, `IF`, or procedure call, that entire enclosing direct statement is skipped; Rice does not resume inside the failed nested construct. A normal handler exit similarly discards the rest of the protected body. `RETRY` and `CONTINUE` are control-flow statements for handlers, not general loop controls; using them outside a handler has no useful defined recovery behavior.

### Structured exception information

`EXTYPE` (also `EXTYPE()`) returns the current structured exception code; `EXTEXT$` (also `EXTEXT$()`) returns its diagnostic text. Initially and after ordinary completion they are `0` and `""`. They do not form a permanent history of the most recent error. Copy them into variables inside `USE` when the information is needed later.

The interpreter has one shared exception-information slot. Nested handlers are supported, but do not save and restore an outer handler's information; a successful inner protected block can clear it. Early control transfers out of a handler can leave information visible until a later clear. Only inspect the registers within the handler currently dealing with the error. The parenthesized `EXTYPE`/`EXTEXT$` implementation currently ignores extra arguments after evaluating them; portable code should pass none.

| Runtime error category | `EXTYPE` |
|---|---:|
| Division by zero | 3001 |
| Subscript out of range | 3000 |
| Numeric overflow reported by Rice | 1000 |
| Type mismatch | 4001 |
| Illegal function call | 5000 |
| File not found (I/O code 53) | 8001 |
| Other mapped I/O error with code `n` | `8000 + n` |
| Other runtime error, including `ERROR n` | 9999 |

These are Rice's implemented mappings, not a complete ANSI exception taxonomy. Floating-point operations do not all raise overflow: some can produce infinity or NaN. Only actual runtime errors enter a handler.

## Classic handlers: QBasic only

```basic
10 ON ERROR GOTO 100
20 PRINT 1 / 0
30 PRINT "continued"
40 END
100 PRINT "Error "; ERR; " at line "; ERL
110 RESUME NEXT
```

This prints error code `11`, line `20`, then `continued`. Numeric and named handler labels are supported. Keep the handler out of the normal execution path with `END` or `GOTO`.

| Statement | Meaning |
|---|---|
| `ON ERROR GOTO label` | Install or replace the current handler target |
| `ON ERROR GOTO 0` | Disable handling and clear `ERR`, `ERL`, and pending resume state |
| `ERROR expression` | Raise a user error code from 1 through 255, after integer conversion |
| `RESUME` or `RESUME 0` | Retry the recorded statement |
| `RESUME NEXT` | Continue after the recorded statement |
| `RESUME label` | Continue at a label in the current statement list |

`ERR`/`ERR()` returns the classic code, initially zero. `ERL`/`ERL()` returns the numeric BASIC label of the trapped statement, or zero if it has no numeric label; it is not the physical source line number. They remain set after a successful `RESUME` until another trapped error or `ON ERROR GOTO 0`. A `WHEN EXCEPTION` handler does not update `ERR`/`ERL`, and a classic handler does not set `EXTYPE`/`EXTEXT$`.

A handler is marked active until `RESUME` or `ON ERROR GOTO 0`; another error while it is active propagates rather than re-entering it. Falling out of or jumping away from the handler does not automatically clear this state. `RESUME` without a pending error raises an error. `ON ERROR RESUME NEXT`, inline handlers, and procedure-local error stacks are not implemented.

### Classic error codes

| Runtime error category | `ERR` |
|---|---:|
| Illegal function call; general runtime error; arity/undefined-variable errors | 5 |
| Reported overflow | 6 |
| Explicit allocation failures handled by Rice | 7 |
| Undefined label | 8 |
| Subscript out of range | 9 |
| Duplicate definition | 10 |
| Division by zero | 11 |
| Type mismatch | 13 |
| File not found | 53 |
| File/directory already exists, if reported by the host | 58 |
| Input past end of file / unexpected EOF | 62 |
| Permission denied | 70 |
| Other mapped host I/O errors | 76 |
| `ERROR n` | `n` |

Mapping follows the runtime error category: for example, a Rice validation failure such as an unopened channel can report general code 5 rather than a historical QB file error number. Diagnostic wording from host filesystem errors varies by platform.

### Recovery scope and known limits

Top-level handlers and tested `GOSUB` continuations are supported. The saved resume location is an index in the current executing statement list. If an error escapes an `IF`, loop, or procedure, `ERL` and the resume target can refer to the enclosing top-level statement instead of the inner failing operation. Handler lookup also occurs in the current statement list, so a target in an unrelated nested block or procedure is not a general-purpose recovery target.

Use structured handlers close to operations that may fail, or keep classic handler targets and recovery statements at top level. Recovery does not roll back partial file writes, record reads, assignments, or output. Open handles remain available after a `GET`/`PUT` error; failed `CLOSE` flushing keeps handles available for a retry. Neither handler system provides automatic `FINALLY` cleanup.
