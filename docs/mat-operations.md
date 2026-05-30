# MAT Operations

RICE BASIC supports a practical subset of ANSI X3.113-1991 MAT operations. MAT statements operate on whole numeric arrays and provide concise syntax for common linear algebra and array manipulation tasks.

## MAT Assignment and Initialization

### MAT ZER - Zero Matrix

Fill an array with zeros:

```basic
DIM a(3, 3) AS NUMERIC
MAT a = ZER
' All elements of a are now 0
```

### MAT CON - Constant Matrix

Fill an array with ones:

```basic
DIM a(4) AS NUMERIC
MAT a = CON
' All elements of a are now 1
```

### MAT IDN - Identity Matrix

Set a square matrix to the identity matrix (ones on the diagonal, zeros elsewhere):

```basic
DIM a(3, 3) AS NUMERIC
MAT a = IDN
' a(1,1)=1, a(2,2)=1, a(3,3)=1, all others=0
```

### MAT Assignment

Copy one array into another:

```basic
DIM a(3, 3) AS NUMERIC
DIM b(3, 3) AS NUMERIC
' ... fill a ...
MAT b = a
' b is now a copy of a
```

---

## MAT I/O

### MAT PRINT

Print all elements of an array:

```basic
DIM a(3) AS NUMERIC
a(1) = 10: a(2) = 20: a(3) = 30

MAT PRINT a
```

MAT PRINT outputs one row per line. A 1D array is treated as a single-column matrix:

```basic
DIM m(2, 3) AS NUMERIC
m(1,1) = 1: m(1,2) = 2: m(1,3) = 3
m(2,1) = 4: m(2,2) = 5: m(2,3) = 6

MAT PRINT m
' Output:
'  1  2  3
'  4  5  6
```

### MAT READ

Read DATA values into an array:

```basic
DATA 1, 2, 3, 4, 5, 6

DIM a(2, 3) AS NUMERIC
MAT READ a
' a(1,1)=1, a(1,2)=2, a(1,3)=3
' a(2,1)=4, a(2,2)=5, a(2,3)=6
```

### MAT INPUT

Read values from the user into an array:

```basic
DIM a(3) AS NUMERIC
MAT INPUT a
' User enters three values separated by commas
```

---

## MAT Arithmetic

### MAT Addition

Add two arrays element by element:

```basic
DIM a(3) AS NUMERIC
DIM b(3) AS NUMERIC
DIM c(3) AS NUMERIC

a(1) = 1: a(2) = 2: a(3) = 3
b(1) = 10: b(2) = 20: b(3) = 30

MAT c = a + b
' c(1)=11, c(2)=22, c(3)=33
```

### MAT Subtraction

Subtract two arrays element by element:

```basic
MAT c = a - b
' c(1)=-9, c(2)=-18, c(3)=-27
```

### MAT Scalar Multiplication

Multiply every element of an array by a scalar:

```basic
MAT c = (2) * a
' c(1)=2, c(2)=4, c(3)=6
```

The scalar must be enclosed in parentheses.

### MAT Matrix Multiplication

Multiply two 2D matrices:

```basic
DIM a(2, 3) AS NUMERIC
DIM b(3, 2) AS NUMERIC
DIM c(2, 2) AS NUMERIC

' ... fill a and b ...

MAT c = a * b
' c is the matrix product of a and b
' c(i,j) = SUM(a(i,k) * b(k,j)) for k = 1 to 3
```

The inner dimensions must match: if `a` is m-by-n, then `b` must be n-by-p, and the result is m-by-p.

---

## MAT Matrix Operations

### MAT TRN - Transpose

Transpose a matrix (swap rows and columns):

```basic
DIM a(2, 3) AS NUMERIC
DIM b(3, 2) AS NUMERIC

' ... fill a ...

MAT b = TRN(a)
' b(j,i) = a(i,j) for all i, j
```

### MAT INV - Inverse

Compute the inverse of a square matrix:

```basic
DIM a(3, 3) AS NUMERIC
DIM b(3, 3) AS NUMERIC

' ... fill a with an invertible matrix ...

MAT b = INV(a)
' b is the inverse of a
' MAT c = a * b would produce the identity matrix
```

If the matrix is singular (not invertible), a runtime error occurs.

### DET - Determinant

After computing `MAT INV`, the `DET` function returns the determinant of the most recently inverted matrix:

```basic
DIM a(3, 3) AS NUMERIC
DIM b(3, 3) AS NUMERIC

' ... fill a ...

MAT b = INV(a)
PRINT "Determinant:"; DET
```

---

## Array Dimensions

MAT operations use the array dimensions established by `DIM` or `REDIM`. `ZER`, `CON`, and `IDN` require the target array to already be dimensioned.

---

## Complete Example

```basic
' Solve a 2x2 linear system: A * x = b
' using x = INV(A) * b

DIM a(2, 2) AS NUMERIC
DIM b(2, 1) AS NUMERIC
DIM ainv(2, 2) AS NUMERIC
DIM x(2, 1) AS NUMERIC

' System: 2x + 3y = 8
'         4x + 1y = 6
a(1,1) = 2: a(1,2) = 3
a(2,1) = 4: a(2,2) = 1

b(1,1) = 8
b(2,1) = 6

MAT ainv = INV(a)
PRINT "Determinant:"; DET

MAT x = ainv * b
PRINT "x ="; x(1,1)    ' 1
PRINT "y ="; x(2,1)    ' 2
```
