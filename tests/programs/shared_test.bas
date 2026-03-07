DIM x AS INTEGER
x = 10
CALL AddTen
PRINT x

SUB AddTen
  SHARED x
  x = x + 10
END SUB
