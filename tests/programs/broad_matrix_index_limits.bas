lo = CINT64("-9223372036854775808")
hi = CINT64("9223372036854775807")
DIM low(lo TO lo, lo TO lo), high(hi TO hi, hi TO hi)
MAT PRINT low
MAT low = CON
PRINT low(lo, lo)
MAT high = low
PRINT high(hi, hi)
MAT PRINT high
PRINT DET(high)

DIM ordinary(1, 1)
ordinary(1, 1) = 7
ordinary(lo, 1) = 99
ordinary(hi, 1) = 99
ordinary(1, lo) = 99
ordinary(1, hi) = 99
MAT PRINT ordinary

DIM source(2, 1), destination(hi TO hi, 1 TO 1)
MAT source = CON
destination(hi, 1) = 8
WHEN EXCEPTION IN
    MAT destination = source
USE
    PRINT "index overflow"
END WHEN
PRINT destination(hi, 1)

OPTION BASE 0
implicit(hi, 0) = 1
WHEN EXCEPTION IN
    MAT PRINT implicit
USE
    PRINT "extent overflow"
END WHEN
WHEN EXCEPTION IN
    MAT implicit = CON
USE
    PRINT "initializer extent overflow"
END WHEN

OPTION BASE 1
sparse(1, 1) = 5
sparse(0, 2) = 9
MAT PRINT sparse
MAT sparse = CON
MAT PRINT sparse
