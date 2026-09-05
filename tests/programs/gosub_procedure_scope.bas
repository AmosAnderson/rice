CALL DoWork
GOSUB helper
PRINT Counted()
GOSUB helper
END

helper:
PRINT "main"
RETURN

SUB DoWork
    FOR i = 1 TO 2
        GOSUB helper
        PRINT "sub"; i
    NEXT i
    EXIT SUB
helper:
    PRINT "local"; i
    RETURN
END SUB

FUNCTION Counted()
    FOR j = 1 TO 2
        GOSUB helper
    NEXT j
    EXIT FUNCTION
helper:
    Counted = Counted + j
    RETURN
END FUNCTION
