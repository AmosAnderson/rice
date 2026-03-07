TYPE PersonType
    FirstName AS STRING * 20
    LastName AS STRING * 20
    Age AS INTEGER
END TYPE

DIM person AS PersonType
person.FirstName = "John"
person.LastName = "Doe"
person.Age = 30

PRINT RTRIM$(person.FirstName)
PRINT RTRIM$(person.LastName)
PRINT person.Age
