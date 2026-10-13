prefix$ = "RICE_ENV_INDEX_TEST_"
DIM positions(3)
i = 1
count = 0
DO
    entry$ = ENVIRON$(i)
    IF entry$ = "" THEN EXIT DO
    IF LEFT$(entry$, LEN(prefix$)) = prefix$ THEN
        count = count + 1
        positions(count) = i
        PRINT entry$
    END IF
    i = i + 1
LOOP
PRINT count; "|["; ENVIRON$(i); "]"
PRINT ENVIRON$(positions(1) + .9)
middleIndex = positions(2)
calls = 0
PRINT ENVIRON$(NextIndex()); "|"; calls
PRINT "["; ENVIRON$(1000000000000000); "]"
FUNCTION NextIndex()
    SHARED calls, middleIndex
    calls = calls + 1
    NextIndex = middleIndex
END FUNCTION
