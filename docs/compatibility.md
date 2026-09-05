# Compatibility gaps, quirks, and unknowns

This page records Rice BASIC **0.14.0 implementation behavior**. QBasic is the default; `OPTION DIALECT "ANSI"` selects the ANSI-style dialect. Neither name establishes full conformance to QBasic 1.1 or ANSI X3.113-1991. The complete Rice specification is the [core reference](language-reference.md), [builtins](builtins.md), [dialect table](dialects.md), and linked topic guides.

“Implemented” means supported by the parser/runtime described here. “Partial” means accepted syntax or familiar names have a smaller or different effect. “Unverified” means there is no established compatibility guarantee; it does not assert a known failure.

## Porting differences verified in the implementation

| Area | Rice behavior and implication |
|---|---|
| Default array bounds | Lower bound is **1 in both modes**, including QB. Specify `OPTION BASE 0` or explicit bounds when porting zero-based programs. |
| Numeric types | All numeric values are `f64`. QB `%`, `&`, `!`, `#` distinguish variable names; they do not enforce integer width, overflow, or single precision during ordinary assignment. Type metadata matters for some binary I/O. |
| Assignment types | Ordinary scalar and array assignment can replace a value with another type despite a suffix or AS declaration. UDT fields also lack strict assignment enforcement. Declarations/defaults and I/O conversions do not form a static type system. |
| Array storage | Declared rank/bounds are recorded but not checked for ordinary indexing. Indices truncate. Flattened keys can collide with user names: `a(1)` and `a_1` share storage. Missing unsuffixed primitive-array elements default to numeric zero even after `AS STRING`; prefer `$` names. |
| Array reset/resizing | ERASE/CLEAR/REDIM have Rice-specific metadata and content retention rules. `REDIM PRESERVE` is not constrained like a historical BASIC array reallocation. See [arrays](language-reference.md). |
| Arithmetic | `MOD` works on reals; integer division `\`, `IMP`, and `EQV` are absent. QB bitwise operators use signed 64-bit casts, not a 16-bit integer machine. Logical operands are evaluated eagerly. Floating-point math may produce NaN/infinity. |
| Output | PRINT/STR$ omit leading/trailing positive-number spaces. PRINT commas use 16-character zones. PRINT USING implements a subset of formatting behavior; see [formatting](print-using.md). |
| Text | Most substring operations count Unicode scalar values. Binary strings use byte-to-character mapping, while screen tracking uses UTF-8 bytes. There is no DOS code-page or grapheme-width model. |
| Quoted literals | Doubled quotes are not a string-literal escape; use `CHR$(34)`. A pair of adjacent literals can become adjacent PRINT expressions instead. Backslash is literal text. `!` is not a comment marker in either mode. |
| Function spelling | `RND()`, `PI()`, `MAXNUM()`, `CURDIR$()`, and `COMMAND$()` need parentheses; bare names read variables. Conversely TIMER/FREEFILE require bare keyword form. Builtin lookup also accepts some omitted `$` aliases. See [calling conventions](builtins.md). |
| Procedure calls | BYREF is copy-in/copy-out of plain variables. Array elements, fields, and parenthesized expressions do not receive writeback. Two arguments referencing the same variable are not live aliases. Array-parameter syntax is parsed but does not bind arrays. |
| Scope | Unqualified reads can fall through caller environments; ordinary writes become local unless SHARED. Parameter AS and FUNCTION return AS are annotations rather than enforced types. Return defaults depend on the function name. See [procedures](procedures.md). |
| Constants/declarations | OPTION EXPLICIT takes effect when executed and has declaration/suffix exceptions. CLEAR preserves constants and declaration metadata. Some internal write paths can silently retain constants rather than raising assignment errors. See [core reference](language-reference.md). |
| DATA and labels | DATA/definitions in eligible nested blocks are prescanned even if those blocks never execute. RESTORE to an unknown DATA label rewinds instead of raising an error. Numeric labels do not sort a source file; only stored REPL lines are sorted. Labels on block terminators such as NEXT/END IF are rejected, which also limits stored numbered REPL blocks. |
| Classic handlers | ON ERROR/RESUME are QB-only and controlled by top-level execution. Nested failures resume at enclosing top-level statements rather than providing arbitrary statement-level continuations. Structured handler state is a shared slot, not a nested exception stack. See [errors](error-handling.md). |
| Binary files | QB GET/PUT uses Rice's type-driven little-endian record serialization; ANSI GET/PUT uses a much simpler text-oriented path. Do not assume files are interchangeable across modes or with existing QBasic data. |
| Relative paths | File and directory operations use process cwd, not source-file directory. FILES does not filter wildcard patterns; KILL does not expand them. |
| Process/clock | QB-only ENVIRON and DATE$/TIME$ assignments are interpreter-local overrides. COMMAND$ does not provide a usable extra-argument CLI contract. SHELL ignores child exit status. SLEEP without a positive argument does not wait for a key. |
| REPL state | NEW clears stored lines only. RUN uses a fresh interpreter. Immediate END/SYSTEM exit detection has top-level syntax quirks. See [runtime](runtime.md). |

## Accepted syntax with limited or ignored effects

These forms should not be mistaken for complete historical features:

- `DECLARE` is parsed but does not link external code or enforce signatures. Procedure array markers and some type annotations do not affect argument binding. `COMMON` and `COMMON SHARED` have the same single-program sharing behavior; no CHAIN transfer exists.
- `STRING * n` records fixed-length metadata, but ordinary string assignment does not pad/truncate to that size. UDT binary serialization is where fixed field sizes matter.
- MAT supports two-dimensional numeric arrays. MAT PRINT/INPUT file-channel syntax is parsed but still uses the console. There is no one-dimensional MAT vector support or standalone `DET(matrix)` function.
- OPEN `SHARED` is accepted without OS sharing/locking enforcement. Both OPEN syntaxes are accepted in both dialects; syntax style alone does not select record semantics.
- Extra LOCATE cursor parameters and COLOR border parameters are parsed and discarded. WIDTH updates logical bounds but does not resize the terminal/screen buffer or change PRINT's fixed calculations. `SCREEN(row, col, flag)` ignores the flag and returns the character byte, not an attribute.
- Unknown string values of `OPTION DIALECT` are consumed without changing mode. A recognized directive is detected before execution and applies to the whole source unit; it is not a runtime mode switch between sections.

Detailed syntax and the exact extent of these limitations are in [procedures](procedures.md), [types](user-defined-types.md), [MAT](mat-operations.md), [files](file-io.md), [console](console.md), and [dialects](dialects.md).

## Features without implemented support

| Category | Examples / boundary |
|---|---|
| Graphics | SCREEN mode-changing statement, PSET, PRESET, LINE graphics, CIRCLE, PAINT, DRAW, image GET/PUT, palettes/pages. The SCREEN **function** and file GET/PUT have separate supported meanings. |
| Audio | SOUND, PLAY; BEEP only writes a terminal bell. |
| Memory/hardware | DEF SEG, PEEK, POKE, INP, OUT, machine-language CALL interfaces, hardware port control. |
| Event traps | ON TIMER, ON KEY, ON PLAY and event-driven callbacks. TIMER itself is available. |
| Modules/program management | CHAIN, MERGE, `$INCLUDE`, external linking, LOAD/SAVE/RENUM commands. `$INCLUDE` written inside a comment is simply ignored. See [modules](multi-module.md). |
| General expressions | Integer division, IMP, EQV, general-purpose MAX/MIN builtins, standalone matrix expressions, implied complete support for all ANSI math/string functions. The [builtin inventory](builtins.md) is exhaustive. |
| Full array/record model | Runtime bounds/rank enforcement, whole-array argument binding, arrays inside TYPE fields, a binary-compatible general record model. |
| Device/file model | Historical device aliases, OS record locking guarantees, printer routing (LPRINT aliases console PRINT), complete QBasic file-mode/error compatibility. Supported file operations are enumerated in [file I/O](file-io.md). |
| Editor/debugger | Breakpoints, step/resume debugger, module browser, cross-file LSP definitions, formatting or rename LSP requests. |

Not every unsupported word is reserved. An unrecognized word may parse as a variable, a call, or an array access instead of receiving an “unsupported feature” diagnostic. Do not use successful parsing alone as a compatibility test.

## Remaining unknowns and verification limits

The repository has unit/integration tests, not a complete external conformance suite. This audit checked source and regression tests and used focused interpreter probes; it did not run original QBasic 1.1 side by side or validate every clause of the ANSI standard. In particular:

- **External program coverage:** no claim is made that arbitrary QBasic or ANSI programs run unchanged. Unlisted standard features are not implicitly supported. Existing file/statement tests define tested cases, not exhaustive compatibility.
- **Binary interoperability:** field ordering, integer widths, string headers, padding, EOF behavior, and packed floats are documented, but interchange with a corpus of original QBasic files remains unverified. Microsoft Binary Format is unsupported.
- **Platforms and terminals:** host filesystem rules, terminal escape handling, raw keyboard input, extended keys, Unicode display width, and Windows-specific operations depend on the platform. The documentation audit did not exercise every supported host OS or terminal.
- **Numerical edges:** overflow, underflow, NaN/infinity propagation, enormous counts/dimensions, and ill-conditioned matrix inversion are not comprehensively characterized against either standard. There is no published precision guarantee or resource-limit specification beyond the explicit implementation checks.
- **Nested state interactions:** recursion with STATIC/SHARED, unusual nested branches, redefinitions across immediate submissions, and mixed structured/classic exception handling are not exhaustively tested. Known constraints are documented; absence of a listed counterexample is not proof of conformance.
- **Editor integrations:** the stdio LSP implementation is tested locally at the source level. Every editor's BASIC grammar, plugin setup, and protocol integration has not been verified; configuration depends on the editor.

When resolving another discrepancy, record a small BASIC example, selected dialect, expected output/error, observed output/error, and host-dependent conditions. Add a targeted regression assertion when changing behavior, and update the relevant reference page plus this index if the issue affects porting.

## Evidence and maintenance

Primary implementation sources are [`lexer.rs`](../src/lexer.rs), [`parser.rs`](../src/parser.rs), [`interpreter.rs`](../src/interpreter.rs), [`builtins.rs`](../src/builtins.rs), [`environment.rs`](../src/environment.rs), and [`value.rs`](../src/value.rs). Existing assertions are in [`tests/integration.rs`](../tests/integration.rs) and source unit tests. Run `cargo test` to check them. Documentation intentionally follows executable branches rather than relying on stale code comments or the familiarity of a BASIC keyword.
