DIM values(2, 2), names$(2)
values(1, 2) = 10
values(2, 1) = 20
SWAP values(1, 2), values(2, 1)
PRINT values(1, 2); ";"; values(2, 1)
index = 1
SWAP index, values(index, 2)
PRINT index; ";"; values(1, 2)
SWAP values(2, 1), values(2, 1)
PRINT values(2, 1)
names$(1) = "hello"
SWAP missing$, names$(1)
PRINT "["; missing$; "]["; names$(1); "]"
names$(2) = "second"
SWAP names$(1), names$(2)
PRINT "["; names$(1); "]["; names$(2); "]"
SWAP absent, values(2, 1)
PRINT absent; ";"; values(2, 1)
