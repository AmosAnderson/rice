# MAT Operations

Rice implements the matrix operations below.
See [Language reference](language-reference.md) and [Language Reference](language-reference.md).

## Syntax and dimensions

```text
MAT target = source
MAT target = left + right
MAT target = left - right
MAT target = left * right
MAT target = (scalar_expression) * source
MAT target = ZER
MAT target = CON
MAT target = IDN
MAT target = TRN(source)
MAT target = INV(source)
MAT PRINT [#channel,] array
MAT READ array
MAT INPUT [#channel,] array
```

Each statement names one target array. Operands are bare array names, not indexed
elements. Matrix expressions cannot be chained or nested: use intermediate arrays
for `A + B + C`, `INV(A) * B`, and similar calculations. Scalar multiplication
requires parentheses around the scalar expression and places it before the array.
Dimension arguments such as `ZER(3, 3)`, scalar addition, elementwise division,
matrix powers, and `MAT PRINT USING` are not implemented.

**Only two-dimensional arrays are supported.** A one-dimensional array is not
automatically treated as a column vector. Declare a vector as
`DIM v(1 TO n, 1 TO 1)` or a row vector as `DIM v(1 TO 1, 1 TO n)`.

Bounds are inclusive. Rice currently defaults to lower bound `1`,
so `DIM a(2, 2)` means a 2-by-2 matrix. `OPTION BASE 0` makes the same declaration
3-by-3. Use explicit bounds to make the matrix shape clear:

```basic
DIM a(1 TO 2, 1 TO 2)
MAT a = IDN
MAT PRINT a
' 1 0
' 0 1
```

Two-dimensional `DIM`/`REDIM` metadata defines the matrix's shape even before any
elements have been assigned. Unassigned cells are zero. Explicit lower bounds,
including zero and negative bounds, are preserved when reading and storing a
dimensioned matrix.

Small matrices can use bounds at either end of the signed 64-bit index range.
Stored cells outside declared bounds are ignored by MAT extraction. Inferred
dimensions and destination indices that exceed the supported index range raise
an overflow error; a failed destination-index check leaves existing cells intact.

## Initialization and copying

| Statement | Result and requirements |
| --- | --- |
| `MAT a = ZER` | Fills the existing matrix shape with zeroes |
| `MAT a = CON` | Fills the existing matrix shape with ones |
| `MAT a = IDN` | Fills a square shape with diagonal ones and other cells zero |
| `MAT b = a` | Copies the source matrix's values |

`ZER`, `CON`, `IDN`, `MAT READ`, and `MAT INPUT` require a known two-dimensional
shape. Normally establish it with `DIM`. When no usable dimension metadata exists,
Rice can infer a rectangle from existing two-index elements, starting at the
current `OPTION BASE`; this fallback is not general implicit array dimensioning.

An assignment that reads a source matrix computes the source before writing the
target, so operations such as `MAT a = TRN(a)` do not overwrite unread input.
However, target bounds are **not resized** to match the result. Dimension every
destination to the intended result shape, including when transposing in place.

## Arithmetic

Addition and subtraction require equal row and column counts. They pair elements
by relative position, so operands need not use the same numeric lower bounds.
Scalar multiplication multiplies each cell by one numeric expression.

```basic
DIM a(1 TO 2, 1 TO 2)
DIM b(1 TO 2, 1 TO 2)
DIM c(1 TO 2, 1 TO 2)
DATA 1, 2, 3, 4
MAT READ a
MAT b = CON
MAT c = a + b
MAT PRINT c
' 2 3
' 4 5
MAT c = (2) * a
MAT PRINT c
' 2 4
' 6 8
```

`MAT c = a * b` is matrix multiplication, not elementwise multiplication. For
`a` of shape m-by-n and `b` of shape n-by-p, the result is m-by-p, with each
cell equal to the dot product of the corresponding row and column. A dimension
mismatch reports a runtime error.

`TRN(a)` swaps rows and columns. `INV(a)` requires a nonempty square matrix.
Inversion uses Gaussian elimination with partial pivoting and `f64` arithmetic.
An exactly zero pivot reports a singular-matrix error. There is no tolerance-based
near-singularity test, so an ill-conditioned matrix can produce inaccurate values
without an error.

