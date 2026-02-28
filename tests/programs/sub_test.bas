DECLARE SUB Greet(name AS STRING)

CALL Greet("World")
Greet "BASIC"

SUB Greet(name AS STRING)
    PRINT "Hello, " + name + "!"
END SUB
