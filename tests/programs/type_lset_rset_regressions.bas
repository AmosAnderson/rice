LSET first$ = "left"
RSET second$ = "right"
DEFSTR D
LSET defaulted = "default"
PRINT "["; first$; "]|["; second$; "]|["; defaulted; "]"
emptyLeft$ = ""
emptyRight$ = ""
LSET emptyLeft$ = "discarded"
RSET emptyRight$ = "discarded"
DIM declared AS STRING
LSET declared = "discarded"
PRINT "["; emptyLeft$; "]|["; emptyRight$; "]|["; declared; "]"
DIM fixed AS STRING * 4
CLEAR
LSET fixed = "x"
PRINT "["; fixed; "]"
RSET fixed = "y"
PRINT "["; fixed; "]"
