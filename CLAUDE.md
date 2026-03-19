# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Rules

- **Never commit or push code without explicit user approval.** Always ask before running `git commit`, `git push`, or any command that modifies the git history.

## Project Overview

RICE BASIC is a structured BASIC interpreter written in Rust (QBasic/FreeBASIC dialect). No graphics or sound support. Supports both interactive REPL and file execution.

## Build & Test Commands

```bash
cargo build                    # Build
cargo run                     # Start REPL
cargo run -- file.bas          # Execute a .bas file
cargo run -- --compile file.bas        # Compile to native executable (outputs ./file)
cargo run -- --compile file.bas -o out # Compile with custom output path
cargo run -- --emit-ir file.bas        # Dump intermediate representation
cargo test                    # Run all tests (unit + integration)
cargo test --lib              # Run unit tests only
cargo test --test integration  # Run integration tests only
cargo test test_hello          # Run a single test by name
cargo build --bin rice-lsp     # Build the language server (stdio-based)
```

Rust edition 2024 (`Cargo.toml`). Uses `thiserror` for error types, `rustyline` for REPL, `crossterm` for terminal manipulation, `pretty_assertions` and `tempfile` for tests, `tower-lsp`/`tokio`/`serde_json` for the LSP server, `cranelift-*` crates for the native compiler backend.

REPL and file execution share interpreter code paths; keep behavior parity between them.

## Architecture

Two execution paths from a shared frontend:

1. **Interpreter**: `Source → Lexer → Tokens → Parser → AST → Tree-Walking Interpreter → Output`
2. **Compiler**: `Source → Lexer → Tokens → Parser → AST → RiceIR → Cranelift → Native Executable`

All hand-written (no parser generators). The compiler is at near-parity with the interpreter; the main exception is CHAIN (requires the interpreter).

### Module Map

