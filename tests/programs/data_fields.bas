DIM numbers(3), words$(2), named AS STRING, grid(2, 2)
DATA
values: DATA , +7, , -2.5E1, "", Rice,
READ numbers(1), numbers(2), words$(1), numbers(3), words$(2), named, tail$
PRINT numbers(1); "|"; numbers(2); "|"; LEN(words$(1)); "|"; numbers(3); "|"; LEN(words$(2)); "|"; named; "|"; LEN(tail$)
RESTORE values
READ repeated
PRINT repeated
RESTORE matrix_values
MAT READ grid
MAT PRINT grid
matrix_values: DATA +1, , , -4
