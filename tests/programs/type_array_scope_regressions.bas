DIM values(2) AS INTEGER
values(1) = 42
values(2) = 43
DIM SHARED sharedValues(2) AS BYTE
sharedValues(1) = 9
CALL ReadOuter
PRINT ReadFunction(); "|"; values(1)
CALL ShadowOuter
PRINT values(1); "|"; TYPEOF$(values(1))
CALL ChangeShared
PRINT sharedValues(1); "|"; TYPEOF$(sharedValues(1))

SUB ReadOuter
    PRINT values(1); "|"; TYPEOF$(values(1)); "|"; values(2)
END SUB

FUNCTION ReadFunction() AS INTEGER
    ReadFunction = values(1)
END FUNCTION

SUB ShadowOuter
    DIM values(2) AS LONG
    values(1) = 7
    PRINT values(1); "|"; TYPEOF$(values(1)); "|"; values(2)
END SUB

SUB ChangeShared
    PRINT sharedValues(1); "|"; TYPEOF$(sharedValues(1))
    sharedValues(1) = sharedValues(1) + 1
END SUB
