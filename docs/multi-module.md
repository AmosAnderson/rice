# Multi-Module Programming

Multi-module programming (CHAIN, COMMON) is not supported in RICE BASIC.

The ANSI X3.113-1991 Full BASIC standard does not include the CHAIN and COMMON statements. Programs should be structured using SUB and FUNCTION procedures within a single source file.

For organizing large programs, use:

- **SUB** and **FUNCTION** definitions to modularize code (see [Procedures and Scope](procedures.md))
- **SHARED** variables for data accessible across procedures
- **User-defined types** to group related data (see [User-Defined Types](user-defined-types.md))
