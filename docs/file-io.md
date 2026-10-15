# File I/O

Rice provides text I/O, typed binary serialization, and raw BYTES transfers. Both `OPEN` syntaxes below are accepted. See the [language reference](language-reference.md) and [error handling](error-handling.md).

Paths are host filesystem paths, relative to the process's current working directory, not automatically to the `.bas` source directory. `CHDIR` changes that directory. File channels are integer-converted numeric expressions from 1 through 255; fractional values truncate toward zero. Opening an occupied or invalid channel raises an error.

## Opening and closing

### Named-channel syntax

```basic
OPEN #channel: NAME path$ [, ACCESS INPUT|OUTPUT|OUTIN] [, ORGANIZATION SEQUENTIAL|STREAM]
```

`ACCESS` defaults to `INPUT`. Clauses may occur in either order; later repeated clauses replace earlier values. Omitted organization has sequential intent, but organization is not an enforced restriction on which I/O statements may use the channel.

| Access | Host behavior |
|---|---|
| `INPUT` | Read only; file must already exist |
| `OUTPUT` | Write only; create or truncate |
| `OUTIN` | Read and write; create if missing, preserve existing contents |

```basic
OPEN #1: NAME "notes.txt", ACCESS OUTPUT
PRINT #1, "Hello"
CLOSE #1
OPEN #1: NAME "notes.txt", ACCESS INPUT
LINE INPUT #1, text$
PRINT text$
CLOSE #1
```

### FOR syntax

```basic
OPEN path$ FOR mode [ACCESS READ|WRITE|READ WRITE] [SHARED|LOCK READ|LOCK WRITE|LOCK READ WRITE] AS [#]channel [LEN = length]
```

| Mode | Behavior |
|---|---|
| `INPUT` | Sequential read only |
| `OUTPUT` | Sequential write; create or truncate |
| `APPEND` | Write at the end; create if missing; no read handle |
| `BINARY` | Read and write without truncation; byte positions unless `LEN` is supplied |
| `RANDOM` | Read and write without truncation; record positions when a record length exists |

The optional `ACCESS` and sharing/locking clauses are parsed and discarded: they do not change the access selected by `FOR`, acquire locks, or enforce sharing. The order shown is the supported clause order. The older comma-separated `OPEN "I", #1, ...` form is not implemented.

`LEN` must convert to a positive integer. It supplies a record length even with modes other than `RANDOM`; the parser does not restrict it by mode. `RANDOM` defaults to **128 bytes**. A named-channel `ACCESS OUTIN, ORGANIZATION SEQUENTIAL` also gets the 128-byte default; omitting `ORGANIZATION` does not trigger it.

```basic
OPEN "events.txt" FOR APPEND AS #1
PRINT #1, "new event"
CLOSE #1
```

`CLOSE [#]n [, [#]n ...]` flushes and closes selected channels. `CLOSE` or `RESET` closes all. Closing a channel that is not open is silently ignored. Both `CLOSE` and `RESET` report flush failures and retain all selected handles so an error handler can retry; channels close only after every selected writer flushes successfully. Normal interpreter destruction attempts to flush remaining writers, but explicit closing reports failures to the program.

## Text output

```basic
PRINT #n, [USING format$;] items
WRITE #n, expression [, expression ...]
```

`PRINT #` has the console print syntax: semicolons join values and a trailing semicolon suppresses the newline; commas move to the next 16-column zone; `TAB(n)` moves forward to a 1-based column and `SPC(n)` inserts spaces. Positions and counts truncate toward zero; negative values are errors. `TAB` at or behind the current column inserts nothing. Columns persist per channel across partial `PRINT #` statements and are separate from the console column. `WRITE #` resets that column after its newline. Seeking or binary I/O does not reset the tracked print column.

`WRITE #` separates values with commas, wraps strings in double quotes, doubles embedded quotes, and terminates the record with LF. It writes no numeric padding. `PRINT #n,` can write a blank line; `WRITE #n,` requires at least one expression. Both use UTF-8 text. File output is buffered, so use `CLOSE` or an operation documented to flush before relying on the bytes being on disk.

```basic
OPEN "people.csv" FOR OUTPUT AS #1
WRITE #1, "Alice", 30, "She said ""hello"""
PRINT #1, USING "Total: ###.##"; 12.5
CLOSE #1
```

[PRINT USING](print-using.md) describes the shared formatting engine. `MAT PRINT #channel, array` writes comma-separated numeric matrix rows with LF endings, starting at the current file position. It resets the channel's print column after each row and flushes its output. See [matrix file channels](mat-operations.md#file-channels) for dimensions, validation, and an example.

## Text input

```basic
INPUT #n, target [, target ...]
LINE INPUT #n, stringTarget
value$ = INPUT$(count, #n)
```

