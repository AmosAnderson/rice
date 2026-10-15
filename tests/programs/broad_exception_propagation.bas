CALL LeaveBody
CALL LeaveHandler

FOR iteration = 1 TO 3
    WHEN EXCEPTION IN
        PRINT 1 / 0
    USE
        EXIT FOR
    END WHEN
    PRINT "wrong loop fallthrough"
NEXT iteration
PRINT "loop exit|"; iteration

WHEN EXCEPTION IN
    WHEN EXCEPTION IN
        ERROR 1
    USE
        ERROR 2
    END WHEN
USE
    PRINT "outer caught handler error"
END WHEN

WHEN EXCEPTION IN
    PRINT 1 / 0
USE
    PRINT "outer slot|"; EXTYPE
    WHEN EXCEPTION IN
        successful = 1
    USE
        PRINT "wrong nested handler"
    END WHEN
    PRINT "shared slot|"; EXTYPE
END WHEN

ON ERROR GOTO classic_handler
WHEN EXCEPTION IN
    PRINT 1 / 0
USE
    ERROR 7
END WHEN
PRINT "classic resumed"
END
classic_handler:
PRINT "classic caught|"; ERR
RESUME NEXT

SUB LeaveBody
    WHEN EXCEPTION IN
        PRINT "body exit"
        EXIT SUB
    USE
        PRINT "wrong body exit handler"
    END WHEN
    PRINT "wrong body exit fallthrough"
END SUB

SUB LeaveHandler
    WHEN EXCEPTION IN
        PRINT 1 / 0
    USE
        PRINT "handler exit"
        EXIT SUB
    END WHEN
    PRINT "wrong handler exit fallthrough"
END SUB
