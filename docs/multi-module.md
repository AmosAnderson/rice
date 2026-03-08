# Multi-Module Programming

RICE BASIC supports multi-module programming through the `CHAIN` and `COMMON` statements, allowing you to split large programs across multiple files.

## CHAIN

Load and execute another BASIC program:

```basic
CHAIN "nextprogram.bas"
```

When `CHAIN` is executed:
1. The current program stops
2. The specified program is loaded and executed
3. Variables declared with `COMMON` are transferred to the new program

---

## COMMON

Declare variables that should be passed between programs linked by `CHAIN`.

### Unnamed COMMON Block

Variables are transferred by position. The first `COMMON` in the current program matches the first `COMMON` in the chained program:

**program1.bas:**
```basic
COMMON x AS INTEGER, name$ AS STRING
x = 42
name$ = "Hello"
CHAIN "program2.bas"
```

**program2.bas:**
```basic
COMMON a AS INTEGER, greeting$ AS STRING
PRINT a           ' 42
PRINT greeting$   ' Hello
```

The variable names don't need to match — values are transferred by position within the block.

### Named COMMON Blocks

Use named blocks to match variables by block name rather than position:

**program1.bas:**
```basic
COMMON /settings/ width AS INTEGER, height AS INTEGER
COMMON /data/ total AS DOUBLE

width = 80
height = 25
total = 1234.56

CHAIN "program2.bas"
```

**program2.bas:**
```basic
COMMON /data/ sum AS DOUBLE
COMMON /settings/ w AS INTEGER, h AS INTEGER

PRINT sum       ' 1234.56
PRINT w; h      ' 80  25
```

Named blocks match by name regardless of the order they appear in the source.

### COMMON SHARED

Mark `COMMON` variables as `SHARED` so they are accessible inside SUB and FUNCTION definitions:

```basic
COMMON SHARED /config/ maxItems AS INTEGER

SUB ProcessItems
    ' maxItems is accessible here because of SHARED
    FOR i = 1 TO maxItems
        PRINT i
    NEXT i
END SUB
```

---

## Example: Multi-Module Program

**main.bas:**
```basic
COMMON SHARED /appdata/ username AS STRING, score AS INTEGER

username = "Player1"
score = 0

PRINT "Welcome, "; username
PRINT "Starting game..."

CHAIN "game.bas"
```

**game.bas:**
```basic
COMMON SHARED /appdata/ username AS STRING, score AS INTEGER

PRINT username; "'s game is running"
score = score + 100
PRINT "Score: "; score

CHAIN "results.bas"
```

**results.bas:**
```basic
COMMON SHARED /appdata/ username AS STRING, score AS INTEGER

PRINT "Final Results for "; username
PRINT "Score: "; score
```

---

## Notes

- Only variables listed in `COMMON` are transferred; all other variables are reset in the new program
- Array transfer via `COMMON` follows the same positional/named block rules
- `CHAIN` loads the file relative to the current working directory
- A chained program can `CHAIN` back to the original or to another program
