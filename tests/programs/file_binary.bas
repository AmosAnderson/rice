OPEN "test_binary.tmp" FOR BINARY AS #1
PUT #1, , msg$
msg$ = "HELLO"
PUT #1, 1, msg$
CLOSE #1

OPEN "test_binary.tmp" FOR BINARY AS #1
DIM result$ AS STRING
GET #1, 1, result$
PRINT result$
CLOSE #1
