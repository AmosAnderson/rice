PRINT -1 EQV -1; ";"; -1 EQV 0; ";"; 0 EQV -1; ";"; 0 EQV 0
PRINT -1 IMP -1; ";"; -1 IMP 0; ";"; 0 IMP -1; ";"; 0 IMP 0
PRINT 5 EQV 3; ";"; 5 IMP 3; ";"; 5.9 EQV 3.2; ";"; -5.9 IMP 2.9
PRINT 1 XOR 2 EQV 3; ";"; 1 EQV 2 IMP 4; ";"; 1 IMP 2 IMP 4
PRINT 1 OR 2 EQV 3; ";"; 1 AND 2 EQV 0; ";"; NOT 0 EQV -1
PRINT VAL("NaN") EQV 0; ";"; EXP(1000) EQV EXP(1000)
trace$ = ""
PRINT Mark(0) imp Mark(1)
PRINT trace$
trace$ = ""
PRINT Mark(-1) eqv Mark(-1)
PRINT trace$

FUNCTION Mark(BYVAL n)
    SHARED trace$
    trace$ = trace$ & STR$(n)
    Mark = n
END FUNCTION
