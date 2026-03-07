FOR i = 1 TO 3
  CALL Accum
NEXT

SUB Accum STATIC
  total = total + 5
  PRINT total
END SUB
