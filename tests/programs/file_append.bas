OPEN "test_append.tmp" FOR OUTPUT AS #1
PRINT #1, "Line 1"
CLOSE #1

OPEN "test_append.tmp" FOR APPEND AS #1
PRINT #1, "Line 2"
CLOSE #1

OPEN "test_append.tmp" FOR INPUT AS #1
LINE INPUT #1, a$
PRINT a$
LINE INPUT #1, b$
PRINT b$
CLOSE #1
