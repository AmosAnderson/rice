# File I/O

Rice provides text I/O in both dialects, typed binary serialization in QBasic mode, and raw text/byte-count `GET`/`PUT` in ANSI mode. Both `OPEN` syntaxes below are accepted in **both** profiles. Historical names do not imply full ANSI or QB file-format compatibility. See [dialects](dialects.md) and [error handling](error-handling.md).

Paths are host filesystem paths, relative to the process's current working directory, not automatically to the `.bas` source directory. `CHDIR` changes that directory. File channels are integer-converted numeric expressions from 1 through 255; fractional values truncate toward zero. Opening an occupied or invalid channel raises an error.

## Opening and closing

### ANSI-style syntax

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
OPTION DIALECT "ANSI"
OPEN #1: NAME "notes.txt", ACCESS OUTPUT
PRINT #1, "Hello"
CLOSE #1
OPEN #1: NAME "notes.txt", ACCESS INPUT
LINE INPUT #1, text$
PRINT text$
CLOSE #1
```

### QB-style syntax

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

`LEN` must convert to a positive integer. It supplies a record length even with modes other than `RANDOM`; the parser does not restrict it by mode. QB `RANDOM` defaults to **128 bytes**. ANSI mode does not supply that default, even when `FOR RANDOM` is used. In QB mode an ANSI-style `ACCESS OUTIN, ORGANIZATION SEQUENTIAL` also gets the 128-byte default; omitting `ORGANIZATION` does not trigger it.

```basic
OPEN "events.txt" FOR APPEND AS #1
PRINT #1, "new event"
CLOSE #1
```

`CLOSE [#]n [, [#]n ...]` flushes and closes selected channels. `CLOSE` or `RESET` closes all. Closing a channel that is not open is silently ignored. If a flush fails, handles remain available so an error handler can retry. Normal interpreter destruction attempts to flush remaining writers, but explicit closing reports failures to the program.

## Text output

```basic
PRINT #n, [USING format$;] items
WRITE #n, expression [, expression ...]
```

`PRINT #` has the console print syntax: semicolons join values and a trailing semicolon suppresses the newline; commas move to the next 16-column zone; `TAB(n)` moves forward to a 1-based column and `SPC(n)` inserts spaces. Positions and counts truncate toward zero; negative values are errors. `TAB` at or behind the current column inserts nothing. Columns persist per channel across partial `PRINT #` statements and are separate from the console column. `WRITE #` resets that column after its newline. Seeking or binary I/O does not reset the tracked print column.

`WRITE #` separates values with commas, wraps strings in double quotes, doubles embedded quotes, and terminates the record with LF. It writes no numeric padding. `PRINT #n,` can write a blank line; `WRITE #n,` requires at least one expression. Both use UTF-8 text. File output is buffered, so use `CLOSE` or an operation documented to flush before relying on the bytes being on disk.

```basic
OPEN "people.csv" FOR OUTPUT AS #1
WRITE #1, "Alice", 30, "She said " & CHR$(34) & "hello" & CHR$(34)
PRINT #1, USING "Total: ###.##"; 12.5
CLOSE #1
```

[PRINT USING](print-using.md) describes the shared formatting engine. `MAT PRINT #` and `MAT INPUT #` are described in [MAT operations](mat-operations.md).

## Text input

```basic
INPUT #n, variable [, variable ...]
LINE INPUT #n, stringVariable
value$ = INPUT$(count, #n)
```

Targets of statement-level file input are scalar names, not array elements or record members. Declare a string with `$` or `DIM name AS STRING` before reading it. A numeric variable cannot receive text through `INPUT #`; `LINE INPUT #` requires a string value.

`INPUT #` reads exactly as many fields as there are targets. Fields may cross physical lines or be consumed by subsequent `INPUT #` statements. It handles quoted fields, doubled quotes, embedded commas/newlines inside quotes, and surrounding whitespace. Unquoted fields are trimmed. Leading spaces, tabs, CR, and LF are skipped before each field. Empty fields between commas are preserved; blank lines and a final delimiter are not a complete CSV empty-record model. Text must be valid UTF-8. This is intended for Rice `WRITE #` round-trips, not unrestricted CSV validation.

Numeric fields use Rust's `f64` text parser rather than BASIC `VAL` syntax; malformed numbers cause a type mismatch. All requested fields are consumed before conversion/assignment; a later conversion error leaves targets unchanged but advances the file position. Numeric file input currently accepts `NaN` and infinity spellings accepted by that parser, unlike interactive `INPUT`, which requires finite numbers.

`LINE INPUT #` reads through LF, removes trailing LF/CR, and preserves spaces and commas. `INPUT #` and `LINE INPUT #` raise error 62 when reading past EOF; an unfinished quoted string also raises 62.

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
| `EOF(n)` | Dialect true (`-1` QB, `1` ANSI) at EOF, otherwise 0 |
| `LOF(n)` | File length in bytes; flushes buffered output first |
| `LOC(n)` | Current **zero-based byte offset**, not a QB record count |
| `SEEK(n)` | Next byte position, 1-based; flushes output first |
| `SEEK [#]n, position` | Seek to 1-based byte position |
| `SET #n: POINTER position` | Same byte seek |
| `ASK #n: POINTER variable` | Store the 1-based byte position in a scalar variable; flushes output first |

All positioning is in bytes, even for UTF-8 text. Explicit `SEEK`/`SET POINTER` positions must convert to integers at least 1; they flush writers, reposition both reader and writer, and clear the cached EOF flag. These operations also work on sequential handles. `SET #n: RECORD` is not implemented. APPEND writes remain at the host file's end even if a seek is requested.

`EOF` peeks at a readable handle and caches its result; a write-only channel reports true. Peek errors are currently treated as EOF. `LOC` prefers the reader on read/write handles; `SEEK(n)` and `ASK POINTER` prefer the writer. Buffered reader/writer positions can diverge when operations are mixed, and their reported values are not a unified QB record pointer. Prefer an explicit `SEEK`/`SET POINTER` between reading and writing. Record-positioned `GET` does not itself clear a previously cached EOF flag. Trailing whitespace can also leave `EOF` false immediately before an `INPUT #` that finds no further field.

## GET and PUT syntax

```basic
GET #n [, position [, variable]]
PUT #n [, position [, variable]]
GET #n, , variable
PUT #n, , variable
```

The parser requires `#` and uses **commas**, not `GET #n: variable`. A variable operand must be a scalar name; whole records qualify, but array elements and individual record members do not. Omitting the position uses the current cursor. An explicit position is at least 1 and means byte offset `position - 1`, or `(position - 1) * recordLength` when `LEN`/the RANDOM default/`FIELD` supplies a record length. Seek positions from `SEEK` remain byte positions regardless of record length.

With no variable, a defined `FIELD` buffer is read/written. Without a field layout, the operation merely validates the handle and optionally seeks; it does not transfer a default record. Typed scalar/record I/O is not padded to `LEN` and does not enforce that the serialized value fits one record; the caller must choose a matching layout and length.

### QBasic typed binary serialization

`PUT` serializes the named scalar or record; `GET` deserializes the same layout. Values are little-endian, without array descriptors or automatic record padding:

| Type selection | Binary layout |
|---|---|
| Scalar `%`; `INTEGER` record field | Signed 16-bit integer, 2 bytes |
| Scalar `&`; `LONG` record field | Signed 32-bit integer, 4 bytes |
| Scalar `!`; `SINGLE` record field | IEEE 754 binary32, 4 bytes |
| Scalar `#`, unsuffixed numeric; `DOUBLE`/`NUMERIC` field | IEEE 754 binary64, 8 bytes |
| Scalar `$`; `STRING` field | Unsigned 16-bit byte count, then that many bytes (maximum 65535) |
| `STRING * n` record field | Exactly `n` bytes, truncated or NUL-padded; `GET` strips trailing NULs |
| User-defined record | Each field recursively in declaration order, using declared field types |

Scalar width selection follows the variable suffix and retained record/array type metadata, not a complete scalar declaration table. For example, `DIM n AS INTEGER` does not make unsuffixed `n` a two-byte scalar; use `n%`. An unsuffixed scalar declared `AS STRING` is likewise not a reliable binary string operand; use `s$`. Fixed-length scalar declarations do not retain the fixed binary width. Record fields retain their declared serialization types.

Integer serialization truncates fractions and saturates out-of-range values through casts; it does not raise QB integer overflow. Single serialization converts to `f32` and can lose precision or produce infinity. Binary strings map characters U+0000–U+00FF to bytes directly; characters above U+00FF become byte 255. They do not use UTF-8 or historical code-page conversion.

```basic
OPTION DIALECT "QB"
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

### ANSI raw transfers

ANSI mode does not use the typed serialization table. `PUT` writes a string as UTF-8 bytes; a numeric value is written as ordinary numeric text. `GET` always stores a string, even if the target previously held a number. It reads up to the existing nonempty string's UTF-8 byte length, otherwise up to 128 bytes, performs lossy UTF-8 decoding, and strips trailing NULs. At EOF it returns an empty string and sets EOF; short reads are allowed.

```basic
OPTION DIALECT "ANSI"
OPEN #1: NAME "raw.txt", ORGANIZATION STREAM, ACCESS OUTIN
text$ = "ABC"
PUT #1, 1, text$
text$ = SPACE$(3)
GET #1, 1, text$
PRINT text$
CLOSE #1
```

This is Rice's implemented raw transfer behavior, not a complete ANSI record/file organization facility.

## FIELD buffers: QBasic only

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

`LSET variable$ = expression$` left-aligns; `RSET` right-aligns. Both pad with spaces and truncate to the **first** `width` source bytes when too long. They update the matched buffer slot and variable. Plain assignment to a field variable does not update the buffer. `GET` without a variable reads into a space-filled buffer, updates all mapped variables, and permits short records; missing bytes stay spaces. `PUT` without a variable writes the whole buffer, including unmapped trailing bytes.

Only `$`-suffixed names qualify for `FIELD`, `LSET`, and `RSET`, even if an unsuffixed variable was declared `AS STRING`. Without a field binding, `LSET`/`RSET` use the existing variable's byte length, or the replacement length if the variable has no string value. Existing empty strings therefore stay empty. Avoid binding the same variable name in multiple channels: lookup searches a hash map and which channel is updated is unspecified. These operations use the binary string mapping above and do not implement record-to-record `LSET`.

### Packed conversion functions: both modes

| Pack | Unpack | Bytes and representation |
|---|---|---|
| `MKI$(number)` | `CVI(string$)` | 2, signed little-endian integer |
| `MKL$(number)` | `CVL(string$)` | 4, signed little-endian integer |
| `MKS$(number)` | `CVS(string$)` | 4, IEEE binary32 |
| `MKD$(number)` | `CVD(string$)` | 8, IEEE binary64 |

All take one argument. The packers use the same casts as typed serialization. Unpackers require at least the listed byte count, ignore extra bytes, and return `f64`. Use them with FIELD buffers or typed binary strings, not UTF-8 `PRINT #`/`INPUT$` for byte-preserving storage. `PUT` of a packed `$` string adds its own two-byte string-length header.

## Filesystem and host commands: both modes

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

If `FILES` receives a non-directory path or wildcard-looking argument, it lists its parent directory without filtering. Individual directory entry failures are skipped. `CHDRIVE` does not provide DOS drive emulation on non-Windows hosts. `SHELL` inherits host standard streams and interpreter-local environment overrides; command launch failures raise an error, but a nonzero child exit status is currently ignored. `ENVIRON` setting is QB-only; `ENVIRON$` reading is shared.
