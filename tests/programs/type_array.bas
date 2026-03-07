TYPE PointType
    X AS INTEGER
    Y AS INTEGER
END TYPE

DIM points(3) AS PointType
points(1).X = 10
points(1).Y = 20
points(2).X = 30
points(2).Y = 40
points(3).X = 50
points(3).Y = 60

FOR i = 1 TO 3
    PRINT points(i).X; points(i).Y
NEXT i
