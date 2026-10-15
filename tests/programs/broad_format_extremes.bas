PRINT USING "#.##^^^^"; 1E-309
PRINT USING "#.##^^^^"; 5E-324
PRINT USING "#.##^^^^"; MAXNUM()
PRINT USING "##.##"; 1E308
OPEN "{DIR}/formatted.txt" FOR OUTPUT AS #1
PRINT #1, USING "#.##^^^^"; 1E-309
PRINT #1, USING "#.##^^^^"; 5E-324
PRINT #1, USING "#.##^^^^"; MAXNUM()
PRINT #1, USING "##.##"; 1E308
CLOSE #1
