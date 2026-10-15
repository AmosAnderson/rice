a = 10
aItems(1) = 20
b = 30
bItems(1) = 40
DEFINT A, B
CALL Outer
PRINT OuterFunction()
PRINT a; "|"; aItems(1); "|"; b; "|"; bItems(1)

SUB Outer
    WHEN EXCEPTION IN
        CALL Change(a, bItems(1))
    USE
        PRINT "sub rejected"
    END WHEN
    PRINT a; "|"; TYPEOF$(a); "|"; bItems(1); "|"; TYPEOF$(bItems(1))
    WHEN EXCEPTION IN
        ignored = ChangeFunction(aItems(1), b)
    USE
        PRINT "function rejected"
    END WHEN
    PRINT aItems(1); "|"; TYPEOF$(aItems(1)); "|"; b; "|"; TYPEOF$(b)
    CALL ChangeValid(a, aItems(1))
    PRINT a; "|"; TYPEOF$(a); "|"; aItems(1); "|"; TYPEOF$(aItems(1))
END SUB

FUNCTION OuterFunction() AS BOOL
    WHEN EXCEPTION IN
        CALL Change(a, bItems(1))
    USE
        PRINT "sub rejected"
    END WHEN
    PRINT a; "|"; TYPEOF$(a); "|"; bItems(1); "|"; TYPEOF$(bItems(1))
    WHEN EXCEPTION IN
        ignored = ChangeFunction(aItems(1), b)
    USE
        PRINT "function rejected"
    END WHEN
    PRINT aItems(1); "|"; TYPEOF$(aItems(1)); "|"; b; "|"; TYPEOF$(b)
    CALL ChangeValid(a, aItems(1))
    PRINT a; "|"; TYPEOF$(a); "|"; aItems(1); "|"; TYPEOF$(aItems(1))
    OuterFunction = TRUE()
END FUNCTION

SUB Change(BYREF first, BYREF second)
    first = 7
    second = 1.5
END SUB

FUNCTION ChangeFunction(BYREF first, BYREF second)
    first = 7
    second = 1.5
    ChangeFunction = 0
END FUNCTION

SUB ChangeValid(BYREF first, BYREF second)
    first = 7
    second = 8
END SUB
