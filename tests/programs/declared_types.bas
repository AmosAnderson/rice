DECLARE FUNCTION CountNext(BYVAL value AS INTEGER) AS LONG
DIM small AS INTEGER, wide AS LONG, approximate AS SINGLE, precise AS DOUBLE
DIM fixed AS STRING * 4, text AS STRING, entries(2) AS INTEGER
small = -32768
wide = 2147483647
approximate = 16777217
precise = 1.5
PRINT TYPEOF$(small); "|"; TYPEOF$(wide); "|"; TYPEOF$(approximate); "|"; TYPEOF$(precise)
PRINT small; "|"; wide; "|"; approximate; "|"; precise
PRINT "["; fixed; "]"
fixed = "é🙂"
PRINT "["; fixed; "]"; LEN(fixed)
fixed = "abcdef"
PRINT fixed
CONST cap AS INTEGER = 12, label AS STRING * 3 = "x"
PRINT TYPEOF$(cap); "|"; "["; label; "]"
entries(1) = 27
PRINT TYPEOF$(entries(1)); "|"; TYPEOF$(entries(2)); "|"; entries(2)
DIM overridden% AS DOUBLE
overridden% = 1.5
PRINT TYPEOF$(overridden%); "|"; overridden%
implicit% = 3: implicit& = 4: implicit! = 1.25: implicit# = 2.5: implicit$ = "text"
PRINT TYPEOF$(implicit%); "|"; TYPEOF$(implicit&); "|"; TYPEOF$(implicit!); "|"; TYPEOF$(implicit#); "|"; TYPEOF$(implicit$)
TYPE Entry
    count AS INTEGER
    title AS STRING * 3
END TYPE
DIM item AS Entry
item.count = 8
item.title = "q"
PRINT TYPEOF$(item.count); "|"; "["; item.title; "]"
PRINT CountNext(4); "|"; TYPEOF$(CountNext(4))
CALL Tick
CALL Tick
SHARED total AS LONG
COMMON greeting AS STRING * 4
total = 20: greeting = "hi"
CALL ChangeShared
PRINT total; "|"; "["; greeting; "]"
FUNCTION CountNext(BYVAL value AS INTEGER) AS LONG
    CountNext = value + 1
END FUNCTION
SUB Tick
    STATIC count AS INTEGER
    count = count + 1
    PRINT count; "|"; TYPEOF$(count)
END SUB
SUB ChangeShared
    SHARED total AS LONG, greeting AS STRING * 4
    total = total + 1
    greeting = "bye"
END SUB
