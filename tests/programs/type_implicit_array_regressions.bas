items(1) = "before"
DEFINT I
items(1) = "after"
items(2) = "second"
PRINT items(1); "|"; items(2); "|"; TYPEOF$(items(1))
initializedLater(1) = 7
PRINT initializedLater(1); "|"; TYPEOF$(initializedLater(1))
WHEN EXCEPTION IN
    initializedLater(1) = 1.5
USE
    PRINT "new array rejected"
END WHEN
CALL LocalArray
CALL UntouchedLocalArray
PRINT items(1); "|"; items(2)

SUB LocalArray
    PRINT items(1)
    WHEN EXCEPTION IN
        items(1) = 1.5
    USE
        PRINT "local array rejected"
    END WHEN
END SUB

SUB UntouchedLocalArray
    PRINT items(3); "|"; TYPEOF$(items(3))
    WHEN EXCEPTION IN
        items(3) = 1.5
    USE
        PRINT "untouched local array rejected"
    END WHEN
END SUB
