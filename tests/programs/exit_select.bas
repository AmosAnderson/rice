' Nested SELECT exits do not terminate the outer SELECT or enclosing FOR.
FOR pass = 1 TO 2
    SELECT CASE pass
    CASE 1
        SELECT CASE 10
        CASE 10
            EXIT SELECT: PRINT "unexpected"
        END SELECT
        PRINT "inner done"
        WHILE 1
            DO
                FOR n = 1 TO 2
                    IF n = 1 THEN EXIT SELECT
                NEXT n
            LOOP
        WEND
        PRINT "unexpected"
    CASE ELSE
        EXIT SELECT
        PRINT "unexpected"
    END SELECT
    PRINT pass; ":"; n
NEXT pass

' No matching branch means no exit is executed.
SELECT CASE 0
CASE 1
    EXIT WHILE
END SELECT
PRINT "done"
