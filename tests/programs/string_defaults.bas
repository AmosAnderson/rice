DIM text$
PRINT LEN(text$)
SUB PrintDefault
    STATIC saved$
    PRINT LEN(saved$)
END SUB
CALL PrintDefault
