TYPE Counter
    count AS INTEGER
END TYPE
CALL Increment
CALL Increment
SUB Increment
    STATIC state AS Counter
    state.count = state.count + 1
    PRINT state.count
END SUB
