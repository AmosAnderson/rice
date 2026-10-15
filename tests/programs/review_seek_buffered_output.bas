DIM payload AS BYTES
payload = BYTES(65, 66, 67)

OPEN "{DIR}/seek-binary.bin" FOR BINARY AS #1
PUT #1, , payload
SEEK #1, 10
CLOSE #1

OPEN "{DIR}/pointer-binary.bin" FOR BINARY AS #1
PUT #1, , payload
SET #1: POINTER 10
CLOSE #1

OPEN "{DIR}/seek-random.bin" FOR RANDOM AS #1 LEN = 3
FIELD #1, 3 AS record$
LSET record$ = "ABC"
PUT #1, 1
SEEK #1, 10
CLOSE #1

OPEN "{DIR}/pointer-random.bin" FOR RANDOM AS #1 LEN = 3
FIELD #1, 3 AS record$
LSET record$ = "ABC"
PUT #1, 1
SET #1: POINTER 10
RESET
PRINT FREEFILE
