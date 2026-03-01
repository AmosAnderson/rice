DIM f AS INTEGER
f = FREEFILE
OPEN "test_text_io.tmp" FOR OUTPUT AS #f
PRINT #f, "Hello, File!"
PRINT #f, "Second line"
PRINT #f, 42
CLOSE #f

OPEN "test_text_io.tmp" FOR INPUT AS #1
LINE INPUT #1, a$
PRINT a$
LINE INPUT #1, b$
PRINT b$
LINE INPUT #1, c$
PRINT c$
PRINT EOF(1)
CLOSE #1
