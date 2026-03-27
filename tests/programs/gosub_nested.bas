I = 0
DO
    I = I + 1
    GOSUB MySub
    IF I = 3 THEN EXIT DO
LOOP
PRINT "Done"
END

MySub:
PRINT "In GOSUB"; I
RETURN
