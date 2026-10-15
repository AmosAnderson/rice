OPEN "{DIR}/records.dat" FOR OUTPUT AS #1
FOR i = 1 TO 84
    PRINT #1, STRING$(100, CHR$(32 + i));
NEXT i
PRINT #1, "END";
CLOSE #1

OPEN "{DIR}/records.dat" FOR RANDOM AS #1 LEN = 100
FIELD #1, 100 AS record$
count = 0
errors = 0
DO WHILE NOT EOF(1)
    GET #1
    count = count + 1
    IF count <= 84 THEN
        IF record$ <> STRING$(100, CHR$(32 + count)) THEN errors = errors + 1
    ELSE
        IF record$ <> "END" + SPACE$(97) THEN errors = errors + 1
    END IF
LOOP
PRINT count; "|"; errors; "|"; LOC(1)
CLOSE #1