### Determinants

`DET(matrix)` and `DET(matrix())` independently compute a matrix's determinant in Rice. The argument is a bare matrix name, optionally
followed by empty parentheses; indexed elements, additional arguments, and other
expressions are rejected without evaluating them. A same-named scalar variable or
function is not read or called to resolve this matrix reference.

The matrix must have a nonempty square shape. Two-dimensional DIM metadata or the
usual local-element inference defines that shape; existing MAT lower-bound and
numeric-extraction rules apply. `OPTION EXPLICIT` also checks the array's base
name. Calculation uses Gaussian elimination with partial pivoting and `f64`
arithmetic; an exactly zero pivot returns determinant `0`. No tolerance is applied
to small pivots. The input cells, array metadata, and saved determinant are unchanged.
As with inversion, ill-conditioned matrices and non-finite values have no special
accuracy guarantee; arithmetic can overflow or produce NaN.

```basic
DIM a(1 TO 2, 1 TO 2)
DATA 1, 2, 2, 4
MAT READ a
PRINT DET(a)                 ' 0: singular matrices are accepted
PRINT DET(a())               ' Same calculation
```

After a successful `MAT target = INV(source)`, `DET` or `DET()` reports that
source matrix's saved determinant. It is initially `0`; a failed inverse, including
a rejected destination assignment, leaves the previous value, and `CLEAR` does not reset it. Calling `DET(matrix)` does not
update this saved value. An ordinary variable named `DET` shadows the bare form;
`DET()` explicitly requests the saved value.

## Matrix I/O

`MAT PRINT a` writes one row per line, with single spaces between numeric cells
and ordinary Rice number formatting. A one-column two-dimensional matrix therefore
prints one value per line. There is no comma-zone alignment or trailing separator
option.

`MAT READ a` consumes the global `DATA` stream in row-major order: all columns of
the first row, then the next row. It shares its cursor with ordinary `READ` and
`RESTORE`. Numeric DATA values are used directly; string DATA values are parsed as
numbers, and an unparseable string becomes `0`. Empty DATA fields also become `0`.
On insufficient DATA, the statement errors after consuming available items but
does not store a partial matrix.

```basic
DIM a(1 TO 2, 1 TO 3)
DATA 10, 20, 30, 40, 50, 60
MAT READ a
MAT PRINT a
' 10 20 30
' 40 50 60
```

`MAT INPUT a` prompts `? ` once per row and reads a line of comma-separated numeric
values for that row. Missing, empty, invalid, or EOF values become `0`; extra values
on the row are ignored. There is no reprompt for malformed input and no quoted CSV
string handling.

### File channels

`MAT PRINT #channel, array` and `MAT INPUT #channel, array` use an open file in Rice. The channel expression is evaluated once before matrix/handle
validation. Neither form prompts, reads console input, or writes console output.
The matrix must have a usable two-dimensional shape, using the same DIM metadata
or local-element inference as the other MAT operations.

File `MAT PRINT` writes comma-separated numeric cells in row-major order, with
one LF-terminated row per matrix row and ordinary Rice number formatting.
It begins at the current file position, without inserting an initial newline:
a preceding `PRINT #channel, "prefix";` shares the first row's line. Each row
resets that channel's tracked print column, and the statement flushes its output.
The channel must be writable; OUTPUT, APPEND, and readable/writable modes follow
their ordinary file-position rules. Console `MAT PRINT` continues to use spaces.

File `MAT INPUT` reads exactly rows-times-columns numeric fields using the same
field reader as `INPUT #`. Commas and line endings separate fields; quoted
numeric fields and fields spanning different physical rows are accepted.
It does not infer or resize the matrix dimensions from the file. It stores cells
by relative row/column position, preserving the destination's own lower bounds.
Additional fields remain available for later input statements.

