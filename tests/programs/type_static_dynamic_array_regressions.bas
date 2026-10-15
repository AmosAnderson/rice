CALL Keep(TRUE())
PRINT KeepFunction(TRUE())
CALL ReadOnly(TRUE())
PRINT ReadOnlyFunction(TRUE())
DEFINT A, B, R, S
CALL Keep(FALSE())
PRINT KeepFunction(FALSE())
CALL ReadOnly(FALSE())
PRINT ReadOnlyFunction(FALSE())

SUB Keep(BYVAL first AS BOOL) STATIC
    IF first THEN
        a(1) = "first"
    ELSE
        a(1) = "second"
    END IF
    PRINT a(1); "|"; TYPEOF$(a(1))
END SUB

FUNCTION KeepFunction(BYVAL first AS BOOL) AS STRING STATIC
    IF first THEN
        b(1) = "first"
    ELSE
        b(1) = "second"
    END IF
    KeepFunction = b(1) & "|" & TYPEOF$(b(1))
END FUNCTION

SUB ReadOnly(BYVAL first AS BOOL) STATIC
    IF first THEN
        PRINT r(1); "|"; TYPEOF$(r(1))
    ELSE
        r(1) = "read-only sub"
        PRINT r(1); "|"; TYPEOF$(r(1))
    END IF
END SUB

FUNCTION ReadOnlyFunction(BYVAL first AS BOOL) AS STRING STATIC
    IF first THEN
        ReadOnlyFunction = TYPEOF$(s(1))
    ELSE
        s(1) = "read-only function"
        ReadOnlyFunction = s(1) & "|" & TYPEOF$(s(1))
    END IF
END FUNCTION
