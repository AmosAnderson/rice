FOR i = 1 TO 2
    FOR j = 1 TO 3
        PRINT i; j;
NEXT j, i
PRINT

total = 0
FOR a = 1 TO 2
    FOR b = 1 TO 2
        FOR c = 1 TO 2
            total = total + a * b * c
    NEXT c, b ' closes only the two inner loops
    PRINT total;
NEXT a
PRINT

FOR row = 1 TO 2
    FOR column = 1 TO 5
        IF column = 2 THEN EXIT FOR
        PRINT row; column;
NEXT column, row: PRINT ":done"

FOR outer = 1 TO 2
    FOR skipped = 3 TO 1
        PRINT "unreachable"
NEXT skipped, outer
PRINT "skipped"

FOR countdown = 2 TO 1 STEP -1
    FOR inner = 2 TO 1 STEP -1
        PRINT countdown; inner;
NEXT INNER, COUNTDOWN
PRINT
