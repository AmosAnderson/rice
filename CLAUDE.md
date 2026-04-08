# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Rules

- **Never commit or push code without explicit user approval.** Always ask before running `git commit`, `git push`, or any command that modifies the git history.

## Project Overview

RICE BASIC is an ANSI X3.113-1991 (Full BASIC) interpreter written in Rust. No graphics or sound support. Supports interactive REPL and file execution.

## Build & Test Commands

```bash
cargo build                    # Build
cargo run                     # Start REPL
cargo run -- file.bas          # Execute a .bas file
cargo test                    # Run all tests (unit + integration)
cargo test --lib              # Run unit tests only
cargo test --test integration  # Run integration tests only
cargo test test_hello          # Run a single test by name
cargo build --bin rice-lsp     # Build the language server (stdio-based)
```

Rust edition 2024 (`Cargo.toml`). Uses `thiserror` for error types, `rustyline` for REPL, `crossterm` for terminal manipulation, `pretty_assertions` and `tempfile` for tests, `tower-lsp`/`tokio`/`serde_json` for the LSP server.

REPL and file execution share interpreter code paths; keep behavior parity between them. The REPL features 24-bit ANSI syntax highlighting, automatic multi-line block detection (FOR/NEXT, IF/END IF, SUB/END SUB, etc.), and old-school line number program editing (type numbered lines to build a program, then RUN/LIST/NEW/DELETE).

## Architecture

`Source → Lexer → Tokens → Parser → AST → Tree-Walking Interpreter → Output`

All hand-written (no parser generators). Interpreter only -- no compiler backend.

### Module Map

