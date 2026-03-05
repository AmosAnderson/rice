' Test RANDOMIZE with fixed seed produces deterministic sequence
RANDOMIZE 12345
a1 = RND
a2 = RND
a3 = RND

' Same seed again should produce same sequence
RANDOMIZE 12345
b1 = RND
b2 = RND
b3 = RND

IF a1 = b1 AND a2 = b2 AND a3 = b3 THEN
    PRINT "deterministic"
ELSE
    PRINT "not deterministic"
END IF

' RND(0) should return the last value
RANDOMIZE 42
x = RND
y = RND(0)
IF x = y THEN
    PRINT "rnd0 ok"
ELSE
    PRINT "rnd0 fail"
END IF

' Values should be in [0, 1)
RANDOMIZE 99
ok% = -1
FOR i% = 1 TO 100
    r = RND
    IF r < 0 OR r >= 1 THEN
        ok% = 0
    END IF
NEXT i%
IF ok% THEN
    PRINT "range ok"
ELSE
    PRINT "range fail"
END IF
