OPEN #1: NAME "test_append.tmp", ACCESS OUTPUT
PRINT #1, "Line 1"
CLOSE #1

OPEN #1: NAME "test_append.tmp", ACCESS OUTIN
SET #1: POINTER LOF(1) + 1
PRINT #1, "Line 2"
CLOSE #1

OPEN #1: NAME "test_append.tmp", ACCESS INPUT
LINE INPUT #1, a$
PRINT a$
LINE INPUT #1, b$
PRINT b$
CLOSE #1
