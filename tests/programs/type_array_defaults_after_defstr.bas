numbers(1) = 10
DEFSTR N
PRINT numbers(2); "|"; TYPEOF$(numbers(2))
CALL ReadExisting
CALL ReadUntouched
numbers(4) = 42
PRINT numbers(1); "|"; numbers(3); "|"; TYPEOF$(numbers(3)); "|"; numbers(4)

SUB ReadExisting
    PRINT numbers(1); "|"; TYPEOF$(numbers(1))
    WHEN EXCEPTION IN
        numbers(1) = 7
    USE
        PRINT "existing read preserves local DEFSTR"
    END WHEN
END SUB

SUB ReadUntouched
    PRINT numbers(3); "|"; TYPEOF$(numbers(3))
    WHEN EXCEPTION IN
        numbers(3) = 7
    USE
        PRINT "untouched read preserves local DEFSTR"
    END WHEN
END SUB
