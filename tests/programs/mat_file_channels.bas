DIM source(-1 TO 0, 2 TO 3), result(4 TO 5, -2 TO -1)
DATA 1.5, -2, 3, 4
MAT READ source
calls = 0
OPEN "{DIR}/matrix.csv" FOR OUTPUT AS #1
MAT PRINT #Channel(), source
PRINT LOF(1)
CLOSE #1
OPEN "{DIR}/matrix.csv" FOR INPUT AS #1
MAT INPUT #Channel(), result
IF EOF(1) THEN PRINT "eof"
CLOSE #1
PRINT calls
MAT PRINT result
PRINT LBOUND(result, 1); "|"; UBOUND(result, 1); "|"; LBOUND(result, 2); "|"; UBOUND(result, 2)
FUNCTION Channel()
    SHARED calls
    calls = calls + 1
    Channel = 1
END FUNCTION