File-input destinations are scalar names or array elements. Members, slices, and empty subscripts are rejected. Retained declarations select conversion and defaults; strings need STRING or a $ suffix, while STRING * n pads/truncates. INPUT # rejects text for numeric destinations, LINE INPUT # requires a string destination, and both reject records.

After evaluating the channel expression, file input resolves all destination indices once from left to right, before consuming file data. Index values truncate through a checked integer conversion. Base-name declaration checks and constant checks also happen before reading, including a constant that aliases a flattened element key. Array bounds/rank are not enforced. All INPUT values are committed after successful conversion, so `INPUT #1, i, values(i)` uses the old `i` to choose its element. Unlike READ, earlier input values cannot change later target locations. Index-expression side effects are not rolled back.

`INPUT #` reads exactly as many fields as there are targets. Fields may cross physical lines or be consumed by subsequent `INPUT #` statements. It handles quoted fields, doubled quotes, embedded commas/newlines inside quotes, and surrounding whitespace. Unquoted fields are trimmed. Leading spaces, tabs, CR, and LF are skipped before each field. Empty fields between commas are preserved; blank lines and a final delimiter are not a complete CSV empty-record model. Text must be valid UTF-8. This is intended for Rice `WRITE #` round-trips, not unrestricted CSV validation.

NUMERIC/DOUBLE fields use the f64 text parser, independently of VAL syntax. INTEGER, LONG, INT64, and BYTE parse decimal integers with range checks; SINGLE converts numeric text to f32 with finite-overflow checks. BOOL accepts TRUE/FALSE/1/0, and BYTES accepts hex pairs with optional &H. All fields are consumed before conversion and assignment; a later conversion error leaves all targets unchanged but advances the file position. NUMERIC/DOUBLE file input accepts the parser's NaN/infinity spellings; interactive INPUT requires finite numeric values.

`LINE INPUT #` reads through LF, removes trailing LF/CR, and preserves spaces and commas. `INPUT #` and `LINE INPUT #` raise error 62 when reading past EOF; an unfinished quoted string also raises 62.