- **`token.rs`** — Token enum, TypeSuffix (`% & ! # $`), Span. All identifiers stored UPPERCASE.
- **`lexer.rs`** — Hand-written tokenizer. Case-insensitive. Detects line numbers at line start. Recognizes compound keywords (`END IF`, `END SUB`, `LINE INPUT`). Attaches type suffixes to identifiers.
- **`ast.rs`** — `Stmt` and `Expr` enums. `LabeledStmt` wraps statements with optional line labels. Key types: `PrintStmt`, `IfStmt`, `ForStmt`, `DoLoopStmt`, `SelectCaseStmt`, `SubDef`, `FunctionDef`.
- **`parser.rs`** — Recursive descent. Expression parsing uses precedence climbing (IMP → EQV → XOR → OR → AND → NOT → comparison → +/- → MOD → \\ → */÷ → unary → ^). `at_stmt_end()` also treats `ELSE` as a terminator for single-line IF support.
- **`interpreter.rs`** — Tree-walking evaluator. Uses `ControlFlow` enum (Normal, ExitFor, ExitDo, ExitSub, ExitFunction, Goto, Gosub, Return, End, Resume, ResumeNext, Chain) for control flow. `SharedOutput` wrapper enables testable output capture. `FileHandle` struct manages open files with `BufReader`/`BufWriter` for text and binary I/O. Error handler state (error_handler, current_error, error_resume_pc) enables ON ERROR GOTO/RESUME. ERR and ERL are resolved as interpreter-state functions. Maintains `def_fns` map for DEF FN definitions, `static_vars` for STATIC variable persistence across calls, and `deftype_map` for DEFtype letter-range defaults.
- **`format_using.rs`** — PRINT USING format engine. Supports QBasic numeric specifiers (`#`, `.`, `+`, `-`, `$$`, `**`, `**$`, `,`, `^^^^`) and string specifiers (`!`, `\ \`, `&`). Escape with `_`. Overflow prefix `%`.
- **`environment.rs`** — `Rc<RefCell<Environment>>` scope chain. Variable key = name + suffix (`X%` and `X$` are different variables). GOSUB return stack and label map stored here. Supports `shared_vars` set for SHARED keyword (reads/writes go to root scope). Constants are checked through the parent chain to prevent reassignment.
- **`value.rs`** — `Value` enum (Integer, Long, Single, Double, Str). QBasic-style PRINT formatting (leading space for positive numbers). Type coercion ladder: Integer < Long < Single < Double.
- **`builtins.rs`** — Built-in function registry. Math (ABS, INT, SQR, SIN, etc.), string (LEFT$, MID$, LEN, etc.), conversion (CINT, VAL, STR$, etc.), binary conversion (MKI$/MKL$/MKS$/MKD$/CVI/CVL/CVS/CVD), system (ENVIRON$, TIMER, DATE$, TIME$).
- **`repl.rs`** — Interactive REPL using rustyline. Environment persists across lines.
- **`error.rs`** — `LexError`, `ParseError`, `RuntimeError` enums via `thiserror`. `RuntimeError::IoError` carries QBasic-compatible error codes for file/directory operations. `io_error_to_qbasic_code()` is the shared mapping function used by both interpreter and runtime.
- **`bin/rice_lsp.rs`** — LSP server binary (stdio transport, `tower-lsp`).
- **`main.rs`** — CLI: no args → REPL, one arg → execute file, `--compile` → native compilation, `--emit-ir` → dump IR.
- **`lib.rs`** — Module declarations. Also provides shared utility functions: `poll_inkey()` for non-blocking key reading via crossterm, `update_screen_buffer()` for tracking printed characters in an 80×25 buffer (used by both interpreter and runtime for `SCREEN()` support).
- **`compiler/`** — Native compiler backend (AST → machine code via Cranelift):
  - `mod.rs` — Public API (`compile_file`, `compile_source`, `emit_ir`); shared parse step.
  - `ir.rs` — `RiceIR`, a flat intermediate representation (typed instructions, basic blocks). Includes `CheckError`, `SetResumePoint`, and `ResumeDispatch` instructions for ON ERROR GOTO support.
  - `lower.rs` — `Lowerer`: AST → RiceIR translation. Inlines single-line DEF FN at call sites. Emits failable runtime calls with error checking when ON ERROR GOTO is active. CHAIN returns a compile-time error.
  - `cranelift_codegen.rs` — `CodeGenerator`: RiceIR → Cranelift IR → object file bytes.
  - `linker.rs` — Invokes system linker (`cc`) to produce final executable from object file.
- **`runtime/`** — C-ABI runtime library linked into compiled executables:
  - `value_ffi.rs` — extern "C" functions for Value creation, arithmetic, string ops, type coercion.
  - `io_ffi.rs` — extern "C" functions for PRINT, file I/O, console operations, error handling (error flag/resume point), and screen buffer tracking for SCREEN().

### Key Design Decisions

- **`=` disambiguation**: at statement level `=` is assignment; inside expressions `=` is comparison
- **Single-line vs block IF**: if tokens follow THEN on the same line, it's single-line
- **Auto-initialization**: undefined variables auto-initialize to 0 or "" (classic BASIC behavior)
- **`name(args)` ambiguity**: resolved at runtime — check builtin registry, then user functions, then arrays
- **GOTO/GOSUB**: label map built during prescan; ControlFlow::Goto bubbles up to exec_block which resolves it
- **Truth values**: true = `-1`, false = `0` (QBasic convention); do not change
- **Prescan ordering**: `Interpreter::run_source` pre-scans labels, DATA, SUB/FUNCTION, and DEF FN definitions before execution; prescan recurses into nested blocks (IF, FOR, WHILE, DO, SELECT CASE); preserve this ordering
- **Compiler runtime**: compiled executables link against `runtime/` (exposed as `staticlib`). Runtime functions use `extern "C"` ABI and are `#[no_mangle]` so the linker can resolve symbols emitted by Cranelift codegen
- **Stack size**: `main.rs` spawns an 8MB-stack thread because debug-mode `match` arms in the interpreter create ~100KB frames, exhausting the default Windows 1MB stack

### Code Conventions

- Follow existing Rust style: `snake_case` for functions/fields, `CamelCase` for types/enums. Use `match` and `Result`-based error propagation.
- Prefer extending existing files (`parser.rs`, `interpreter.rs`, `value.rs`, `builtins.rs`) over adding new modules/abstractions.
- Module boundaries are declared in `src/lib.rs`; avoid unnecessary public API churn.
- All identifiers are normalized to UPPERCASE internally; preserve case-insensitive BASIC behavior.
- Arrays are currently implemented with flattened keys; avoid broad refactors without targeted tests.
- Builtins are centralized in `builtins.rs`; function resolution order: builtin → user-defined function → array.

### Test Programs

Integration tests in `tests/programs/*.bas` cover: hello world, arithmetic, variables, FizzBuzz, while loops, do/loops, select case, gosub/return, recursive factorial, string functions, DATA/READ, SUB calls, file I/O (text, binary, append, WRITE#/INPUT# round-trip, FREEFILE, EOF, LOF), WRITE (console), SLEEP, CLEAR, file system operations (NAME, KILL, MKDIR, RMDIR, CHDIR), SHELL, ENVIRON$, MID$ assignment, LSET/RSET, SHARED, STATIC, DEFtype, DEF FN, date/time functions, binary conversion (MKI$/CVI etc.), ON n GOTO/GOSUB, RANDOMIZE/RND, TYPE (user-defined types with dot notation, arrays of TYPE, TYPE in SUB).

To add a new integration test: create a `.bas` file in `tests/programs/`, then add a test function in `tests/integration.rs` using one of these helpers:
- `run_file("tests/programs/foo.bas")` — load and execute a `.bas` file, returns captured output
- `run_bas("PRINT 42\n")` — parse/execute inline BASIC source
- `run_bas_with_tmpdir(src)` — execute with a temp directory; use `{DIR}` placeholder in source for paths
- `run_bas_may_fail(src)` — returns both output and `Result` for testing error conditions
- `run_chain_test(main_source, files)` — multi-file CHAIN test helper
- `run_chain_test_may_fail(main_source, files)` — CHAIN test with error handling

The interpreter's `SharedOutput` captures PRINT output for assertion.

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

**Adding a new error:**
1. Add variant to `LexError`, `ParseError`, or `RuntimeError` in `error.rs` with `#[error(...)]` attribute
2. For `RuntimeError`: add QBasic error code mapping in `qbasic_error_code()` if applicable

## Status of BASIC Features

**Working**: PRINT, PRINT USING, LET, DIM, CONST, INPUT, LINE INPUT, IF/ELSEIF/ELSE, FOR/NEXT, WHILE/WEND, DO/LOOP, SELECT CASE, GOTO, GOSUB/RETURN, EXIT FOR/DO/SUB/FUNCTION, SUB/FUNCTION definitions, CALL, DECLARE, DATA/READ/RESTORE, SWAP, all string/math/conversion builtins, ERR/ERL, OPTION BASE, REDIM, ERASE, File I/O (OPEN, CLOSE, PRINT#, WRITE#, INPUT#, LINE INPUT#, GET, PUT, FIELD, SEEK), file functions (FREEFILE, EOF, LOF, LOC, SEEK), ON ERROR GOTO/RESUME, ON n GOTO/GOSUB, RANDOMIZE/RND, WRITE (console), SLEEP, CLEAR, NAME/KILL/MKDIR/RMDIR/CHDIR, SHELL, ENVIRON$, MID$ (statement form), LSET/RSET, SHARED, STATIC, DEFtype (DEFINT/DEFLNG/DEFSNG/DEFDBL/DEFSTR), DEF FN, MKI$/MKL$/MKS$/MKD$/CVI/CVL/CVS/CVD, TYPE (user-defined types with dot-notation, STRING * n, arrays of TYPE), CHAIN/COMMON (multi-module programming), text/console features (CLS, LOCATE, COLOR, BEEP, WIDTH, VIEW PRINT, CSRLIN, POS, INKEY$, INPUT$, SCREEN()), BYVAL parameter semantics.

**Not implemented**: proper array storage (currently uses flattened key hack), LBOUND/UBOUND (stubs only), CHAIN in compiled mode (compile-time error; use interpreter).
