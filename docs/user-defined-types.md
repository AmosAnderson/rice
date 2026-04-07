# User-Defined Types

User-defined types (UDTs) allow you to group related variables into a single record structure, similar to structs in C or records in Pascal.

## Defining a Type

```basic
TYPE typename
    field1 AS type
    field2 AS type
    ...
END TYPE
```

### Supported Field Types

- `NUMERIC` - Numeric value
- `STRING` - Variable-length string

### Example

```basic
TYPE PersonType
    FirstName AS STRING
    LastName AS STRING
    Age AS NUMERIC
    Salary AS NUMERIC
END TYPE
```

---

## Using Types

### Declaring Variables

```basic
DIM person AS PersonType
```

### Accessing Fields

Use dot notation to read and write fields:

```basic
person.FirstName = "Alice"
person.LastName = "Smith"
person.Age = 30
person.Salary = 65000.50

PRINT person.FirstName
PRINT person.Age
```

---

## Arrays of Types

Declare arrays where each element is a user-defined type:

```basic
TYPE StudentType
    Name AS STRING
    Grade AS NUMERIC
    GPA AS NUMERIC
END TYPE

DIM students(1 TO 30) AS StudentType

students(1).Name = "Alice"
students(1).Grade = 12
students(1).GPA = 3.85

students(2).Name = "Bob"
students(2).Grade = 11
students(2).GPA = 3.42

FOR i = 1 TO 2
    PRINT students(i).Name & " - Grade" & STR(students(i).Grade) & ", GPA:" & STR(students(i).GPA)
NEXT i
```

---

## Passing Types to Procedures

User-defined types can be passed as parameters to SUB and FUNCTION:

```basic
TYPE PointType
    x AS NUMERIC
    y AS NUMERIC
END TYPE

SUB PrintPoint (p AS PointType)
    PRINT "("; p.x; ","; p.y; ")"
END SUB

FUNCTION Distance (a AS PointType, b AS PointType) AS NUMERIC
    dx = a.x - b.x
    dy = a.y - b.y
    Distance = SQR(dx * dx + dy * dy)
END FUNCTION

DIM p1 AS PointType
DIM p2 AS PointType
p1.x = 0: p1.y = 0
p2.x = 3: p2.y = 4

CALL PrintPoint(p1)
PRINT "Distance:"; Distance(p1, p2)    ' 5
```

---

## Complete Example

```basic
TYPE EmployeeType
    Name AS STRING
    Department AS STRING
    YearsWorked AS NUMERIC
    HourlyRate AS NUMERIC
END TYPE

DIM employees(1 TO 3) AS EmployeeType

employees(1).Name = "Alice Johnson"
employees(1).Department = "Engineering"
employees(1).YearsWorked = 5
employees(1).HourlyRate = 45.00

employees(2).Name = "Bob Smith"
employees(2).Department = "Marketing"
employees(2).YearsWorked = 3
employees(2).HourlyRate = 38.50

employees(3).Name = "Carol Davis"
employees(3).Department = "Engineering"
employees(3).YearsWorked = 8
employees(3).HourlyRate = 52.00

PRINT "Employee Report"
PRINT REPEAT("-", 50)

FOR i = 1 TO 3
    PRINT employees(i).Name
    PRINT "  Dept: " & employees(i).Department
    PRINT "  Years: "; employees(i).YearsWorked
    PRINT "  Rate: $"; employees(i).HourlyRate
    PRINT "  Weekly: $"; employees(i).HourlyRate * 40
    PRINT
NEXT i
```
