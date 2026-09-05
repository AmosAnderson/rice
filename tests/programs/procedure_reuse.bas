FOR attempt = 1 TO 2
    total = 0
    CALL Accumulate(3, total)
    PRINT total
    PRINT SumTo(3)
    GOSUB helper
NEXT attempt
END

helper:
PRINT "main"
RETURN

SUB Accumulate(BYVAL depth, BYREF total)
    GOSUB helper
    IF depth > 1 THEN CALL Accumulate(depth - 1, total)
    GOSUB helper
    EXIT SUB
helper:
    total = total + depth
    RETURN
END SUB

FUNCTION SumTo(BYVAL depth)
    IF depth > 1 THEN SumTo = SumTo(depth - 1)
    GOSUB helper
    EXIT FUNCTION
helper:
    SumTo = SumTo + depth
    RETURN
END FUNCTION
