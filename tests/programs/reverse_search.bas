PRINT INSTRREV("banana", "ana"); "|"; INSTRREV("banana", "ana", 5)
PRINT INSTRREV("é🙂é🙂", "🙂"); "|"; INSTRREV("é🙂é🙂", "🙂", 3)
PRINT INSTRREV("é🙂é🙂", "é🙂", 3)
PRINT INSTRREV("abc", ""); "|"; INSTRREV("abc", "", 2); "|"; INSTRREV("", "")
PRINT INSTRREV("abc", "a", 4); "|"; INSTRREV("abc", "A")
PRINT INSTRREV("aba", "a", 2.9); "|"; INSTRREV("aba", "a", -1.9)
path$ = "folder/subfolder/file.bas"
PRINT MID$(path$, INSTRREV(path$, "/") + 1)
