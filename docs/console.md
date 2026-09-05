# Console input and output

All features on this page are accepted in both Rice dialects. They implement text output and terminal escape sequences, with an internal byte buffer for limited screen queries. They do not provide a graphical screen, DOS video memory, or a complete terminal emulator.

## PRINT and WRITE

```basic
PRINT "Hello"
PRINT "A"; "B"         ' AB, then newline
PRINT "partial";       ' no newline
PRINT "left", "right"  ' next 16-column zone
PRINT TAB(20); "at column 20"
PRINT SPC(3); "three spaces"
WRITE "Alice", 30
```

Plain `PRINT` emits a blank line. Strings are emitted as-is. Numbers have no leading/trailing spaces; use an explicit separator when printing adjacent values. Semicolons join values; a final semicolon suppresses the newline. Commas insert spaces up to the next zone at columns 17, 33, 49, and so on; a trailing comma also suppresses the newline. Zones do not wrap at `WIDTH`.

`TAB(n)` inserts enough spaces to reach 1-based column `n`, only if that position is ahead of the current position. At/behind the cursor it emits nothing; it never starts a new line. `TAB(0)` also emits nothing. `SPC(n)` emits exactly `n` spaces. Counts truncate toward zero; negative counts are errors. They are special print items, not general string functions.

`WRITE` emits comma-delimited values, quoting strings and doubling embedded quotes, then a newline. Bare `WRITE` emits a blank line. `PRINT USING` adds field formatting; see its [full guide](print-using.md). File-channel output is documented under [file I/O](file-io.md).

## INPUT and LINE INPUT

```basic
INPUT "Name"; name$
INPUT "Coordinates"; x, y
LINE INPUT "Whole line: "; line$
```

`INPUT ["literal prompt";] variable [, variable ...]` always prints `? `, prefixed by the supplied literal prompt. A comma instead of the prompt semicolon is accepted but does not suppress `? `. Prompt expressions and leading-semicolon variants are not implemented.

For a single target, the entire line is the input field. With several targets, input is split at commas and fields are trimmed. Interactive `INPUT` does not implement quoted-CSV parsing: quote characters remain part of string input, and quoted commas still split fields. Numeric targets require finite numbers accepted by the numeric parser. A wrong field count or invalid number prints `? Redo from start` and requests the full line again, without assigning any targets from the rejected line. The existing value/declaration or `$` suffix determines whether a target is a string.

`LINE INPUT ["literal prompt";] variable` prints only the supplied prompt, with no automatic `? `, and reads one complete line, stripping LF/CR. It currently stores a string even in an unsuffixed target that previously held a number; use a `$` variable to keep intent clear. Both statements accept scalar targets, not array elements or record members, and report error 62 when the input stream ends. Console `LINE INPUT` differs from `LINE INPUT #`, which checks that its target is a string.

## Screen statements

| Statement | Implemented behavior |
|---|---|
| `CLS` | Emit `ESC[2J` and `ESC[H`, clear the internal buffer, reset tracked cursor to row 1/column 1 |
| `LOCATE [row] [, column]` | Emit cursor-position escape and set tracked position; omitted values keep their current position |
| `COLOR [foreground] [, background] [, border]` | Set terminal colors; border is parsed and ignored |
| `WIDTH [columns] [, rows]` | Set logical bounds used by `LOCATE`; initially 80 columns and 25 rows |
| `VIEW PRINT top TO bottom` | Emit a terminal scrolling-region escape |
| `VIEW PRINT` | Emit scrolling-region reset |
| `BEEP` | Emit the BEL byte (`CHR$(7)`); whether it sounds depends on the terminal |

`LOCATE` row/column and `WIDTH` dimensions truncate numeric arguments toward zero. Explicit cursor coordinates must be within the current logical bounds, and width/height must be positive. Additional `LOCATE` cursor visibility/start/stop arguments are parsed but discarded without evaluation. `WIDTH` does not resize the physical terminal or the internal screen buffer and does not change comma zones, `TAB`, or automatic wrapping. `WIDTH , 40` changes just the logical row count.

`COLOR` accepts foreground and background indices 0–15. An omitted component retains its previous value; bare `COLOR` re-emits saved colors, or an empty SGR reset if none was set. Both foreground and background support bright colors. Border is not modeled and its expression is not evaluated.

