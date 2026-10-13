PRINT REPLACE$("banana", "na", "XYZ")
PRINT REPLACE$("aaaaa", "aa", "b")
PRINT REPLACE$("prefix abc abc", "abc", "X", 8, 1)
PRINT REPLACE$("abc", "", "X", 2)
PRINT REPLACE$("abc", "b", "X", 2, 0)
PRINT REPLACE$("é🙂é🙂", "🙂", "")
PRINT REPLACE$("é🙂é🙂", "é", "X", 2)
PRINT REPLACE("aba", "a", "aa")
PRINT "["; REPLACE$("abc", "a", "X", 4); "]"
PRINT "["; REPLACE$("", "a", "X"); "]"
source$ = "unchanged"
PRINT REPLACE$(source$, "changed", "touched"); "|"; source$
