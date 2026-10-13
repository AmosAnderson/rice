PRINT VAL("1E2tail"); "|"; VAL("-2.5d+2🙂"); "|"; VAL(".5D-1")
PRINT VAL("1.2.3"); "|"; VAL("1E+"); "|"; VAL("2D-words"); "|"; VAL(".E2")
PRINT VAL("&HFFtail"); "|"; VAL("&O778"); "|"; VAL("-&H10")
PRINT VAL(" 12.5D1rest "); "|"; VAL("1 2")
calls = 0
PRINT VAL(SourceText$()); "|"; calls
FUNCTION SourceText$()
    SHARED calls
    calls = calls + 1
    SourceText$ = "3D2tail"
END FUNCTION
