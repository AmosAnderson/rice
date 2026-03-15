# File I/O Guide

## Opening Files

```basic
OPEN filename$ FOR mode AS #filenum [LEN = reclen]
```

### File Modes

| Mode     | Description                                      |
|----------|--------------------------------------------------|
| `INPUT`  | Read text. File must exist.                      |
| `OUTPUT` | Write text. Creates file or truncates existing.  |
| `APPEND` | Write text. Creates file or appends to existing. |
| `BINARY` | Read/write raw bytes at arbitrary positions.     |
| `RANDOM` | Fixed-length record access. Supports FIELD.      |

### File Numbers

File numbers range from 1 to 255. Use `FREEFILE` to get the next available number:

```basic
f = FREEFILE
OPEN "data.txt" FOR INPUT AS #f
```

### Examples

```basic
OPEN "output.txt" FOR OUTPUT AS #1
OPEN "data.csv" FOR INPUT AS #2
OPEN "log.txt" FOR APPEND AS #3
OPEN "record.dat" FOR BINARY AS #4
```

---

## Closing Files

```basic
CLOSE #1              ' Close a specific file
CLOSE #1, #2, #3      ' Close multiple files
CLOSE                  ' Close all open files
```

Always close files when done to ensure data is flushed to disk.

---

## Text Output

### PRINT #

Write formatted text to a file (same syntax as console PRINT):

```basic
OPEN "output.txt" FOR OUTPUT AS #1
PRINT #1, "Hello, World!"
PRINT #1, "x = "; x
PRINT #1, a; b; c
CLOSE #1
```

### WRITE #

Write comma-delimited output. Strings are automatically quoted, numbers are not:

```basic
OPEN "data.csv" FOR OUTPUT AS #1
WRITE #1, "Alice", 30, 5.5
WRITE #1, "Bob", 25, 6.1
CLOSE #1
```

Produces:

```
"Alice",30,5.5
"Bob",25,6.1
```

This format is designed to be read back with `INPUT #`.

---

## Text Input

### INPUT #

Read comma-separated values from a file:

```basic
OPEN "data.csv" FOR INPUT AS #1
INPUT #1, name$, age%, height!
PRINT name$; " is "; age%; " years old"
CLOSE #1
```

`INPUT #` correctly parses the format produced by `WRITE #`, handling quoted strings and unquoted numbers.

### LINE INPUT #

Read an entire line without parsing:

```basic
OPEN "text.txt" FOR INPUT AS #1
DO WHILE NOT EOF(1)
    LINE INPUT #1, line$
    PRINT line$
LOOP
CLOSE #1
```

---

## Binary I/O

### GET and PUT

Read and write raw bytes at specific positions in binary mode:

```basic
OPEN "data.bin" FOR BINARY AS #1

' Write data
x% = 42
PUT #1, 1, x%          ' Write at position 1

' Read data
DIM y AS INTEGER
GET #1, 1, y            ' Read from position 1
PRINT y                 ' 42

CLOSE #1
```

Position is 1-based (first byte is position 1).

### FIELD

For RANDOM-mode files, the `FIELD` statement defines named string variables that map to portions of the record buffer:

```basic
OPEN "records.dat" FOR RANDOM AS #1 LEN = 40
FIELD #1, 20 AS name$, 2 AS age$, 8 AS salary$

' Write a record using LSET/RSET and PUT
LSET name$ = "Alice"
LSET age$ = MKI$(30)
LSET salary$ = MKD$(65000.50)
PUT #1, 1

' Read a record using GET
GET #1, 1
PRINT name$
PRINT CVI(age$)
PRINT CVD(salary$)
CLOSE #1
```

The total width of all fields must not exceed the record length specified in OPEN.

### SEEK Statement

Set the current file position for the next read or write:

```basic
SEEK #1, 10    ' Move to byte position 10 (1-based)
```

The `SEEK` function returns the current file position:

```basic
PRINT SEEK(1)  ' Current position of file #1
```

For RANDOM-mode files, SEEK returns the record number rather than the byte position.

### Binary Conversion Functions

Convert between numeric types and binary strings for file I/O:

```basic
' Encoding
s$ = MKI$(32767)     ' INTEGER to 2-byte string
s$ = MKL$(100000)    ' LONG to 4-byte string
s$ = MKS$(3.14)      ' SINGLE to 4-byte string
s$ = MKD$(3.14159)   ' DOUBLE to 8-byte string

' Decoding
n% = CVI(s$)         ' 2-byte string to INTEGER
n& = CVL(s$)         ' 4-byte string to LONG
n! = CVS(s$)         ' 4-byte string to SINGLE
n# = CVD(s$)         ' 8-byte string to DOUBLE
```

---

## File Functions

### FREEFILE

Returns the next available file number:

```basic
f1 = FREEFILE
OPEN "file1.txt" FOR INPUT AS #f1

f2 = FREEFILE
OPEN "file2.txt" FOR INPUT AS #f2
```

### EOF

Test for end-of-file. Returns `-1` (true) at end of file, `0` (false) otherwise:

```basic
OPEN "data.txt" FOR INPUT AS #1
DO WHILE NOT EOF(1)
    LINE INPUT #1, line$
    PRINT line$
LOOP
CLOSE #1
```

### LOF

Returns the length of an open file in bytes:

```basic
OPEN "data.txt" FOR INPUT AS #1
PRINT "File size:"; LOF(1); "bytes"
CLOSE #1
```

### LOC

Returns the current position in the file:

```basic
PRINT LOC(1)    ' Current position in file #1
```

---

## Complete Example: Round-Trip File I/O

```basic
' Write structured data
OPEN "people.dat" FOR OUTPUT AS #1
WRITE #1, "Alice", 30, 65000.50
WRITE #1, "Bob", 25, 55000.00
WRITE #1, "Carol", 35, 75000.75
CLOSE #1

' Read it back
OPEN "people.dat" FOR INPUT AS #1
DO WHILE NOT EOF(1)
    INPUT #1, name$, age%, salary#
    PRINT name$; " (age "; age%; "): $"; salary#
LOOP
CLOSE #1
```

## Complete Example: Binary File

```basic
' Write binary data
OPEN "values.bin" FOR BINARY AS #1
FOR i = 1 TO 10
    PUT #1, , i    ' Write integers sequentially
NEXT i
CLOSE #1

' Read binary data
OPEN "values.bin" FOR BINARY AS #1
DIM v AS INTEGER
FOR i = 1 TO 10
    GET #1, , v
    PRINT v;
NEXT i
CLOSE #1
' Output: 1 2 3 4 5 6 7 8 9 10
```

---

## File System Operations

Manage files and directories from your program:

```basic
MKDIR "reports"                    ' Create a directory
CHDIR "reports"                    ' Change working directory
KILL "old_report.txt"              ' Delete a file
NAME "draft.txt" AS "final.txt"    ' Rename a file
RMDIR "temp"                       ' Remove an empty directory
```
