CONST start = 3, twice = start * 2, title$ = "ok", smallest = MIN(7, twice), copied$ = title$ & "!"
PRINT start; ":"; twice; ":"; smallest; ":"; copied$
IF 0 THEN CONST unused = 1, other = 2 ELSE CONST branch = twice + 1, last = branch + 1
PRINT branch; ":"; last
CALL ShowConstants

SUB ShowConstants
    CONST localOne = twice + 10, localTwo = localOne + 1
    PRINT localOne; ":"; localTwo
END SUB
