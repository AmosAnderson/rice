DIM numbers(-1 TO 2), grid(1 TO 2, -1 TO 1), names$(2)
initial: DATA 2, 11, 22, "rice", 33, 44
READ index, numbers(index), grid(2, -1), names$(1), scalar, numbers(-0.9)
PRINT index; "|"; numbers(2); "|"; grid(2, -1); "|"; names$(1); "|"; scalar; "|"; numbers(0)
RESTORE initial
READ numbers(-1), grid(1, 1)
PRINT numbers(-1); "|"; grid(1, 1)
RESTORE more
READ names$(2), grid(2, 1)
PRINT names$(2); "|"; grid(2, 1)
more: DATA "fresh", 55
