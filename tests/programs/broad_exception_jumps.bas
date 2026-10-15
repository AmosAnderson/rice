visits = 0
WHEN EXCEPTION IN
    GOTO body_start
    PRINT "wrong body jump"
body_start:
    visits = visits + 1
    IF visits < 2 THEN GOTO body_start
    PRINT "body|"; visits
USE
    PRINT "wrong body handler"
END WHEN

visits = 0
WHEN EXCEPTION IN
    PRINT 1 / 0
USE
    GOTO handler_start
    PRINT "wrong handler jump"
handler_start:
    visits = visits + 1
    IF visits < 2 THEN GOTO handler_start
    PRINT "handler|"; visits; "|"; EXTYPE
END WHEN
PRINT "cleared|"; EXTYPE

WHEN EXCEPTION IN
    WHEN EXCEPTION IN
        GOTO outer_body
    USE
        PRINT "wrong inner handler"
    END WHEN
    PRINT "wrong outer body jump"
outer_body:
    PRINT "nested body jump"
USE
    PRINT "wrong outer handler"
END WHEN

WHEN EXCEPTION IN
    GOTO after_body
USE
    PRINT "wrong outward body handler"
END WHEN
PRINT "wrong outward body fallthrough"
after_body:
PRINT "outward body jump"

WHEN EXCEPTION IN
    PRINT 1 / 0
USE
    IF 1 THEN GOTO after_handler
END WHEN
PRINT "wrong outward handler fallthrough"
after_handler:
PRINT "outward handler jump|"; EXTYPE
