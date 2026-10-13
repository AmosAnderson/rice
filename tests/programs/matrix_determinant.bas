DIM a(-1 TO 0, 3 TO 4), singular(1 TO 2, 1 TO 2), inv(1 TO 2, 1 TO 2)
DATA 0, 2, 3, 4, 1, 2, 2, 4
MAT READ a
MAT READ singular
PRINT DET(); "|"; DET(a); "|"; DET(a()); "|"; DET(singular); "|"; DET()
MAT inv = INV(a)
PRINT DET(); "|"; DET(singular); "|"; DET()
MAT PRINT a
DIM one(0 TO 0, -2 TO -2)
one(0, -2) = 7
PRINT DET(one)
DIM blank(1 TO 2, 1 TO 2)
PRINT DET(blank)
implicit(1, 1) = 5
PRINT DET(implicit)
DET = 99
PRINT DET; "|"; DET(); "|"; DET(a)
