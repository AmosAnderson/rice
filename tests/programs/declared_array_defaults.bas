DIM words(3) AS STRING, fixed(2) AS STRING * 2
DIM numbers$(2) AS NUMERIC, grid(2, 2) AS STRING
DIM whole(1) AS INTEGER, wide(1) AS LONG
DIM real32(1) AS SINGLE, real64(1) AS DOUBLE
PRINT LEN(words(1)); "|"; LEN(fixed(1)); "|"; numbers$(1)
PRINT whole(1); "|"; wide(1); "|"; real32(1); "|"; real64(1)
words(1) = "hello"
fixed(1) = "longer"
PRINT words(1); "|"; LEN(fixed(1)); "|"; LEN(grid(99, -2))
DIM replacement AS NUMERIC
replacement = 42
SWAP words(2), replacement
PRINT words(2); "|"; LEN(replacement)
READ words(3), fixed(2), numbers$(2)
PRINT LEN(words(3)); "|"; LEN(fixed(2)); "|"; numbers$(2)
DATA ,,0
