' EXIT WHILE passes through other blocks but stops at the nearest WHILE.
outer = 0
WHILE outer < 2
    outer = outer + 1
    inner = 0
    WHILE 1
        inner = inner + 1
        FOR j = 1 TO 2
            DO
                SELECT CASE inner
                CASE 1
                    IF outer > 0 THEN EXIT WHILE ELSE PRINT "unexpected"
                END SELECT
            LOOP
        NEXT j
        PRINT "unexpected"
    WEND
    PRINT outer; ":"; inner; ":"; j
END WHILE

' An unexecuted body cannot raise an unmatched-exit error.
WHILE 0
    EXIT SELECT
WEND

' Exiting does not evaluate the condition again.
checks = 0
WHILE Probe()
    EXIT WHILE: PRINT "unexpected"
WEND
PRINT checks

FUNCTION Probe()
    SHARED checks
    checks = checks + 1
    Probe = checks < 5
END FUNCTION
