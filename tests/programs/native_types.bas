REM Native values preserve exact integers and distinguish text from binary data.
DIM exact AS INT64, enabled AS BOOL, octet AS BYTE, payload AS BYTES
PRINT TYPEOF$(exact); "|"; TYPEOF$(enabled); "|"; TYPEOF$(octet); "|"; TYPEOF$(payload)
PRINT exact; "|"; enabled; "|"; octet; "|"; LEN(payload)
exact = CINT64("9007199254740993")
PRINT exact + CINT64(2)
PRINT exact - CINT64(2)
PRINT exact + 1; "|"; exact - 1
PRINT CINT64(3000000001) * CINT64(3)
PRINT exact MOD CINT64(2)
PRINT exact > 9007199254740992; "|"; exact = 9007199254740992
PRINT CINT64("9223372036854775807") < 9223372036854775808
PRINT HEX$(exact); "|"; STR$(exact)
enabled = TRUE()
PRINT enabled AND NOT FALSE(); "|"; enabled XOR TRUE(); "|"; FALSE() OR enabled
PRINT TYPEOF$(enabled = FALSE()); "|"; CINT64(enabled); "|"; CDBL(FALSE())
octet = CBYTE(255)
PRINT octet; "|"; TYPEOF$(octet)
payload = BYTES(0, 127, 128, 255)
PRINT HEXENCODE$(payload); "|"; BYTEAT(payload, 4); "|"; LEN(payload)
copy = BYTESET(payload, 2, 33)
PRINT HEXENCODE$(payload); "|"; HEXENCODE$(copy)
PRINT HEXENCODE$(BYTESLICE(payload, 3)); "|"; LEN(BYTESLICE(payload, 5))
PRINT UTF8DECODE$(UTF8ENCODE("Rice 🍚"))
PRINT HEXENCODE$(HEXDECODE("aB00fF"))
PRINT HEXENCODE$(BYTES(0) & BYTES(255)); "|"; (BYTES(1, 2) = BYTES(1, 2))
