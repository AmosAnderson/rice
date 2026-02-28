FOR i = 1 TO 5
    SELECT CASE i
        CASE 1
            PRINT "one"
        CASE 2, 3
            PRINT "two or three"
        CASE 4 TO 5
            PRINT "four or five"
        CASE ELSE
            PRINT "other"
    END SELECT
NEXT i
