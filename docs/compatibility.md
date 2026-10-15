# Implementation limits

This page records Rice BASIC **0.20.0 implementation behavior**, constraints, and partial features. The authoritative Rice specification is the [core reference](language-reference.md), [native types](data-types.md), [builtins](builtins.md), and linked topic guides.

“Implemented” means supported by the parser/runtime described here. “Partial” means accepted syntax or familiar names have a smaller or different effect. “Unverified” identifies behavior that has not been established by the available tests or inspection.

## Documented runtime behavior and limits

| Area | Rice behavior and implication |
|---|---|
| Default array bounds | Lower bound is **1**. Specify `OPTION BASE 0` or explicit bounds when zero-based indexing is needed. |
| Numeric types | NUMERIC/DOUBLE are f64 aliases; INTEGER i16, LONG i32, SINGLE f32, INT64 i64, and BYTE u8 retain their own representation. Numeric literals begin as f64. |
| Assignment types | Every declared type and suffix shorthand enforces runtime conversion. Untyped unsuffixed implicit bindings remain dynamic unless DEFtype applies. This is runtime validation, not static type analysis. |
| Array storage/defaults | Bounds/rank are recorded but not enforced. Indices truncate. Flattened keys can collide with scalar names. Declared element types enforce assignments and defaults; metadata remains interpreter-wide. |
| Array reset/resizing | ERASE/CLEAR/REDIM have Rice-specific metadata and content retention rules. `REDIM PRESERVE` may change any dimension and retains stored values outside the new bounds. See [arrays](language-reference.md). |
| Arithmetic | `MOD` works on reals. Integer division `\` rounds operands to nearest-even integers, checks signed 64-bit range, and truncates the quotient toward zero. Bitwise operators, including EQV/IMP, use truncating/saturating signed 64-bit casts. Logical operands are evaluated eagerly. Floating-point math may produce NaN/infinity; MIN/MAX propagate NaN. |
| Output | PRINT/STR$ omit leading/trailing positive-number spaces. PRINT commas use 16-character zones. PRINT USING implements a subset of formatting behavior; see [formatting](print-using.md). |
| Text | Most substring operations count Unicode scalar values. Binary strings use byte-to-character mapping, while screen tracking uses UTF-8 bytes. There is no DOS code-page or grapheme-width model. |
| Quoted literals | Doubled quotes escape one quote; `PRINT "a""b"` now prints `a"b`. Use `PRINT "a"; "b"` for separate values. Backslash is literal text. `!` is not a comment marker. |
| Input destinations | READ/INPUT/LINE INPUT support scalar and array-element destinations, including console/file input. INPUT resolves all indices before reading and commits values as a batch; READ assigns each target before resolving the next. Failed destination resolution consumes no input; index-expression side effects and earlier READ assignments persist. Empty subscripts, slices, and record-member targets remain unsupported. |
| Function spelling | `RND()`, `PI()`, `MAXNUM()`, `CURDIR$()`, and `COMMAND$()` need parentheses; bare names read variables. Conversely TIMER/FREEFILE require bare keyword form. Builtin lookup also accepts some omitted `$` aliases. See [calling conventions](builtins.md). |
| Numeric conversion | VAL now reads scientific decimal prefixes with E/D exponents and stops at a second decimal point. An incomplete exponent leaves the valid mantissa. Radix width limits and whole-string NaN/infinity behavior remain Rice-specific. See [builtins](builtins.md). |
| Decimal places | ROUND/TRUNCATE accept negative places down to -308 for tens, hundreds, and larger powers of ten. ROUND keeps ties away from zero, distinct from CINT/CLNG's nearest-even rule. Finite overflow in a negative-place result raises an error; the nonnegative-place path is unchanged. |
| String search/replacement | REPLACE$ and INSTRREV are Rice extensions. They are case-sensitive and count Unicode scalar values. REPLACE$ returns only the selected suffix; INSTRREV's start limits the final character of a match. No locale/case-insensitive comparison option is implemented. |
| String ordering and patterns | STRCOMP mode 1 compares Unicode-lowercased strings, without locale collation, full case folding, or normalization. LIKE is case-sensitive whole-string matching with Unicode scalars and ASCII digit wildcards. Both are Rice extensions; LIKE is newly reserved as an unsuffixed keyword. Malformed pattern classes raise error 5. |
| Indexed string overwrite | MID$ assignment now accepts array elements, resolving indices once before start/replacement/count processing. It retains the scalar evaluation order and never changes string length. Record-member/slice targets remain unsupported. |
| Environment enumeration | ENVIRON$(index) now returns sorted, 1-based NAME=value entries, merging interpreter overrides and Unicode host entries; indices truncate, and past-end returns an empty string. The ENVIRON() alias remains host-only. |
| Procedure calls | BYREF is copy-in/copy-out of variables and individual array elements. Element indices run once during argument evaluation; later index changes do not redirect writeback. Fields, expressions, constants, and function results have no writeback. Two arguments referencing the same location are independent copies; later parameters win. Array-parameter syntax is parsed but does not bind whole arrays. |
| SWAP destinations | Scalar names and array elements are accepted. Both locations resolve once before values are exchanged. Missing values use ordinary numeric/string/record defaults; missing string scalars no longer become numeric zero. Members/slices remain unsupported. |
| Scope and result types | Reads can fall through caller environments; writes become local unless SHARED. Explicit parameter/result types and suffixes are enforced. Unsuffixed parameters/results without AS remain dynamic. See [procedures](procedures.md). |
| Constants/declarations | OPTION EXPLICIT takes effect when executed and has declaration/suffix exceptions. CLEAR preserves constants and declaration metadata. Declarations and assignments reject constant destinations. See [core reference](language-reference.md). |
| CONST lists | Constants initialize left to right at runtime, with optional AS conversion. Earlier definitions and side effects survive later failure. Trailing commas are rejected. |
| DATA and labels | DATA/definitions in eligible nested blocks are prescanned even if those blocks never execute. RESTORE to an unknown DATA label rewinds instead of raising an error. Numeric labels do not sort a source file; only stored REPL lines are sorted. Labels on block terminators such as NEXT/END IF are rejected, which also limits stored numbered REPL blocks. |
| DATA fields | Leading plus is accepted on numeric literals. Leading, interior, and trailing empty fields supply zero or an empty string according to the READ destination; empty fields reject records. Bare DATA adds no item. A trailing comma now adds an empty item, changing the previous behavior that discarded it. |
| NEXT lists | `NEXT k, j, i` closes matching nested FOR loops from innermost to outermost. It cannot close an intervening unfinished non-FOR block. Bare NEXT closes one loop. |
| Structured exits | EXIT WHILE and EXIT SELECT are Rice extensions. They leave the nearest matching active construct and cannot cross a SUB/FUNCTION boundary. Unmatched uses raise errors. |
| Targeted RETURN | `RETURN label` unwinds exactly one GOSUB, then uses caller GOTO resolution. Rice also permits it within a procedure, with local targets; it cannot escape to module labels. Unmatched RETURN and unresolved targets at the top-level boundary may bypass ON ERROR. See [control flow](language-reference.md#control-flow). |
| Matrix files | `MAT PRINT #n, a` and `MAT INPUT #n, a` now evaluate and use the channel. File output uses comma-separated numeric rows; input reads numeric fields in row-major order and stores only a complete matrix. The former behavior of ignoring channels and using console streams is removed. See [MAT](mat-operations.md) for errors and storage limits. |
| Classic handlers | ON ERROR/RESUME are controlled by top-level execution. Nested failures resume at enclosing top-level statements rather than providing arbitrary statement-level continuations. Structured handler state is a shared slot, not a nested exception stack. See [errors](error-handling.md). |
| Binary files | GET/PUT uses Rice's type-driven little-endian record serialization. Native INT64/BOOL/BYTE scalar layouts and raw BYTES transfers preserve their representations. |
| Indexed binary operands | GET/PUT accept scalars or individual array elements, including records. Retained declarations select binary layouts before suffix shorthand; members, slices, and whole arrays remain unsupported. |
| Record length | LEN(record) reports the current binary payload byte count, including variable-string headers and nested records. It is value/schema-dependent and excludes RANDOM stride padding and host alignment. LEN(string) continues to count Unicode scalar values. |
| SQLite | Bundled database engine, scalar parameter binding, transactions, and materialized results. NULL is distinct from empty text/blob; all declared types reject NULL. Results occupy memory in full; no streaming/prepared-handle, custom-function, or extension-loading APIs. See [SQLite](sqlite.md). |
| Relative paths | File, directory, and database operations use process cwd, not source-file directory. FILES does not filter wildcard patterns; KILL does not expand them. |
| Process/clock | ENVIRON and DATE$/TIME$ assignments are interpreter-local overrides. COMMAND$() returns only BASIC program arguments, joined with spaces; shell quoting is not reconstructed. It defaults to empty in new library/REPL interpreters. Use `--` to pass dash-leading CLI arguments. SHELL ignores child exit status. SLEEP without a positive argument does not wait for a key. |
| REPL state | NEW clears stored lines only. RUN uses a fresh interpreter. LOAD/SAVE restore/persist numbered source only; LOAD leaves the old buffer intact on read/structure errors and never executes or resets immediate state. Immediate END/SYSTEM exit detection has top-level syntax quirks. See [runtime](runtime.md). |

## Accepted syntax with limited or ignored effects

These accepted forms have the following runtime limits:

- DECLARE signatures must match available definitions but do not link external code. Array markers do not implement whole-array binding. COMMON and COMMON SHARED share state within one interpreter; no CHAIN transfer exists.
- STRING * n enforces a Unicode character count. Binary strings use a one-byte character mapping, so this is not a UTF-8 byte-width declaration.
- MAT supports two-dimensional numeric arrays with console and file I/O. `DET(matrix)` calculates a square matrix determinant independently, including singular matrices, without updating cached DET(). There is no one-dimensional MAT vector support; destination shapes are not automatically resized. Numerical precision remains limited by f64 arithmetic.
- OPEN `SHARED` is accepted without OS sharing/locking enforcement. Both OPEN syntaxes are accepted; syntax style alone does not select record semantics.
- Extra LOCATE cursor parameters and COLOR border parameters are parsed and discarded. WIDTH updates logical bounds but does not resize the terminal/screen buffer or change PRINT's fixed calculations. `SCREEN(row, col, flag)` ignores the flag and returns the character byte, not an attribute.

Detailed syntax and the exact extent of these limitations are in [procedures](procedures.md), [types](user-defined-types.md), [MAT](mat-operations.md), [files](file-io.md), [console](console.md), and [language reference](language-reference.md).

## Features without implemented support

| Category | Examples / boundary |
|---|---|
| Graphics | SCREEN mode-changing statement, PSET, PRESET, LINE graphics, CIRCLE, PAINT, DRAW, image GET/PUT, palettes/pages. The SCREEN **function** and file GET/PUT have separate supported meanings. |
| Audio | SOUND, PLAY; BEEP only writes a terminal bell. |
| Memory/hardware | DEF SEG, PEEK, POKE, INP, OUT, machine-language CALL interfaces, hardware port control. |
| Event traps | ON TIMER, ON KEY, ON PLAY and event-driven callbacks. TIMER itself is available. |
| Modules/program management | CHAIN, MERGE, `$INCLUDE`, external linking, RENUM, and BASIC LOAD/SAVE statements. REPL LOAD/SAVE support numbered-source persistence only. `$INCLUDE` written inside a comment is simply ignored. See [modules](multi-module.md). |
| General expressions | Standalone matrix expressions are unsupported. Numeric MIN/MAX accepts individual expressions rather than whole arrays. The [builtin inventory](builtins.md) is exhaustive. |
| Full array/record model | Runtime bounds/rank enforcement, whole-array argument binding, arrays inside TYPE fields. |
| Device/file model | Device aliases, OS record locking, and printer routing (LPRINT aliases console PRINT). Supported file operations are enumerated in [file I/O](file-io.md). |
| Editor/debugger | Breakpoints, step/resume debugger, module browser, cross-file LSP definitions, formatting or rename LSP requests. |

Not every unsupported word is reserved. An unrecognized word may parse as a variable, a call, or an array access instead of receiving an “unsupported feature” diagnostic. Successful parsing alone does not establish runtime support.

## Remaining unknowns and verification limits

The repository uses unit tests, integration tests, source inspection, and focused interpreter probes. The following areas have remaining verification limits:

- **Binary formats:** field ordering, integer widths, string headers, padding, EOF behavior, and packed floats follow the documented Rice layouts. Microsoft Binary Format is unsupported.
- **Platforms and terminals:** host filesystem rules, terminal escape handling, raw keyboard input, extended keys, Unicode display width, and Windows-specific operations depend on the platform. The documentation audit did not exercise every supported host OS or terminal.
- **Numerical edges:** overflow, underflow, NaN/infinity propagation, enormous counts/dimensions, and ill-conditioned matrix inversion are not exhaustively tested. There is no published precision guarantee or resource-limit specification beyond the explicit implementation checks.
- **Nested state interactions:** recursion with STATIC/SHARED, unusual nested branches, redefinitions across immediate submissions, and mixed structured/classic exception handling are not exhaustively tested. Known constraints are documented on their owning reference pages.
- **Editor integrations:** the stdio LSP implementation is tested locally at the source level. Every editor's BASIC grammar, plugin setup, and protocol integration has not been verified; configuration depends on the editor.

When resolving another discrepancy, record a small BASIC example, expected output/error, observed output/error, and host-dependent conditions. Add a targeted regression assertion when changing behavior, and update the relevant reference page plus this index if the issue changes a documented runtime limit.

## Evidence and maintenance

Primary implementation sources are [`lexer.rs`](../src/lexer.rs), [`parser.rs`](../src/parser.rs), [`interpreter.rs`](../src/interpreter.rs), [`builtins.rs`](../src/builtins.rs), [`environment.rs`](../src/environment.rs), and [`value.rs`](../src/value.rs). Existing assertions are in [`tests/integration.rs`](../tests/integration.rs) and source unit tests. Run `cargo test` to check them. Documentation intentionally follows executable branches rather than relying on stale code comments or the familiarity of a BASIC keyword.
