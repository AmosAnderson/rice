# Console Features

RICE BASIC provides text-mode console control for cursor positioning and keyboard input.

## Screen Control

### CLS

Clear the screen and move the cursor to the top-left corner:

```basic
CLS
```

### LOCATE

Position the cursor at a specific row and column (1-based):

```basic
LOCATE 10, 20       ' Move cursor to row 10, column 20
PRINT "Here!"

LOCATE 1, 1          ' Top-left corner
```

### WIDTH

Set the logical terminal width (used for TAB and comma-zone calculations):

```basic
WIDTH 80             ' Set width to 80 columns
```

### VIEW PRINT

Define a scrolling region on the screen:

```basic
VIEW PRINT 5 TO 20   ' Only rows 5-20 scroll
VIEW PRINT            ' Reset to full screen
```

### BEEP

Sound the terminal bell:

```basic
BEEP
```

---

## Console Functions

### CSRLIN

Returns the current cursor row (1-based):

```basic
row = CSRLIN
PRINT "Cursor is on row"; row
```

### POS

Returns the current cursor column (1-based). Takes a dummy argument:

```basic
col = POS(0)
PRINT "Cursor is at column"; col
```

### INKEY

Reads a single keypress without waiting. Returns an empty string if no key is available:

```basic
DO
    k = INKEY
    IF k <> "" THEN PRINT "You pressed: "; k
LOOP UNTIL k = CHR(27)   ' ESC to exit
```

### INPUT$

Reads exactly n characters from the keyboard (blocking) or from a file:

```basic
' From keyboard
PRINT "Press any 3 keys: ";
k = INPUT$(3)
PRINT k

' From file
OPEN #1: NAME "data.bin", ORGANIZATION STREAM, ACCESS INPUT
chunk = INPUT$(10, #1)
CLOSE #1
```

### SCREEN()

Returns the character code of the character at a given screen position:

```basic
code = SCREEN(1, 1)       ' Character code at row 1, col 1
PRINT "Character: "; CHR(code)
```

---

## Example: Simple Status Bar

```basic
CLS
LOCATE 1, 1
PRINT SPACE(80);
LOCATE 1, 30
PRINT "My Application";

LOCATE 5, 10
PRINT "Press any key to continue..."
k = INPUT$(1)
```
