passes = 0
divisor = 0
retried = 0
handled = 0
WHEN EXCEPTION IN
    passes = passes + 1
    IF divisor = 0 THEN PRINT 1 / 0
    divisor = 1
    IF retried = 0 THEN PRINT 1 / 0
    PRINT "completed|"; passes; "|"; handled
USE
    handled = handled + 1
    IF handled = 1 THEN CONTINUE
    retried = 1
    RETRY
END WHEN
PRINT "cleared|"; EXTYPE

visits = 0
WHEN EXCEPTION IN
before_continue:
    visits = visits + 1
    IF visits = 1 THEN PRINT 1 / 0
    IF visits = 1 THEN GOTO before_continue
    PRINT "retained label|"; visits
USE
    CONTINUE
END WHEN
