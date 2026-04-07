' Test GOSUB from within a FOR loop
FOR I = 1 TO 3
    GOSUB PrintIt
NEXT I
PRINT "Done"
END

PrintIt:
PRINT "Iter"; I
RETURN
