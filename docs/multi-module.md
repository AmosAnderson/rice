# Multi-Module Programming

`CHAIN` is not supported in RICE BASIC.

`COMMON` is supported as a single-source shared-variable declaration. `COMMON [SHARED] name[, name...]` declares module-level variables that are visible inside `SUB` and `FUNCTION` procedures without requiring `SHARED` in each procedure.

```basic
COMMON SHARED total, count
DIM total AS NUMERIC
DIM count AS NUMERIC

SUB Increment
    total = total + 1
    count = count + 1
END SUB
```

For organizing large programs, use:

- **SUB** and **FUNCTION** definitions to modularize code (see [Procedures and Scope](procedures.md))
- **SHARED** or **COMMON** variables for data accessible across procedures
- **User-defined types** to group related data (see [User-Defined Types](user-defined-types.md))
