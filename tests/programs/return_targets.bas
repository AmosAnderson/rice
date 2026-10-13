PRINT "start"
GOSUB outer
PRINT "main"
END
outer:
PRINT "outer"
GOSUB inner
PRINT "skipped"
outer_continue:
PRINT "outer-return"
RETURN
inner:
PRINT "inner"
RETURN outer_continue
