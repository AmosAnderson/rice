existing = "shared"
commonValue = "common"
retained = "common shared"
existingItems(1) = "one"
commonItems(1) = "two"
retainedItems(1) = "three"
DEFINT E, C, R
CALL BindExisting
PRINT existing; "|"; commonValue; "|"; retained
PRINT existingItems(1); "|"; commonItems(1); "|"; retainedItems(1)

SUB BindExisting
    SHARED existing, existingItems()
    COMMON commonValue, commonItems()
    COMMON SHARED retained, retainedItems()
    PRINT existing; "|"; commonValue; "|"; retained
    PRINT existingItems(1); "|"; commonItems(1); "|"; retainedItems(1)
    existing = "updated shared"
    commonValue = "updated common"
    retained = "updated common shared"
    existingItems(1) = "updated one"
    commonItems(1) = "updated two"
    retainedItems(1) = "updated three"
END SUB
