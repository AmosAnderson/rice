' DO WHILE at top
x = 1
DO WHILE x <= 3
    PRINT "while:"; x
    x = x + 1
LOOP

' DO UNTIL at bottom
y = 1
DO
    PRINT "until:"; y
    y = y + 1
LOOP UNTIL y > 3
