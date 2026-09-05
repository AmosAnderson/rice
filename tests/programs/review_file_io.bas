OPEN "{DIR}/fields.txt" FOR OUTPUT AS #1
WRITE #1, "héllo " + CHR$(34) + "世界" + CHR$(34), 42
CLOSE #1
OPEN "{DIR}/fields.txt" FOR INPUT AS #1
INPUT #1, text$, n
PRINT text$
PRINT n
ON ERROR GOTO no_more_fields
INPUT #1, text$
PRINT "unexpected input"
END
no_more_fields:
PRINT ERR
RESUME after_eof
after_eof:
ON ERROR GOTO 0
CLOSE #1

OPEN "{DIR}/columns.txt" FOR OUTPUT AS #1
PRINT #1, TAB(5); "A"; SPC(2); "B"
PRINT #1, "x",
PRINT #1, "y"
CLOSE #1
OPEN "{DIR}/columns.txt" FOR INPUT AS #1
LINE INPUT #1, text$
PRINT text$
LINE INPUT #1, text$
PRINT text$
CLOSE #1

TYPE TestRecord
  amount AS INTEGER
  label AS STRING * 4
END TYPE
DIM first AS TestRecord
DIM second AS TestRecord
first.amount = 123
first.label = "rice"
OPEN "{DIR}/records.bin" FOR BINARY AS #1
PUT #1, 1, first
PRINT LOF(1)
GET #1, 1, second
PRINT second.amount
PRINT second.label
CLOSE #1

OPEN "{DIR}/length.txt" FOR OUTPUT AS #1
PRINT #1, "abc";
PRINT LOF(1)
CLOSE #1
