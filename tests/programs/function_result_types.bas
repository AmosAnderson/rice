PRINT "["; TextValue(); "]"
PRINT "["; TextValue; "]"
PRINT "["; FixedText(); "]"
PRINT NumericValue$() + 5
PRINT LEN(DefaultRecord())
PRINT BuildRecord(17).position.x
PRINT BuildRecord(17).caption
PRINT FreshRecord().position.x
PRINT FreshRecord().position.x
PRINT TypedResult()

TYPE Point
    x AS INTEGER
    y AS NUMERIC
END TYPE
TYPE Entry
    position AS Point
    caption AS STRING
END TYPE

FUNCTION TextValue() AS STRING
    EXIT FUNCTION
END FUNCTION
FUNCTION FixedText() AS STRING * 4
END FUNCTION
FUNCTION NumericValue$() AS LONG
END FUNCTION
FUNCTION DefaultRecord() AS Entry
END FUNCTION
FUNCTION BuildRecord(BYVAL amount) AS Entry
    BuildRecord.position.x = amount
    BuildRecord.caption = "record"
END FUNCTION
FUNCTION FreshRecord() AS Entry STATIC
    FreshRecord.position.x = FreshRecord.position.x + 1
END FUNCTION
FUNCTION TypedResult() AS INTEGER
    TypedResult = 42
END FUNCTION
