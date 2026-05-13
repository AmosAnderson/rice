REM Matrix operations test
DIM A(1 TO 2, 1 TO 2)
DIM B(1 TO 2, 1 TO 2)
DIM C(1 TO 2, 1 TO 2)

REM Initialize A manually
A(1, 1) = 1
A(1, 2) = 2
A(2, 1) = 3
A(2, 2) = 4

REM Initialize B manually
B(1, 1) = 5
B(1, 2) = 6
B(2, 1) = 7
B(2, 2) = 8

REM Test MAT PRINT
PRINT "Matrix A:"
MAT PRINT A

REM Test MAT C = A + B
MAT C = A + B
PRINT "A + B:"
MAT PRINT C

REM Test MAT C = A - B
MAT C = A - B
PRINT "A - B:"
MAT PRINT C

REM Test MAT C = A * B (matrix multiplication)
MAT C = A * B
PRINT "A * B:"
MAT PRINT C

REM Test scalar multiply
MAT C = (3) * A
PRINT "(3) * A:"
MAT PRINT C

REM Test transpose
DIM D(1 TO 2, 1 TO 3)
D(1, 1) = 1
D(1, 2) = 2
D(1, 3) = 3
D(2, 1) = 4
D(2, 2) = 5
D(2, 3) = 6
DIM E(1 TO 3, 1 TO 2)
MAT E = TRN(D)
PRINT "TRN(D):"
MAT PRINT E

REM Test ZER
MAT A = ZER
PRINT "ZER:"
MAT PRINT A

REM Test CON
MAT A = CON
PRINT "CON:"
MAT PRINT A

REM Test IDN
MAT A = IDN
PRINT "IDN:"
MAT PRINT A

REM Test INV and DET
DIM F(1 TO 2, 1 TO 2)
F(1, 1) = 4
F(1, 2) = 7
F(2, 1) = 2
F(2, 2) = 6
DIM G(1 TO 2, 1 TO 2)
MAT G = INV(F)
PRINT "INV(F):"
MAT PRINT G
PRINT "DET ="; DET

REM Test MAT READ
DIM H(1 TO 2, 1 TO 2)
DATA 10, 20, 30, 40
MAT READ H
PRINT "MAT READ:"
MAT PRINT H

REM Test copy
DIM I(1 TO 2, 1 TO 2)
MAT I = H
PRINT "Copy:"
MAT PRINT I
