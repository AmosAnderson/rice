10 IF 1 THEN 40 ELSE 20
20 PRINT "unreachable"
30 END
40 PRINT "then"
50 IF 0 THEN 20 ELSE 80
60 PRINT "unreachable"
70 END
80 PRINT "else"
90 IF 0 THEN 20
100 PRINT "fallthrough"
110 IF 0 THEN 20 ELSE PRINT "mixed else": PRINT "second"
120 IF 1 THEN PRINT "mixed then" ELSE 20
130 IF 1 THEN 160: PRINT "unreachable" ELSE 20
140 PRINT "unreachable"
150 END
160 PRINT "colon jump"
170 END