`MAT INPUT #channel, array` fills an existing two-dimensional numeric matrix in row-major order using the same field reader and numeric conversions as `INPUT #`. It reads exactly the matrix's cell count, independent of physical file rows, and preserves the destination's lower bounds. All requested fields are consumed before numeric conversion; a read or conversion failure leaves the matrix unchanged without rewinding the file. It emits no console prompt. The matching comma-separated `MAT PRINT #` output can be read back with the same matrix shape; see [MAT file channels](mat-operations.md#file-channels).

```basic
DIM names$(2), scores(2)
OPEN "people.csv" FOR OUTPUT AS #1
WRITE #1, "Alice", 30
WRITE #1, "Bob", 25
CLOSE #1
OPEN "people.csv" FOR INPUT AS #1
INPUT #1, names$(1), scores(1)
INPUT #1, names$(2), scores(2)
CLOSE #1
```

`INPUT$(count, #n)` reads up to `count` **bytes**, with `count >= 1`, and returns a lossy UTF-8 string. A short read or read error can return a shorter string rather than error 62. Invalid UTF-8 becomes replacement characters; it does **not** preserve arbitrary binary bytes. The `#` in its second argument is optional. See [console](console.md) for the keyboard form.

```basic
OPEN "people.csv" FOR OUTPUT AS #1
WRITE #1, "Alice", 30
WRITE #1, "Bob", 25
CLOSE #1
OPEN "people.csv" FOR INPUT AS #1
DO WHILE NOT EOF(1)
    INPUT #1, name$, age
    PRINT name$; " is "; age
LOOP
CLOSE #1
```

## Position and status

| Form | Meaning |
|---|---|
| `FREEFILE` (bare keyword only) | First unused channel in 1–255; returns 0 if none is free |
| `EOF(n)` | `-1` at EOF, otherwise `0` |
| `LOF(n)` | File length in bytes; flushes buffered output first |
| `LOC(n)` | Current **zero-based byte offset** |
| `SEEK(n)` | Next byte position, 1-based; flushes output first |
| `SEEK [#]n, position` | Seek to 1-based byte position |
| `SET #n: POINTER position` | Same byte seek |
| `ASK #n: POINTER variable` | Store the 1-based byte position in a scalar variable; flushes output first |

All positioning is in bytes, even for UTF-8 text. Explicit `SEEK`/`SET POINTER` positions must convert to integers at least 1; they flush pending output at its original position before repositioning either reader or writer, then clear the cached EOF flag. A flush failure prevents the requested seek. These operations also work on sequential handles. `SET #n: RECORD` is not implemented. APPEND writes remain at the host file's end even if a seek is requested.

`EOF` peeks at a readable handle and caches its result; a write-only channel reports true. Peek errors are currently treated as EOF. `LOC` prefers the reader on read/write handles; `SEEK(n)` and `ASK POINTER` prefer the writer. Buffered reader/writer positions can diverge when operations are mixed. Prefer an explicit `SEEK`/`SET POINTER` between reading and writing. Record-positioned `GET` does not itself clear a previously cached EOF flag. Trailing whitespace can also leave `EOF` false immediately before an `INPUT #` that finds no further field.

## GET and PUT syntax

```basic
GET #n [, position [, target]]
PUT #n [, position [, source]]
GET #n, , records(i)
PUT #n, , values%(i, j)
```

The parser requires `#` and uses **commas**, not `GET #n: variable`. Operands may be scalar names or indexed array elements, including record-valued elements. Empty subscripts, individual record members, and slices are not accepted. Omitting the position uses the current cursor. An explicit position is at least 1 and means byte offset `position - 1`, or `(position - 1) * recordLength` when `LEN`/the RANDOM default/`FIELD` supplies a record length. Seek positions from `SEEK` remain byte positions regardless of record length.

The channel, optional position, and array indices are evaluated in that order, with indices evaluated left to right exactly once. The location, current/default value, and binary layout are captured before this operation flushes, seeks, or transfers data. Index functions can inspect or use the same open channel; their side effects are retained. Invalid indices or destination checks fail before this operation moves the file cursor. GET checks the base name and flattened element key for constants and follows INPUT's declaration checks. PUT permits constant sources; indexed PUT checks the base declaration under `OPTION EXPLICIT`, while scalar PUT retains its existing declaration-check exception. No array bounds or rank checks are added.

Missing declared values use their typed defaults, including after CLEAR. Missing array elements use their declared type's default or a default record for a declared record array. An undeclared scalar PUT source defaults to numeric zero. GET assigns one complete decoded value to its captured location, so a failed typed read leaves that value unchanged even when it has consumed bytes.

With no variable, a defined `FIELD` buffer is read/written. Without a field layout, the operation merely validates the handle and optionally seeks; it does not transfer a default record. Typed scalar/record I/O is not padded to `LEN` and does not enforce that the serialized value fits one record; the caller must choose a matching layout and length.

### Native scalar values and raw byte buffers

Native `INT64`, `BOOL`/`BOOLEAN`, `BYTE`, and `BYTES` scalar or array-element
operands work. Their native representation takes
precedence over suffix shorthand. INT64 uses eight little-endian bytes;
BOOL uses one byte (`0` or `1`, with other values rejected when read); BYTE uses
one byte. These transfers preserve the native type.

Top-level BYTES PUT writes the buffer directly, without a length header. Before
GET, allocate the requested read size with BYTEBUFFER; a short read returns only
the bytes available, preserving every byte without text decoding. A zero-length
buffer requests zero bytes.

```basic
DIM payload AS BYTES
OPEN "payload.bin" FOR BINARY AS #1
payload = BYTES(0, 128, 255)
PUT #1, 1, payload
payload = BYTEBUFFER(3)
GET #1, 1, payload
PRINT HEXENCODE$(payload)           ' 0080FF
CLOSE #1
```

Native fields inside a record follow the structured table below. In
particular, BYTES fields include a four-byte length header; this differs from
raw top-level BYTES transfer.

### Typed binary serialization

`PUT` serializes the scalar or selected element; `GET` deserializes the same layout. Values are little-endian, without array descriptors or automatic record padding:

| Type selection | Binary layout |
|---|---|
| INTEGER declaration or `%` suffix | Signed 16-bit integer, 2 bytes |
| LONG declaration or `&` suffix | Signed 32-bit integer, 4 bytes |
| SINGLE declaration or `!` suffix | IEEE 754 binary32, 4 bytes |
| NUMERIC/DOUBLE declaration, `#`, or dynamic numeric value | IEEE 754 binary64, 8 bytes |
| Native INT64 scalar/element; `INT64` record field | Signed 64-bit integer, 8 bytes |
| Native BOOL scalar/element; `BOOL`/`BOOLEAN` record field | One byte, `0` or `1` |
| Native BYTE scalar/element; `BYTE` record field | One unsigned byte |
| `BYTES` record field | Unsigned 32-bit little-endian length, followed by that many bytes |
| STRING declaration or `$` suffix | Unsigned 16-bit byte count, then that many bytes (maximum 65535) |
| STRING * n declaration | Exactly n bytes; assignment pads/truncates characters before serialization; GET restores the fixed character width |
| User-defined record | Each field recursively in declaration order, using declared field types |

Retained AS metadata selects binary layouts for scalars, array elements, and fields. An explicit AS overrides suffix shorthand: DIM n AS INTEGER transfers two bytes, DIM n% AS DOUBLE transfers eight, and AS STRING * n transfers exactly n bytes. Without an explicit declaration, suffixes or the actual typed value select the layout.

Array-element transfers use the same declared types and layouts as scalars, taking suffix shorthand from the base name. Untouched elements use declared defaults, including records and fixed strings. Array syntax names storage here; it never calls a same-named builtin or user function.

Integer serialization requires exact in-range values; SINGLE conversion rejects finite overflow. Binary strings map Unicode U+0000–U+00FF directly to bytes and clamp higher characters to 255. They do not use UTF-8; use BYTES and explicit encoding for UTF-8 data.

```basic
TYPE Person
    name AS STRING * 12
    age AS INTEGER
END TYPE
DIM person AS Person
person.name = "Alice"
person.age = 30
OPEN "person.bin" FOR BINARY AS #1
PUT #1, 1, person
GET #1, 1, person
PRINT person.name; ": "; person.age
CLOSE #1
```

Typed `GET` requires every byte of the selected layout and reports an I/O error on truncation. It assigns the target only after successful decoding, but the file position can have advanced on failure. `PUT` can have written earlier fields before a later field fails. Neither is a transactional operation.

## FIELD buffers

```basic
OPEN "records.dat" FOR RANDOM AS #1 LEN = 12
FIELD #1, 5 AS name$, 3 AS code$
LSET name$ = "ALPHA"
RSET code$ = "7"
PUT #1, 1
GET #1, 1
PRINT name$; "|"; code$
CLOSE #1
```

`FIELD [#]n, width AS variable$ [, ...]` defines consecutive byte slices of a channel buffer. Widths truncate toward zero and must be nonnegative; the total cannot exceed the record length. A channel without a record length gets `MAX(totalWidth, 1)`; the implementation does not require it to have been opened `FOR RANDOM`. New buffers are space-filled. A later `FIELD` replaces that channel's layout.

`LSET variable$ = expression$` left-aligns; `RSET` right-aligns. Both pad with spaces and truncate to the **first** `width` source bytes when too long. They update the matched buffer slot and variable. Plain assignment to a field variable does not update the buffer. `GET` without a variable reads a complete record across internal buffer boundaries before updating all mapped variables. A final record shortened by EOF is permitted; missing bytes stay spaces. `PUT` without a variable writes the whole buffer, including unmapped trailing bytes.

FIELD, LSET, and RSET accept $-suffixed names and variables explicitly declared AS STRING or STRING * n. Without a field binding, `LSET`/`RSET` use the existing variable's byte length, or the replacement length if the variable has no string value. Existing empty strings therefore stay empty. Avoid binding the same variable name in multiple channels: lookup searches a hash map and which channel is updated is unspecified. These operations use the binary string mapping above and do not implement record-to-record `LSET`.

### Packed conversion functions

| Pack | Unpack | Bytes and representation |
|---|---|---|
| `MKI$(number)` | `CVI(string$)` | 2, signed little-endian integer |
| `MKL$(number)` | `CVL(string$)` | 4, signed little-endian integer |
| `MKS$(number)` | `CVS(string$)` | 4, IEEE binary32 |
| `MKD$(number)` | `CVD(string$)` | 8, IEEE binary64 |

All packed conversion functions take one argument. MKI$/MKL$ require exact integers in range, and MKS$ rejects finite overflow. CVI, CVL, and CVS return INTEGER, LONG, and SINGLE; CVD returns NUMERIC. Unpackers require the specified byte count and ignore trailing bytes. Use packed strings with binary I/O; PUT of a variable STRING adds its own two-byte length header.

## Filesystem and host commands

| Statement | Behavior |
|---|---|
| `KILL path$` | Remove one file; no wildcard expansion |
| `NAME old$ AS new$` | Host rename; overwrite/cross-filesystem behavior depends on the OS |
| `MKDIR path$` | Create one directory; does not recursively create parents |
| `RMDIR path$` | Remove an empty directory |
| `CHDIR path$` | Change process working directory |
| `CHDRIVE drive$` | Use the first character to construct a Windows-style `C:\` path and change directory; an empty string does nothing |
| `FILES [path$]` | List directory names, default `.`; no sorting or wildcard filtering |
| `SHELL [command$]` | Synchronously invoke `sh -c` on non-Windows or `cmd /c` on Windows; no argument does nothing |

If `FILES` receives a non-directory path or wildcard-looking argument, it lists its parent directory without filtering. Individual directory entry failures are skipped. `CHDRIVE` does not provide DOS drive emulation on non-Windows hosts. `SHELL` inherits host standard streams and interpreter-local environment overrides; command launch failures raise an error, but a nonzero child exit status is currently ignored. `ENVIRON` sets interpreter-local overrides; `ENVIRON$` reads environment values.
