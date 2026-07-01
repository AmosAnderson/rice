# File I/O Guide

RICE BASIC supports ANSI X3.113-1991 file I/O syntax in ANSI mode and selected QuickBasic file syntax in QuickBasic compatibility mode.

## Opening Files

```basic
OPEN #channel: NAME filename, ORGANIZATION org, ACCESS mode
```

### File Organizations

| Organization | Description                                      |
|-------------|--------------------------------------------------|
| `SEQUENTIAL`| Sequential text access (default). Read or write in order. |
| `STREAM`    | Byte-oriented access at arbitrary positions.     |

### Access Modes

| Access    | Description                                      |
|-----------|--------------------------------------------------|
| `INPUT`   | Read only. File must exist.                      |
| `OUTPUT`  | Write only. Creates file or truncates existing.  |
| `OUTIN`   | Read and write. Creates file if needed.          |

### Channel Numbers

Channel numbers identify open files. Use `FREEFILE` to get the next available number:

```basic
f = FREEFILE
OPEN #f: NAME "data.txt", ACCESS INPUT
```

### Examples

```basic
OPEN #1: NAME "output.txt", ACCESS OUTPUT
OPEN #2: NAME "data.csv", ACCESS INPUT
OPEN #3: NAME "log.txt", ORGANIZATION SEQUENTIAL, ACCESS OUTIN
OPEN #4: NAME "data.bin", ORGANIZATION STREAM, ACCESS OUTIN
```

When ORGANIZATION is omitted, SEQUENTIAL is assumed.

### QuickBasic OPEN Syntax

In the default QBasic compatibility mode, `OPEN file$ FOR mode AS #n` is accepted:

```basic
OPEN "input.txt" FOR INPUT AS #1
OPEN "output.txt" FOR OUTPUT AS #2
OPEN "log.txt" FOR APPEND AS #3
OPEN "data.bin" FOR BINARY AS #4
OPEN "records.dat" FOR RANDOM AS #5 LEN = 32
```

Supported modes are `INPUT`, `OUTPUT`, `APPEND`, `BINARY`, and `RANDOM`. Optional `ACCESS`, `SHARED`/`LOCK`, and `LEN = n` clauses are parsed for compatibility. `LEN = n` defines the byte size for RANDOM record buffers; locking semantics are accepted but not modeled.

---

## Closing Files

```basic
CLOSE #1              ' Close a specific file
CLOSE #1, #2, #3      ' Close multiple files
CLOSE                  ' Close all open files
RESET                  ' Flush and close all open files
```

Always close files when done to ensure data is flushed to disk.

---

## File Position

```basic
SEEK #1, 10           ' Move pointer to byte 10 (1-based)
p = SEEK(1)           ' Next read/write byte position (1-based)
```

`SEEK` is equivalent to `SET #n: POINTER` for stream files; `SEEK(n)` mirrors `ASK #n: POINTER`.

---

## Sequential File Output

### PRINT #

Write formatted text to a file (same syntax as console PRINT):

```basic
OPEN #1: NAME "output.txt", ACCESS OUTPUT
PRINT #1, "Hello, World!"
PRINT #1, "x = "; x
PRINT #1, a; b; c
CLOSE #1
```

### WRITE #

Write comma-delimited output. Strings are automatically quoted, numbers are not:

```basic
OPEN #1: NAME "data.csv", ACCESS OUTPUT
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

## Sequential File Input

### INPUT #

Read comma-separated values from a file:

```basic
OPEN #1: NAME "data.csv", ACCESS INPUT
INPUT #1, name, age, height
PRINT name & " is " & STR(age) & " years old"
CLOSE #1
```

`INPUT #` correctly parses the format produced by `WRITE #`, handling quoted strings and unquoted numbers.

### LINE INPUT #

Read an entire line without parsing:

```basic
OPEN #1: NAME "text.txt", ACCESS INPUT
DO WHILE NOT EOF(1)
    LINE INPUT #1, line
    PRINT line
LOOP
CLOSE #1
```

