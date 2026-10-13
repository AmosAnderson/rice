PRINT MIN(7)
PRINT MAX(-2.5)
PRINT MIN(6, -8, 2.5, -8, 0)
PRINT MAX(-12, -3, -5)
PRINT min(7.5, MAX(1, 4), 9)
PRINT max(-4, MIN(5, 8), 3)

DIM values(2)
values(1) = 9
values(2) = 2
PRINT MIN(values(1), values(2), 5)

trace$ = ""
PRINT MIN(Visit(3), Visit(1), Visit(2))
PRINT trace$
trace$ = ""
PRINT MAX(Visit(2), Visit(3), Visit(1))
PRINT trace$

FUNCTION Visit(BYVAL n AS NUMERIC) AS NUMERIC
    SHARED trace$
    trace$ = trace$ & STR$(n)
    Visit = n
END FUNCTION