| Index | Color | Index | Color |
|---|---|---|---|
| 0 | Black | 8 | Dark gray |
| 1 | Blue | 9 | Light blue |
| 2 | Green | 10 | Light green |
| 3 | Cyan | 11 | Light cyan |
| 4 | Red | 12 | Light red |
| 5 | Magenta | 13 | Light magenta |
| 6 | Brown / dark yellow | 14 | Yellow |
| 7 | Light gray | 15 | Bright white |

Terminal palettes determine the actual colors. `VIEW PRINT` only writes an escape: it does not validate the region, update the simulated screen, or implement scrolling itself. `CLS`, `LOCATE`, `COLOR`, `VIEW PRINT`, and `BEEP` emit their bytes even to redirected output. Their write errors are currently ignored, unlike normal `PRINT`/`WRITE` output errors.

## Cursor and screen functions

| Function | Result |
|---|---|
| `CSRLIN` or `CSRLIN()` | Tracked 1-based row; takes no arguments |
| `POS(dummy)` | Tracked 1-based column; takes one evaluated but otherwise ignored argument |
| `SCREEN(row, column [, attribute])` | Byte code from the simulated screen; the third argument is evaluated and ignored |

The screen buffer is always **80 × 25 bytes**, initially spaces. `PRINT`/`WRITE` output updates it, including `PRINT USING`. LF resets column and increments row; CR resets column; other bytes occupy cells. There is no tracked automatic wrapping or scrolling, and UTF-8 multibyte characters occupy several simulated byte cells. Escape sequences printed inside a string, tabs, backspaces, and wide Unicode characters are not interpreted as terminal commands by this buffer. The real terminal and `CSRLIN`/`POS`/`SCREEN` can therefore disagree.

`SCREEN` rejects coordinates below 1, but returns space (`32`) beyond the fixed buffer, even when `WIDTH` permits `LOCATE` there. It never returns a color attribute, including for `SCREEN(row, col, 1)`. The third argument does not change the result. Input prompts, echoed keyboard input, and every form of output are not consistently reflected in the buffer: interactive `INPUT` advances the tracked row once after success, while console `LINE INPUT` does not update cursor tracking.

```basic
CLS
LOCATE 2, 3
PRINT "A";
code = SCREEN(2, 3)      ' 65
column = POS(0)          ' 4
```

## Keyboard functions

`INKEY$` or `INKEY$()` polls without waiting and returns `""` when no key is ready, when terminal polling fails, or when the interpreter uses noninteractive injected I/O. The CLI attempts terminal polling; redirected input is not a reliable source for this function. It briefly enables terminal raw mode where needed. Released-key events are ignored.

Ordinary characters are returned as strings, including Unicode characters. Enter returns `CHR$(13)`, Escape `CHR$(27)`, Backspace `CHR$(8)`, and Tab `CHR$(9)`. Ctrl+A–Z returns codes 1–26; supported ASCII control combinations are mapped similarly. Unsupported key events return `""`.

Extended keys return two characters: NUL (`CHR$(0)`) followed by the code below. Modifiers on these special keys do not select additional QB scan-code variants.

| Key | Second character/code |
|---|---|
| Up, down, left, right | `H`/72, `P`/80, `K`/75, `M`/77 |
| Home, End | `G`/71, `O`/79 |
| Page Up, Page Down | `I`/73, `Q`/81 |
| Insert, Delete | `R`/82, `S`/83 |
| F1–F10 | Codes 59–68 |

```basic
DO
    key$ = INKEY$
    IF key$ <> "" THEN PRINT LEN(key$)
LOOP UNTIL key$ = CHR$(27)
```

`INPUT$(count)` reads up to `count` bytes from the interpreter's input stream, blocking while waiting for available input; `count` must convert to an integer at least 1. Unlike `INKEY$`, it does not enable raw keyboard mode: a terminal may require Enter before delivering input. EOF or a read error can return fewer bytes. It decodes with lossy UTF-8, so its result may contain replacement characters and its character count may differ from the requested byte count. `INPUT$(count, #channel)` is the analogous [file form](file-io.md).

There is no `INPUT$` guarantee of exactly one unbuffered keypress, no implemented `ON KEY`/`ON TIMER` event system, and no resumable `STOP` debugger. At the REPL prompt `Ctrl+D` exits; `Ctrl+C` during execution may terminate the host process. Physical terminal resizing, color fidelity, modified special-key mappings, and graceful interruption across hosts are not comprehensively validated.
