TYPE Position
  x AS INTEGER
  tag AS STRING * 3
END TYPE
TYPE Entry
  point AS Position
  note AS STRING
END TYPE
DIM ints%(2, 2), longs&(2), singles!(2), doubles#(2), words$(2), entries(2) AS Entry
ints%(1, 2) = -123
longs&(1) = 2147483000
singles!(1) = 1.25
doubles#(1) = 3.5
words$(1) = "éAB"
entries(1).point.x = 17
entries(1).point.tag = "xyz"
entries(1).note = "hi"
OPEN "{DIR}/elements.bin" FOR BINARY AS #1
PUT #1, , ints%(1, 2)
PUT #1, , longs&(1)
PUT #1, , singles!(1)
PUT #1, , doubles#(1)
PUT #1, , words$(1)
PUT #1, , entries(1)
PRINT LOF(1)
GET #1, 1, ints%(2, 1)
GET #1, , longs&(2)
GET #1, , singles!(2)
GET #1, , doubles#(2)
GET #1, , words$(2)
GET #1, , entries(2)
PRINT ints%(2, 1); ";"; longs&(2); ";"; singles!(2); ";"; doubles#(2)
PRINT words$(2); ";"; entries(2).point.x; ";"; entries(2).point.tag; ";"; entries(2).note
CLOSE #1
