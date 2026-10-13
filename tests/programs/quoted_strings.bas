PRINT ""
PRINT """"
PRINT """"""
PRINT "He said ""hello""."
PRINT "a""b"; "|"; """start"; "|"; "end"""
PRINT LEN(""""); ":"; ASC("""")
text$ = "é""😀"
PRINT text$; ":"; LEN(text$)
PRINT "quote "" ' REM : remain text" ' "" ignored
DATA "one ""quoted"" value", """", ""
READ first$, second$, third$
PRINT first$; "|"; second$; "|"; third$
PRINT "OPTION DIALECT ""ANSI"""
