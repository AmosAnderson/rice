DIM evenExponent AS INT64, oddExponent AS INT64
evenExponent = CINT64("4294967296")
oddExponent = CINT64("9223372036854775807")
PRINT CINT64(0) ^ evenExponent; "|"; CINT64(1) ^ evenExponent; "|"; CINT64(-1) ^ evenExponent
PRINT CINT64(0) ^ oddExponent; "|"; CINT64(1) ^ oddExponent; "|"; CINT64(-1) ^ oddExponent
PRINT CINT64(0) ^ CINT64(0); "|"; CINT64(-2) ^ CINT64(63)
