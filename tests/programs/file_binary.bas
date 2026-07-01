OPEN #1: NAME "test_binary.tmp", ORGANIZATION STREAM, ACCESS OUTIN
msg$ = "HELLO"
PUT #1, , msg$
PUT #1, 1, msg$
CLOSE #1

OPEN #1: NAME "test_binary.tmp", ORGANIZATION STREAM, ACCESS OUTIN
GET #1, 1, result$
PRINT result$
CLOSE #1
