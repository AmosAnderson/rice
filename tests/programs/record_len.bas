TYPE InnerRec
    code AS INTEGER
    fixed AS STRING * 3
END TYPE
TYPE Packet
    child AS InnerRec
    total AS LONG
    ratio AS SINGLE
    precise AS DOUBLE
    number AS NUMERIC
    text AS STRING
    empty AS STRING * 0
END TYPE
TYPE EmptyRec
END TYPE
DIM packet AS Packet, packets(1 TO 2) AS Packet, emptyRecord AS EmptyRec
packet.child.fixed = "longer than three"
packet.text = "é🙂z"
PRINT LEN(packet); "|"; LEN(packet.child); "|"; LEN(packet.text)
PRINT LEN(packets(1)); "|"; LEN(emptyRecord)
packets(2).text = "x"
PRINT LEN(packets(2))
calls = 0
PRINT LEN(CopyPacket()); "|"; calls
packet.text = "hello"
PRINT LEN(packet)
FUNCTION CopyPacket()
    SHARED packet, calls
    calls = calls + 1
    CopyPacket = packet
END FUNCTION
