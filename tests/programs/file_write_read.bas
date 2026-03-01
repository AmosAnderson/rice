OPEN "test_write_read.tmp" FOR OUTPUT AS #1
WRITE #1, "Alice", 30
WRITE #1, "Bob", 25
CLOSE #1

OPEN "test_write_read.tmp" FOR INPUT AS #1
INPUT #1, name1$, age1%
PRINT name1$; age1%
INPUT #1, name2$, age2%
PRINT name2$; age2%
CLOSE #1
