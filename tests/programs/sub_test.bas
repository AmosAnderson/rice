DECLARE SUB Greet(nm AS STRING)

CALL Greet("World")
Greet "BASIC"

SUB Greet(nm AS STRING)
    PRINT "Hello, " + nm + "!"
END SUB
