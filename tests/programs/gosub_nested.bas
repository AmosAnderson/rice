FOR i = 1 TO 2
    IF i > 0 THEN
        ON 1 GOSUB first, skipped
        PRINT "loop"; i
    END IF
NEXT i
PRINT "done"
END

first:
PRINT "first"; i
GOSUB second
PRINT "back"; i
RETURN

second:
DO
    IF 1 THEN RETURN
LOOP

skipped:
ERROR 5
