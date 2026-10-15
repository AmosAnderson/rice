DIM values(3) AS INTEGER, octets(2) AS BYTE
CALL SetValue(values(2))
PRINT values(2); "|"; TYPEOF$(values(2))
WHEN EXCEPTION IN
    CALL OverflowSecond(values(3), octets(2))
USE
    PRINT "copyback rejected"
END WHEN
PRINT values(3); "|"; TYPEOF$(values(3)); "|"; octets(2); "|"; TYPEOF$(octets(2))

SUB SetValue(BYREF target AS LONG)
    PRINT target; "|"; TYPEOF$(target)
    target = 12
END SUB

SUB OverflowSecond(BYREF first AS LONG, BYREF second AS LONG)
    first = 23
    second = 256
END SUB
