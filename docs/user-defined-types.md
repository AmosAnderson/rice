# User-Defined Types

Rice BASIC supports records declared with `TYPE` in both the default QBasic mode
and ANSI mode. These are Rice's implemented semantics, not a claim that every
record feature belongs to or matches both historical standards. Numeric fields
are stored as `f64`; strings and nested records are separate runtime values.
See [Dialects](dialects.md) and [Procedures and Scope](procedures.md).

## Declaration syntax

```text
TYPE type_name
    field_name AS type
    ...
END TYPE

DIM variable AS type_name
DIM array(lower TO upper [, lower TO upper ...]) AS type_name
```

Type and field names are case-insensitive. Each field requires `AS type`.
The type body accepts field declarations only, with one field per declaration;
methods, field initializers, field arrays, inheritance, pointers, and unions are
not implemented. Empty records are accepted.

| Field declaration | Initial value | In-memory behavior |
| --- | --- | --- |
| `AS NUMERIC` | `0` | Numeric `f64` |
| `AS INTEGER`, `LONG`, `SINGLE`, `DOUBLE` | `0` | Numeric `f64`; no integer rounding or narrow-range enforcement |
| `AS STRING` | `""` | Variable-length string |
| `AS STRING * n` | `""` | Variable-length in memory; fixed width during binary record I/O |
| `AS OtherType` | A recursively initialized record | Nested fields |

All these declarations are accepted in both modes. `STRING * n` requires a
non-negative integer numeric literal within the host size range; `0` is legal.
Expressions, named constants, negative values, and fractional lengths are rejected.

Besides ordinary identifiers, these keywords may be used as field names:
`WIDTH`, `NAME`, `COLOR`, `LEN`, `TYPE`, `ERROR`, `TIMER`, `STEP`, `INPUT`, `OUTPUT`,
`OPEN`, `CLOSE`, `SHARED`, `STATIC`, `BASE`, `EXPLICIT`, `VIEW`, and `LOCATE`.
Other reserved keywords are not generally accepted as field names.

```basic
TYPE PointType
    x AS NUMERIC
    y AS NUMERIC
END TYPE

TYPE PersonType
    Name AS STRING * 20
    Age AS INTEGER
    Position AS PointType
END TYPE

DIM person AS PersonType
person.Name = "Alice"
person.Age = 30
person.Position.x = 3
person.Position.y = 4

PRINT person.Name                  ' Alice (not padded in memory)
PRINT person.Position.x            ' 3
PRINT person.Position.y            ' 4
```

Top-level type definitions are collected before execution, including definitions
inside top-level conditional/loop blocks. Referenced nested types can therefore be
defined later in the source. Definitions inside procedure bodies are not collected
as local types. Instantiating an undefined type or a directly/indirectly recursive
type reports a runtime error; records contain values, not references.

## Reading, writing, and copying

Dot notation can traverse any number of nested fields:
`person.Position.x`. Reading or assigning an unknown field reports a runtime
error. Accessing a field on a number/string reports a type mismatch. A scalar
record must first have been initialized by a declaration or whole-record
assignment; arbitrary `p.x = 1` does not create a new record schema.

Whole-record assignment copies the complete value, including nested records:

```basic
TYPE PointType
    x AS NUMERIC
    y AS NUMERIC
END TYPE
DIM first AS PointType
first.x = 3
second = first
second.x = 9
PRINT first.x; ","; second.x        ' 3,9
```

Under QBasic's `OPTION EXPLICIT`, declare `second` as well. `PRINT record` displays
`[TYPENAME]`; it does not enumerate fields. Compare individual fields when testing
record equality: expression operators such as `record1 = record2` do not support
record operands.

Field types establish defaults and binary layouts, but **field assignments are
not type-checked or coerced**. For example, assigning `2.5` to an `INTEGER` field
keeps `2.5`, assigning a string to that field also succeeds, and assigning a longer
string to `STRING * 4` does not truncate it in memory. Whole-record assignment does
not enforce the destination's declared record type either. Later arithmetic,
member access, or binary I/O may fail if a stored value no longer fits its use.

Duplicate field names are not rejected; the last field default wins in the
in-memory map, while binary layout still follows the declaration list. Duplicate
type names replace earlier schemas. Avoid both forms: duplicate/redefined schemas
are not a supported layout-versioning mechanism, especially for existing records.

## Arrays of records

Use explicit bounds to make the intended range clear. Rice currently defaults to
lower bound `1` in both dialects unless `OPTION BASE` changes it. This differs from
QBasic's usual zero base; select `OPTION BASE 0` explicitly when needed.

```basic
TYPE StudentType
    Name AS STRING
    Grade AS NUMERIC
END TYPE

DIM students(1 TO 3) AS StudentType
students(1).Name = "Alice"
students(1).Grade = 12
students(2).Name = "Bob"
students(2).Grade = 11

FOR i = 1 TO 2
    PRINT students(i).Name; " - Grade "; students(i).Grade
NEXT i
PRINT students(3).Grade             ' 0: initialized on first access
```

Each element is initialized lazily on first access, with an independent record
value. Multidimensional record arrays and nested field access on elements are
accepted, for example `grid(1, 2).Position.x` when that field is declared.

