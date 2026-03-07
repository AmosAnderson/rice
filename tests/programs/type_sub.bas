TYPE RectType
    Width AS SINGLE
    Height AS SINGLE
END TYPE

DECLARE SUB PrintArea (r AS RectType)

DIM rect AS RectType
rect.Width = 5.5
rect.Height = 3.0

PrintArea rect

SUB PrintArea (r AS RectType)
    DIM area AS SINGLE
    area = r.Width * r.Height
    PRINT area
END SUB
