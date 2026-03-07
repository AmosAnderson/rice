FOR i = 1 TO 3
  CALL Counter
NEXT

SUB Counter
  STATIC count AS INTEGER
  count = count + 1
  PRINT count
END SUB
