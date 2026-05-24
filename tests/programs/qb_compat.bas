OPTION DIALECT "QB"

' Test suffix-typed variables co-existing
x% = 10
x& = 200000
x! = 3.14
x# = 2.718281828
x$ = "hello"
PRINT x%
PRINT x&
PRINT x!
PRINT x#
PRINT x$

' Test comparison returning -1 for true
PRINT 5 > 3
PRINT 2 = 2
PRINT 1 < 0

' Test bitwise logical operators on numbers
PRINT 5 AND 3
PRINT 5 OR 3
PRINT NOT -1
PRINT 5 XOR 3

' Test string concatenation with +
PRINT "hello " + "world"

' Test hex/octal literals
PRINT &HFF
PRINT &O77

' Test GOSUB / RETURN
GOSUB 100
PRINT "after GOSUB"
GOTO 200
100 PRINT "inside GOSUB"
RETURN
200 PRINT "done GOSUB"

' Test ON GOTO
idx = 2
ON idx GOTO 300, 400, 500
300 PRINT "300": GOTO 600
400 PRINT "400": GOTO 600
500 PRINT "500"
600 PRINT "done ON GOTO"

' Test default BYREF parameter passing in QB mode
val = 10
ChangeMe val
PRINT val

' Test forced BYVAL parameter passing via parentheses
val = 10
ChangeMe (val)
PRINT val

SUB ChangeMe (a)
  a = 42
END SUB