---

## Stream I/O

### GET and PUT

Read and write binary values at specific positions using STREAM organization:

```basic
OPEN #1: NAME "data.bin", ORGANIZATION STREAM, ACCESS OUTIN

' Write data
x = 42
PUT #1: x

' Read data
DIM y AS NUMERIC
SET #1: POINTER 1
GET #1: y
PRINT y                 ' 42

CLOSE #1
```

### SET POINTER and ASK POINTER

Control and query the current file position:

```basic
SET #1: POINTER 10      ' Move to byte position 10
ASK #1: POINTER p       ' Get current position into variable p
PRINT p
```

---

## Structured Binary File I/O

`GET` and `PUT` support simple values and user-defined type values. User-defined type fields are serialized recursively in declaration order:

```basic
TYPE Person
    name AS STRING
    age AS NUMERIC
END TYPE

DIM p AS Person
p.name = "Alice"
p.age = 30

OPEN "people.dat" FOR BINARY AS #1
PUT #1, 1, p
GET #1, 1, p

CLOSE #1
```

### QuickBasic RANDOM Records

In QBasic-compatible mode, `OPEN ... FOR RANDOM ... LEN = n` creates a fixed-length record buffer. `FIELD` maps string variables onto slices of that buffer. `LSET` and `RSET` left-align or right-align values into the mapped slots. `PUT #n, record` writes the whole field buffer and `GET #n, record` reads it back:

```basic
OPEN "records.dat" FOR RANDOM AS #1 LEN = 12
FIELD #1, 5 AS name$, 3 AS code$
LSET name$ = "ALPHA"
RSET code$ = "7"
PUT #1, 1

GET #1, 1
PRINT name$; code$
CLOSE #1
```

`MKI$`, `MKL$`, `MKS$`, and `MKD$` create packed binary strings for integer, long, single, and double values. `CVI`, `CVL`, `CVS`, and `CVD` convert those packed strings back to numeric values.

`SET #n: RECORD` is not currently implemented as a distinct ANSI file feature.

---

## File Functions

### FREEFILE

Returns the next available channel number:

```basic
f1 = FREEFILE
OPEN #f1: NAME "file1.txt", ACCESS INPUT

f2 = FREEFILE
OPEN #f2: NAME "file2.txt", ACCESS INPUT
```

### EOF

Test for end-of-file. Returns the dialect true value (`-1` in QBasic mode, `1` in ANSI mode) at end of file, `0` otherwise:

```basic
OPEN #1: NAME "data.txt", ACCESS INPUT
DO WHILE NOT EOF(1)
    LINE INPUT #1, line$
    PRINT line$
LOOP
CLOSE #1
```

### LOF

Returns the length of an open file in bytes:

```basic
OPEN #1: NAME "data.txt", ACCESS INPUT
PRINT "File size:"; LOF(1); "bytes"
CLOSE #1
```

---

## Complete Example: Round-Trip File I/O

```basic
' Write structured data
OPEN #1: NAME "people.dat", ACCESS OUTPUT
WRITE #1, "Alice", 30, 65000.50
WRITE #1, "Bob", 25, 55000.00
WRITE #1, "Carol", 35, 75000.75
CLOSE #1

' Read it back
OPEN #1: NAME "people.dat", ACCESS INPUT
DO WHILE NOT EOF(1)
    INPUT #1, name, age, salary
    PRINT name & " (age " & STR(age) & "): $" & STR(salary)
LOOP
CLOSE #1
```

---

## File System Operations

Manage files and directories from your program:

```basic
MKDIR "reports"                    ' Create a directory
CHDIR "reports"                    ' Change working directory
CHDRIVE "C"                        ' Change current drive, where available
FILES "."                          ' List directory entries
KILL "old_report.txt"              ' Delete a file
NAME "draft.txt" AS "final.txt"    ' Rename a file
RMDIR "temp"                       ' Remove an empty directory
```
