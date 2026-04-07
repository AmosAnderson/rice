# MAT Operations

RICE BASIC supports ANSI Full BASIC MAT (matrix) operations for numeric arrays.

## Initialization

```basic
DIM A(3, 3)
MAT A = ZER          ! Fill with zeros
MAT A = CON          ! Fill with ones
MAT A = IDN          ! Identity matrix (square only)
```

## I/O

```basic
MAT PRINT A          ! Print matrix to console
MAT READ A           ! Read matrix values from DATA statements
MAT INPUT A          ! Read matrix values from user input
```

## Arithmetic

```basic
DIM A(3, 3), B(3, 3), C(3, 3)
MAT C = A + B        ! Element-wise addition
MAT C = A - B        ! Element-wise subtraction
MAT C = A * B        ! Matrix multiplication (A cols must equal B rows)
```

## Scalar Multiplication

```basic
MAT B = (2.5) * A    ! Multiply every element by 2.5
```

## Inverse and Transpose

```basic
MAT B = INV(A)       ! Matrix inverse (A must be square and non-singular)
MAT B = TRN(A)       ! Matrix transpose
```

## Determinant

After a `MAT B = INV(A)` operation, the `DET` function returns the determinant of the matrix that was inverted:

```basic
MAT B = INV(A)
PRINT DET            ! Determinant of A
```

## Notes

- All MAT operations work on two-dimensional numeric arrays.
- Matrix multiplication checks dimensions: if A is m x n and B is n x p, the result is m x p.
- INV uses Gaussian elimination with partial pivoting. A singular matrix produces a runtime error.
- Arrays use OPTION BASE 1 by default (1-based indexing).
