DECLARE FUNCTION Factorial(n AS INTEGER)

PRINT Factorial(10)

FUNCTION Factorial(n AS INTEGER)
    IF n <= 1 THEN
        Factorial = 1
    ELSE
        Factorial = n * Factorial(n - 1)
    END IF
END FUNCTION
