DIM SHARED counter AS INTEGER
CALL Count
PRINT counter; "|"; TYPEOF$(counter)
WHEN EXCEPTION IN
    CALL FractionalStart
USE
    PRINT "rejected"
END WHEN
PRINT counter; "|"; TYPEOF$(counter)

SUB Count
    FOR counter = 1 TO 2
        PRINT counter; "|"; TYPEOF$(counter)
    NEXT counter
END SUB

SUB FractionalStart
    FOR counter = 1.5 TO 2
        PRINT "unexpected"
    NEXT counter
END SUB
