DIM f AS INTEGER
f = FREEFILE
OPEN #f: NAME "test_text_io.tmp", ACCESS OUTPUT
PRINT #f, "Hello, File!"
PRINT #f, "Second line"
PRINT #f, 42
CLOSE #f

OPEN #1: NAME "test_text_io.tmp", ACCESS INPUT
LINE INPUT #1, a$
PRINT a$
LINE INPUT #1, b$
PRINT b$
LINE INPUT #1, c$
PRINT c$
PRINT EOF(1)
CLOSE #1
