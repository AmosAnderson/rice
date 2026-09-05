# MAT Operations

Rice implements the matrix operations below in both the default QBasic mode and
ANSI mode. They form a limited ANSI-style extension, not the complete ANSI Full
BASIC matrix facility or a claim that QBasic 1.1 supports this syntax.
See [Dialects](dialects.md) and [Language Reference](language-reference.md).

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

Bounds are inclusive. Rice currently defaults to lower bound `1` in both modes,
so `DIM a(2, 2)` means a 2-by-2 matrix. `OPTION BASE 0` makes the same declaration
3-by-3; this explicit option is needed when porting code that assumes QBasic's
usual zero base. Use explicit bounds to make the matrix shape clear:

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

After a successful `MAT target = INV(source)`, `DET` or `DET()` reports that
source matrix's determinant. It is initially `0`; a failed inverse leaves the
previous value, and `CLEAR` does not reset it. `DET` does not accept a matrix
argument or independently calculate a determinant. An ordinary variable named
`DET` shadows the bare form; `DET()` explicitly requests the built-in value.

## Matrix I/O

`MAT PRINT a` writes one row per line, with single spaces between numeric cells
and ordinary Rice number formatting. A one-column two-dimensional matrix therefore
prints one value per line. There is no comma-zone alignment or trailing separator
option.

`MAT READ a` consumes the global `DATA` stream in row-major order: all columns of
the first row, then the next row. It shares its cursor with ordinary `READ` and
`RESTORE`. Numeric DATA values are used directly; string DATA values are parsed as
numbers, and an unparseable string becomes `0`. On insufficient DATA, the statement
errors after consuming available items but does not store a partial matrix.

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

**Channel limitation:** the parser accepts `MAT PRINT #n, a` and `MAT INPUT #n, a`,
but the runtime ignores the channel entirely, including its expression evaluation.
They still use console/program output and standard input, even if the channel is
not open. Use ordinary [File I/O](file-io.md) statements and loops for matrix files.

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
- MAT paths do not consistently perform ordinary declaration/constant checks.
  Do not rely on `OPTION EXPLICIT` as full matrix validation.

These are implementation limitations, not alternate mathematical definitions.
Portable ANSI conformance, broader MAT grammar, channel I/O, and automatic
destination redimensioning are not implemented or claimed.

## Complete example: solving a linear system

This example works in both Rice dialects:

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