- **`token.rs`** — Token enum, Span. All identifiers stored UPPERCASE. No type suffixes -- ANSI Full BASIC uses only NUMERIC and STRING types.
- **`lexer.rs`** — Hand-written tokenizer. Case-insensitive. Detects line numbers at line start. Recognizes compound keywords (`END IF`, `END SUB`, `END FUNCTION`, `END SELECT`, `END TYPE`, `END WHILE`, `END WHEN`, `LINE INPUT`, `SELECT CASE`, `OPTION BASE`).
- **`ast.rs`** — `Stmt` and `Expr` enums. `LabeledStmt` wraps statements with optional line labels. Key types: `PrintStmt`, `IfStmt`, `ForStmt`, `DoLoopStmt`, `SelectCaseStmt`, `SubDef`, `FunctionDef`, `WhenExceptionStmt`.
- **`parser.rs`** — Recursive descent. Expression parsing uses precedence climbing (XOR → OR → AND → NOT → comparison → +/-/&(additive+concat) → MOD → */÷ → unary → ^). `at_stmt_end()` also treats `ELSE` as a terminator for single-line IF support.
- **`interpreter.rs`** — Tree-walking evaluator. Uses `ControlFlow` enum (Normal, ExitFor, ExitDo, ExitSub, ExitFunction, Goto, End, Retry, Continue) for control flow. `SharedOutput` wrapper enables testable output capture. `FileHandle` struct manages open files with `BufReader`/`BufWriter` for text and binary I/O. `ExceptionInfo` struct tracks EXTYPE and EXTEXT$ for WHEN EXCEPTION error handling.
- **`format_using.rs`** — PRINT USING format engine. Supports numeric specifiers (`#`, `.`, `+`, `-`, `$$`, `**`, `**$`, `,`, `^^^^`) and string specifiers (`!`, `\ \`, `&`). Escape with `_`. Overflow prefix `%`.
- **`environment.rs`** — `Rc<RefCell<Environment>>` scope chain. Variable key = name (no suffixes). Label map stored here. Supports `shared_vars` set for SHARED keyword (reads/writes go to root scope). Constants are checked through the parent chain to prevent reassignment. Default OPTION BASE is 1.
- **`value.rs`** — `Value` enum (Numeric, Str, Record). Only two primitive types: NUMERIC (f64) and STRING. No leading space on positive numbers in PRINT output. 16-char zone width for comma-separated PRINT.
- **`mat.rs`** — MAT (matrix) operations. Element-wise arithmetic (add, subtract), matrix multiply, scalar multiply, INV (inverse), TRN (transpose), DET (determinant), ZER (zero matrix), CON (ones matrix), IDN (identity matrix).
- **`builtins.rs`** — Built-in function registry. Math (ABS, INT, FIX, SGN, SQR, SIN, COS, TAN, ATN, EXP, LOG, ROUND, ASIN, ACOS, COT, CSC, SEC, ANGLE, CEIL, TRUNCATE, REMAINDER, MAXNUM, PI), string (LEN, INSTR, LTRIM$, RTRIM$, SPACE$, STRING$, CHR$, ASC, STR$, VAL, LEFT$, RIGHT$, MID$, UCASE$, LCASE$, HEX$, OCT$), system (ENVIRON$, TIMER, DATE$, TIME$). Note: FREEFILE, EOF, LOF, LOC are implemented directly in `interpreter.rs` (not in the builtin registry).
- **`repl.rs`** — Interactive REPL using rustyline. Environment persists across immediate-mode lines. Supports old-school line-number program editing: numbered lines are stored in a `BTreeMap<u32, String>`, RUN reconstructs source and executes with a fresh interpreter, LIST/NEW/DELETE manage the stored program. Input classification (`classify_input`) runs before multi-line block accumulation so numbered lines bypass depth tracking.
- **`error.rs`** — `LexError`, `ParseError`, `RuntimeError` enums via `thiserror`. `RuntimeError::IoError` carries error codes for file/directory operations.
- **`bin/rice_lsp.rs`** — LSP server binary (stdio transport, `tower-lsp`). Provides diagnostics, completions, hover documentation, and go-to-definition.
- **`main.rs`** — CLI: no args → REPL, one arg → execute file.
- **`lib.rs`** — Module declarations. Also provides shared utility functions: `poll_inkey()` for non-blocking key reading via crossterm, `update_screen_buffer()` for tracking printed characters in an 80x25 buffer (used for `SCREEN()` support).

### Key Design Decisions

- **Type system**: Only NUMERIC (f64) and STRING. No type suffixes. Variables ending in `$` are string; all others are numeric.
- **`=` disambiguation**: at statement level `=` is assignment; inside expressions `=` is comparison
- **Single-line vs block IF**: if tokens follow THEN on the same line, it's single-line
- **Auto-initialization**: undefined variables auto-initialize to 0 or "" (BASIC behavior)
- **`name(args)` ambiguity**: resolved at runtime -- check builtin registry, then user functions, then arrays
- **GOTO**: label map built during prescan; ControlFlow::Goto bubbles up to exec_block which resolves it
- **No GOSUB/RETURN**: not supported; use SUB/FUNCTION instead
- **Truth values**: true = `1`, false = `0` (ANSI BASIC convention); do not change
- **Logical operators**: AND, OR, NOT, XOR are logical (not bitwise). No IMP or EQV operators. No `\` integer division.
- **MOD**: works on real numbers (not integer-only)
- **String concatenation**: `&` operator (not `+`). `+` is always arithmetic.
- **String slicing**: colon syntax `A$(3:7)` instead of MID$/LEFT$/RIGHT$
- **Error handling**: WHEN EXCEPTION IN...USE...END WHEN with RETRY, CONTINUE, EXTYPE, EXTEXT$. No ON ERROR GOTO.
- **File I/O**: ANSI OPEN syntax: `OPEN #n: NAME "file", ACCESS INPUT/OUTPUT/OUTIN, ORGANIZATION SEQUENTIAL/STREAM`. SET/ASK POINTER instead of SEEK.
- **OPTION BASE**: defaults to 1 (ANSI convention), not 0
- **Parameters**: BYVAL by default (not BYREF)
- **PRINT formatting**: no leading space on positive numbers; 16-character zone width for comma-separated output
- **Prescan ordering**: `Interpreter::run_source` pre-scans labels, DATA, SUB/FUNCTION definitions before execution; prescan recurses into nested blocks (IF, FOR, WHILE, DO, SELECT CASE); preserve this ordering
- **Stack size**: `main.rs` spawns an 8MB-stack thread because debug-mode `match` arms in the interpreter create ~100KB frames, exhausting the default Windows 1MB stack

### Code Conventions

- Follow existing Rust style: `snake_case` for functions/fields, `CamelCase` for types/enums. Use `match` and `Result`-based error propagation.
- Prefer extending existing files (`parser.rs`, `interpreter.rs`, `value.rs`, `builtins.rs`) over adding new modules/abstractions.
- Module boundaries are declared in `src/lib.rs`; avoid unnecessary public API churn.
- All identifiers are normalized to UPPERCASE internally; preserve case-insensitive BASIC behavior.
- Arrays are currently implemented with flattened keys; avoid broad refactors without targeted tests.
- Builtins are centralized in `builtins.rs`; function resolution order: builtin → user-defined function → array.

### Test Programs

Integration tests in `tests/programs/*.bas` cover: hello world, arithmetic, variables, FizzBuzz, while loops, do/loops, select case, recursive factorial, string functions, DATA/READ, SUB calls, file I/O (text, append, FREEFILE, EOF, LOF), WRITE (console), SLEEP, CLEAR, file system operations (NAME, KILL, MKDIR, RMDIR, CHDIR), SHELL, ENVIRON$, SHARED, STATIC, RANDOMIZE/RND, TYPE (user-defined types with dot notation, arrays of TYPE, TYPE in SUB), MAT operations, WHEN EXCEPTION error handling, string slicing.

To add a new integration test: create a `.bas` file in `tests/programs/`, then add a test function in `tests/integration.rs` using one of these helpers:
- `run_file("tests/programs/foo.bas")` — load and execute a `.bas` file, returns captured output
- `run_bas("PRINT 42\n")` — parse/execute inline BASIC source
- `run_bas_with_tmpdir(src)` — execute with a temp directory; use `{DIR}` placeholder in source for paths
- `run_bas_may_fail(src)` — returns both output and `Result` for testing error conditions

The interpreter's `SharedOutput` captures PRINT output for assertion.

**Note on nondeterministic builtins**: TIMER, DATE$, TIME$, and RND produce varying output. Avoid asserting exact values for these in tests; test structure/format instead.

### Extending the Interpreter

**Adding a new statement:**
1. Add `Token::Kw*` variant to `token.rs`
2. Add `"KEYWORD" => Token::KwKeyword` in the lexer's keyword match table in `lexer.rs`
3. Add `Stmt::*` variant in `ast.rs`
4. Add `Token::Kw* => self.parse_*()` case in `parse_statement()` in `parser.rs`
5. Add `Stmt::* => ...` case in `exec_stmt()` in `interpreter.rs` (return `ControlFlow::Normal` for simple statements)
6. If the statement needs prescan (labels, data, definitions), add handling in the prescan phase

**Adding a builtin function:**
1. Write `fn builtin_name(args: &[Value]) -> Result<Value, RuntimeError>` in `builtins.rs`
2. Call `reg.register("NAME", builtin_name, arity)` in `BuiltinRegistry::new()` (use arity `0` for variadic)
3. Use `args[n].to_f64()?` or `.to_string_val()?` for type coercion; return `Value::Numeric(...)` or `Value::Str(...)`

**Adding a new error:**
1. Add variant to `LexError`, `ParseError`, or `RuntimeError` in `error.rs` with `#[error(...)]` attribute
2. For `RuntimeError`: add error code mapping in `io_error_to_basic_code()` if applicable

## Status of BASIC Features

**Working**: PRINT, PRINT USING, LET, DIM, CONST, INPUT, LINE INPUT, IF/ELSEIF/ELSE/END IF, FOR/NEXT, WHILE/END WHILE, DO/LOOP, SELECT CASE, GOTO, EXIT FOR/DO/SUB/FUNCTION, SUB/END SUB, FUNCTION/END FUNCTION, CALL, DECLARE, DATA/READ/RESTORE, SWAP, OPTION BASE (default 1), REDIM, ERASE, SHARED, STATIC, TYPE/END TYPE (user-defined types with dot notation, arrays of TYPE), RANDOMIZE/RND, WRITE (console), SLEEP, CLEAR, WHEN EXCEPTION IN/USE/END WHEN (with RETRY, CONTINUE, EXTYPE, EXTEXT$), string slicing with colon syntax (A$(3:7)), & string concatenation, MAT operations (MAT PRINT, MAT READ, MAT INPUT, MAT +/-/*, scalar multiply, INV, TRN, DET, ZER, CON, IDN), File I/O (ANSI OPEN with NAME/ACCESS/ORGANIZATION, CLOSE, PRINT#, INPUT#, LINE INPUT#, SET POINTER, ASK POINTER), file functions (FREEFILE, EOF, LOF, LOC), file system operations (NAME...AS, KILL, MKDIR, RMDIR, CHDIR), console features (CLS, LOCATE, COLOR, BEEP, WIDTH, VIEW PRINT, CSRLIN, POS, INKEY$, INPUT$, SCREEN()), SHELL, ENVIRON$, BYVAL parameter semantics, logical AND/OR/NOT/XOR, STOP, SYSTEM, END, QBasic string functions (LEFT$, RIGHT$, MID$, UCASE$, LCASE$, HEX$, OCT$), REPL line-number mode (RUN, LIST, NEW, DELETE).

**Not implemented**: proper array storage (currently uses flattened key hack), LBOUND/UBOUND (stubs only).
