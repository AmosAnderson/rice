DIM numbers(1 TO 2, -1 TO 0), words$(1 TO 2)
numbers(1, -1) = 10
CALL Increase(numbers(1, -1))
PRINT numbers(1, -1)
PRINT IncreaseAgain(numbers(1, -1)); "|"; numbers(1, -1)
CALL SetWord(words$(2))
PRINT words$(2)

TYPE Entry
    amount AS NUMERIC
END TYPE
DIM entries(1 TO 2) AS Entry
CALL SetEntry(entries(1))
PRINT entries(1).amount; "|"; entries(2).amount

SUB Increase(BYREF value)
    value = value + 1
END SUB
FUNCTION IncreaseAgain(BYREF value)
    value = value + 2
    IncreaseAgain = value * 10
END FUNCTION
SUB SetWord(BYREF text$)
    text$ = "updated"
END SUB
SUB SetEntry(BYREF item)
    item.amount = 42
END SUB
