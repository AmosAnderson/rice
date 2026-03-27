TIMER ON
ON TIMER(1) GOSUB HandleTimer

T! = TIMER
PRINT "Starting 3-second wait..."
DO
    IF TIMER - T! > 3 THEN EXIT DO
LOOP
PRINT "Done!"
END

HandleTimer:
PRINT "Timer fired at "; TIMER
RETURN