Arrays use the same flattened storage as primitive arrays. Bounds are recorded
but not enforced for ordinary element reads/writes. Record-array type and bounds
metadata is interpreter-wide, so reusing an array name in a different procedure
can affect later initialization. `REDIM ... AS TypeName` updates the record-array
type; `REDIM PRESERVE` retains existing element values without validating them
against a new type or removing out-of-range elements. `ERASE` clears element
values but retains array metadata, allowing later elements to initialize again.

`SHARED`/`COMMON` on an array base name does not make its element assignments shared.
Declared array parameters also do not bind whole arrays. These are limitations of
the shared array implementation, detailed in [Procedures and Scope](procedures.md).

## Procedure arguments and results

A scalar record variable can be passed to a `SUB` or `FUNCTION`, or to a `DEF`
function in QBasic mode.
Use explicit `BYREF` for a modifying procedure that behaves the same in both modes:

```basic
TYPE PointType
    x AS NUMERIC
    y AS NUMERIC
END TYPE

SUB MovePoint(BYREF p AS PointType, BYVAL dx, BYVAL dy)
    p.x = p.x + dx
    p.y = p.y + dy
END SUB

DIM point AS PointType
CALL MovePoint(point, 3, 4)
PRINT point.x; ","; point.y         ' 3,4
```

The runtime copies a record into the parameter and, for a variable or array-element
`BYREF` argument, copies it back on exit. Unqualified parameters default to `BYREF`
in QBasic and `BYVAL` in ANSI. `CALL MovePoint(points(1), 3, 4)` writes the changed
record back to that element, evaluating its indices once before the call.
Parenthesizing the argument as `CALL MovePoint((point), 3, 4)` or
`CALL MovePoint((points(1)), 3, 4)` prevents write-back. Nested record fields such
as `person.Position` can be read as arguments but do not receive write-back.

`AS PointType` on a parameter does not perform runtime validation. Functions can
return a record assigned to their function-name variable, but `AS PointType` on the
function does not initialize an empty record result: an unassigned result defaults
to `0` (or `""` for a `$` name).

`STATIC state AS PointType` retains a record between calls, and a shared scalar
record's field writes update its shared value. See the scope and static-storage
limitations in [Procedures and Scope](procedures.md).

## Record byte length

`LEN(record_expression)` reports the number of bytes that QBasic-mode `PUT` would
write for the current record value. This Rice extension is available in both
dialects; ANSI `GET`/`PUT` still use their separate raw-string behavior. It counts
the payload only, without record-position gaps, a RANDOM record stride, or host
structure alignment.

The expression is evaluated once. Scalar records, array elements, nested record
fields, and record-valued function results are accepted. `LEN(string)` continues
to count Unicode scalar values; `LEN(number)` remains a type mismatch.

| Declared field type | Bytes counted |
| --- | --- |
| `INTEGER` | 2 |
| `LONG`, `SINGLE` | 4 |
| `DOUBLE`, `NUMERIC` | 8 |
| `STRING * n` | Exactly `n`, regardless of the string's current length |
| `STRING` | 2-byte length header plus the current Unicode scalar count |
| Another record type | Recursively count that declared type's fields in declaration order |

Binary strings encode one byte per Unicode scalar, with characters above U+00FF
mapped to byte 255. The size therefore differs from the UTF-8 byte length.
Variable-length fields longer than 65535 characters raise illegal function call,
matching `PUT`; fixed-length fields do not have that limit. Empty records have
length zero, and checked size sums raise overflow if they exceed the host size
range. No payload is allocated or file accessed to compute the length.

```basic
TYPE Entry
    id AS INTEGER
    tag AS STRING * 4
    description AS STRING
END TYPE
DIM item AS Entry
item.description = "é🙂"
PRINT LEN(item)              ' 10: 2 + 4 + (2 + 2)
```

Length calculation checks the same stored value kinds and required fields as
binary serialization. A string assigned to a numeric field, a number assigned to
a string field, or a primitive assigned to a nested record field causes a type
mismatch. These checks do not add assignment-time type enforcement. The current
schema defines the layout: duplicate declared fields are counted repeatedly, and
redefining a type affects existing records' reported size or can cause a missing
field error. A nested record uses its parent's declared nested schema, as `PUT`
does, even if loose assignment stored a record with a different type name.

Record lengths require the interpreter's type definitions; a standalone
`BuiltinRegistry` only supports the string form of `LEN`.

## Binary records and compatibility limits

In QBasic mode, `GET` and `PUT` serialize record fields recursively in declaration
order. ANSI mode uses the separate raw-string path and does not serialize records.
An indexed record element is accepted directly, for example `PUT #1, , entries(i)`
and `GET #1, , entries(i)`. The indices run once before the file transfer. Untouched
elements use the declared record-array layout; a successfully decoded GET replaces
only the selected element. Individual record members are not GET/PUT operands.
Declared numeric kinds and fixed string lengths control the binary representation,
even though they do not restrict assignment in memory. See [File I/O](file-io.md)
for byte widths, string encoding, record positioning, and mode restrictions.

Rice does not promise native host struct alignment, pointers, an external ABI,
or interoperability with every historical QBasic record file. Recursive records,
field arrays, methods, and type-safe assignments are not implemented. Binary
compatibility beyond the explicitly documented formats and integration fixtures
remains unverified.