The channel must be readable. Missing fields or an unfinished quoted field raise
error 62; malformed numeric fields raise a type mismatch. Numeric conversion uses
the same parser as scalar file `INPUT`, including its NaN/infinity support.
All requested fields are read before numeric conversion, so a conversion failure
consumes the entire requested matrix's fields. A read failure, such as an unfinished
quoted field, can stop earlier. The destination matrix is replaced only after
every read and conversion succeeds; failures leave its values unchanged but do
not rewind consumed file data. A successful read updates the file's EOF status.
As with ordinary file input, mixed buffered reads/writes
on one handle retain the limitations described in [File I/O](file-io.md).

File MAT operations check the base name under `OPTION EXPLICIT`. File `MAT INPUT`
also rejects a constant base name or a constant alias of any destination's
flattened key before consuming fields. Invalid dimensions and unavailable
read/write handles fail before transfer. Channel-expression side effects are
not rolled back. These checks do not change existing console MAT behavior.

```basic
DIM a(1 TO 2, 1 TO 2), b(-1 TO 0, 3 TO 4)
DATA 1, 2, 3, 4
MAT READ a
OPEN "matrix.csv" FOR OUTPUT AS #1
MAT PRINT #1, a                  ' File rows: 1,2 and 3,4
CLOSE #1
OPEN "matrix.csv" FOR INPUT AS #1
MAT INPUT #1, b
CLOSE #1
PRINT b(-1, 3); ","; b(0, 4)    ' 1,4
```

## Storage limitations

MAT uses the interpreter's existing flattened array storage:

- Source values are gathered from the current procedure's local element keys.
  A matrix present only in a caller environment is not read through ordinary
  parent-scope lookup. Array parameters and shared array names do not remedy this;
  see [Procedures and Scope](procedures.md).
- String and record-valued cells are silently treated as zero during matrix
  extraction. MAT does not implement string matrices or record matrices.
- Storing a result replaces the target's local keys beginning with `target_`.
  Avoid separate scalar names that collide with flattened array element names.
- The store does not declare the target or update its bounds/type metadata. An
  undimensioned destination can hold a result, but `LBOUND`/`UBOUND` cannot infer
  metadata from it, and `OPTION EXPLICIT` can reject later ordinary element access.
- If a destination was dimensioned with the wrong shape, the result is stored but
  later MAT reads use its old bounds, potentially truncating it or padding with
  zeroes. Out-of-bound stored cells can still be read by ordinary array indexing.
- An existing target uses its own lower bounds. An undimensioned target uses the
  current `OPTION BASE`, so copying does not implicitly transfer source bounds.
- File MAT paths perform the declaration/constant checks described above;
  other MAT paths do not consistently perform ordinary checks. Do not rely on
  `OPTION EXPLICIT` as full matrix validation.

Only the MAT grammar described on this page is implemented. Destinations are not
automatically redimensioned.

## Complete example: solving a linear system

This example works:

```basic
' Solve 2*x + 3*y = 8 and 4*x + y = 6.
DIM a(1 TO 2, 1 TO 2)
DIM b(1 TO 2, 1 TO 1)
DIM ainv(1 TO 2, 1 TO 2)
DIM solution(1 TO 2, 1 TO 1)

a(1,1) = 2: a(1,2) = 3
a(2,1) = 4: a(2,2) = 1
b(1,1) = 8
b(2,1) = 6

MAT ainv = INV(a)
PRINT "Determinant: "; DET()        ' -10
MAT solution = ainv * b
PRINT "x = "; solution(1,1)         ' Approximately 1
PRINT "y = "; solution(2,1)         ' Approximately 2
```

Relevant regression coverage is in [`tests/integration.rs`](../tests/integration.rs)
and [`src/mat.rs`](../src/mat.rs), including explicit lower bounds, shape changes
through `REDIM`, arithmetic, transpose, inverse, and small invertible matrices.

## Native element types

MAT retains its floating-point calculation model. INTEGER, LONG, SINGLE, INT64, and BYTE inputs convert to f64; INT64 must be exactly representable. BOOL and BYTES are not numeric matrix inputs. Every declared destination element is validated and converted before any destination cells change. Fractions cannot enter integer arrays and finite overflow cannot enter SINGLE. Use scalar INT64 arithmetic for exact full-range integers.
