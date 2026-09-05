FOR i = 1 TO 2
    GOSUB finish
    PRINT "unreachable loop"
NEXT i
PRINT "unreachable main"
END

finish:
PRINT "finished"
END
