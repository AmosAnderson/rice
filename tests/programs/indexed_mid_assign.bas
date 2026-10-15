DIM text$(1 TO 2, -1 TO 1)
text$(1, -1) = "é🙂abcd"
MID$(text$(1.9, -1), 2.9, 2.9) = "漢ZQ"
PRINT text$(1, -1); "|"; LEN(text$(1, -1))
MID$(text$(1, -1), 99) = "ignored"
MID$(text$(1, -1), 1, 0) = "ignored"
PRINT text$(1, -1)
text$(2, 1) = "abcdef"
MID$(text$(2, 1), 4) = "XY"
PRINT text$(2, 1)
MID$(text$(2, 1), 5, 99) = "Z"
PRINT text$(2, 1)
MID$(text$(2, 0), 1) = "cannot extend empty text"
PRINT "["; text$(2, 0); "]"
