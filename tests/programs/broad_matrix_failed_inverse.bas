DIM source(1, 1), inverse(1, 1), integers(1, 1) AS INTEGER
source(1, 1) = 2
MAT inverse = INV(source)
PRINT DET()
source(1, 1) = 3
WHEN EXCEPTION IN
    MAT integers = INV(source)
USE
    PRINT "type rejected"
END WHEN
PRINT DET(); "|"; integers(1, 1)
CONST blocked_1_1 = 7
WHEN EXCEPTION IN
    MAT blocked = INV(source)
USE
    PRINT "constant rejected"
END WHEN
PRINT DET(); "|"; blocked_1_1
MAT inverse = INV(source)
PRINT DET()
