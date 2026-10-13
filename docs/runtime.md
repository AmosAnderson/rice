# Running Rice, the REPL, and editor integration

This page describes Rice BASIC 0.15.1. See the [quick start](quickstart.md), [language specification](language-reference.md), and [dialect selection rules](dialects.md).

## Build and command line

Build from the repository with a Rust toolchain supporting edition 2024:

```sh
cargo build --release --bin rice
cargo build --release --bin rice-lsp
cargo test
```

The executables are `target/release/rice` and `target/release/rice-lsp` (`.exe` on Windows). For development, `cargo build` builds the package binaries, `cargo run` selects the `rice` binary, and output goes in `target/debug/`.

```text
rice [--compat] [--dialect ansi|qb|qbasic|quickbasic] [--] [source-file [program-argument ...]]
```

With no file, Rice starts the REPL. With a file, it reads UTF-8 source and executes it once. The extension is not enforced. Default dialect is QB. Options may precede or follow the filename, and the last CLI dialect option wins. Dialect argument values are case-insensitive; option names are case-sensitive. A recognized `OPTION DIALECT` in source overrides the CLI selection for the entire source unit.

The first positional argument is the source filename; subsequent positional arguments are BASIC program arguments. `--` stops interpreter option processing. After that marker, even `--dialect`, `--compat`, and another `--` are passed as ordinary arguments; if no filename has appeared yet, the first argument after the marker is the filename. Use the marker to pass arguments beginning with `-`, or to open a filename such as `-program.bas` with `rice -- -program.bas`.

```sh
cargo run -- program.bas
cargo run -- --dialect ansi program.bas
cargo run -- program.bas First "two words" -- --flag
cargo run -- --compat
```

`COMMAND$()` returns only the program arguments, joined with one space between arguments. In the third example it returns `First two words --flag`. Case, Unicode, and whitespace inside arguments are preserved; shell quoting is not reconstructed. Empty arguments contribute empty fields, so `one`, an empty argument, and `two` become `one  two`. A single empty argument and no arguments both produce `""`. This string cannot recover the original argument boundaries. `COMMAND()` is an alias; bare `COMMAND$` remains an ordinary variable.

`--help` and `--version` are not implemented. Missing/unknown dialect values, unknown options before `--`, source read failures, and BASIC errors exit with status 1; successful file execution exits with 0. Extra positional arguments are passed to the program rather than treated as additional files. A missing dialect value remains an error even after a filename or program arguments; `--dialect --` reports an unknown dialect value rather than treating that `--` as a separator.

The CLI runs the interpreter on an 8 MiB stack thread to accommodate large debug-mode interpreter frames. This is not an unlimited recursion guarantee. Host stack/memory exhaustion is not a catchable BASIC exception.

## Release downloads and publishing

[GitHub Releases](https://github.com/AmosAnderson/rice/releases) provides archives containing `rice`, `rice-lsp`, the documentation, and the MIT license. Extract the archive for your platform and put the executables on your `PATH`, or run them by their full paths.

| Platform | Archive suffix | Build host |
|---|---|---|
| Linux x64 | `x86_64-unknown-linux-gnu.tar.gz` | Ubuntu 22.04 (glibc; not a static/musl build) |
| Linux ARM64 | `aarch64-unknown-linux-gnu.tar.gz` | Ubuntu 22.04 ARM64 (glibc; not a static/musl build) |
| Windows x64 | `x86_64-pc-windows-msvc.zip` | Windows Server 2022 |
| macOS Apple Silicon | `aarch64-apple-darwin.tar.gz` | macOS 15 |
| macOS Intel | `x86_64-apple-darwin.tar.gz` | macOS 15 |

For a Raspberry Pi running 64-bit Linux, choose `aarch64-unknown-linux-gnu`. Check `uname -m` for `aarch64` and `getconf LONG_BIT` for `64`. The ARM64 archive does not run on a 32-bit ARM OS; no ARMv6/ARMv7 release archive is provided. Linux archives require the system's glibc runtime and `libgcc_s.so.1`; Rust itself need not be installed. These are dynamically linked builds, so the CPU architecture alone does not guarantee compatibility with every Linux distribution.

Archives are named `rice-<Cargo-version>-<target>.<extension>`. Each release includes `SHA256SUMS`; after downloading the archives, verify them with `sha256sum --check --ignore-missing SHA256SUMS` on Linux. On macOS use `shasum -a 256 <archive>`; on Windows use `Get-FileHash <archive> -Algorithm SHA256` and compare the result with the matching entry.

The [Release workflow](../.github/workflows/release.yml) runs when a tag is pushed. Tags identify commits rather than belonging to branches, so it checks that the tagged commit is in `origin/master` history; tags outside that history are skipped. Both annotated and lightweight tags work. Push the version commit to `master` before the tag, or push the branch and tag together:

```sh
# After updating Cargo.toml, Cargo.lock, and versioned documentation,
# and committing the release on master (replace X.Y.Z):
git tag -a vX.Y.Z -m "Rice BASIC X.Y.Z"
git push --atomic origin master vX.Y.Z
```

Each native runner installs stable Rust, runs `cargo test --locked`, and builds both optimized binaries from the validated commit using the lockfile. Linux ARM64 is tested and built natively on `ubuntu-22.04-arm`. A release is published with generated notes and checksummed archives only after all five builds pass. The workflow checks the tag again before publication. It uses the repository's automatic `GITHUB_TOKEN`; no extra secret is required. Only the publishing job has write permission.

To release a tag that predates this workflow, or retry a release, use **Actions → Release → Run workflow** on `master` and supply the existing tag. The equivalent CLI command is:

```sh
gh workflow run release.yml --ref master -f tag=v0.15.1
```

Manual runs enforce the same master-history check and build the tagged source. A new release is staged as a draft until all assets are uploaded. Retries resume uploads to an existing draft; an already published release is left unchanged. Publish a new tag to distribute changed binaries. Push release tags individually: [GitHub does not emit tag-push events](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#push) when more than three tags are pushed together; use manual runs for those tags.

## REPL state and program editing

Immediate unnumbered input executes as soon as a complete statement/block is entered. Variables, procedures, types, options, and open files persist between immediate inputs. A leading decimal line number stores or replaces a program line; the line is not executed until `RUN`. Stored lines are sorted numerically. The parser currently rejects labels on block terminators such as `NEXT` and `END IF`, so a conventional stored multiline numbered loop cannot run as written. Enter such blocks immediately without numbers, or use unnumbered terminators in a source file. Colon-separated block headers/terminators are not a workaround. File execution, by contrast, follows source order, even when numeric labels are out of order.

| REPL command/input | Effect |
|---|---|
| `10 PRINT "Hello"` | Store/replace line 10. Whitespace between number and text is optional. |
| `10` | Delete line 10. |
| `RUN` | Execute stored lines in ascending order using a **new interpreter**, preserving the selected dialect. Clears previous variables, open files, procedures, and other interpreter state. An empty stored program returns silently and does not reset state. |
| `LIST` | List all stored lines. |
| `LIST 20`, `LIST 10-50`, `LIST -50`, `LIST 20-` | List one line or an inclusive range. |
| `DELETE 20`, `DELETE 10-50`, `DELETE 20-` | Delete one line or an inclusive range. Bare `DELETE`, an omitted lower bound, and reversed ranges are errors. |
| `NEW` | Clear stored program lines only; immediate interpreter state remains. |

These commands are REPL operations, not BASIC statements in source files. There is no `LOAD`, `SAVE`, `RENUM`, `RUN "file"`, or resumable debugger. Use an editor and file execution for persistent programs.

Block detection accumulates `FOR`, `WHILE`, `DO`, block `IF`, `SELECT CASE`, `SUB`, `FUNCTION`, block `DEF`, `TYPE`, and `WHEN` until their closing keywords. A comma-separated NEXT list closes one FOR per listed variable, so `NEXT j, i` closes two nested FOR blocks. The continuation prompt is `. ` with suggested indentation. Numbered lines bypass block accumulation. Detection is a lexical nesting heuristic; it is not a second full parser. While a block is being entered, text is part of the block and REPL commands are not intercepted.

Top-level immediate `SYSTEM`/`QUIT` or `END` causes the REPL to exit after successful execution. `STOP` ends that submission but leaves the REPL open. Exit detection scans top-level parsed statements rather than the actual executed path: an unreachable top-level `END`/`SYSTEM` can still close the REPL; an `END` inside a nested block ends execution without closing it. `END`/`SYSTEM` in a stored `RUN` program returns to the prompt. Ctrl+D and Ctrl+C during line editing exit the REPL, including while entering a block; Ctrl+C is not a “discard block” command.

History is loaded/saved at `$HOME/.rice_history`, falling back to `$USERPROFILE/.rice_history`, then `./.rice_history`; history I/O failures are ignored. Syntax highlighting uses 24-bit ANSI terminal colors. Terminal behavior depends on the host terminal; see [console](console.md).

Every immediate submission uses `Interpreter::run_source`; it is parsed independently and prescanned for definitions and DATA before execution. Definitions persist, and new DATA is appended to the existing DATA pool. Labels should be kept within one complete submission or stored program. `RUN` resets this accumulated state; `NEW` does not. An immediate dialect directive changes the dialect for subsequent submissions. For predictable block highlighting, enter that directive separately before starting a block. REPL interpreters have no program arguments, so `COMMAND$()` returns `""`, including during stored `RUN` execution.

## Host statements and paths

Both dialects expose these host operations except the QB-only setters marked below. All relative file/directory paths use the **process working directory**, not the directory containing the `.bas` file. `CHDIR` changes that directory for later operations and `SHELL`; `CURDIR$()` reports it. Running `rice examples/demo.bas` does not change into `examples`.

| Syntax | Implemented behavior |
|---|---|
| `NAME old$ AS new$` | Host filesystem rename; replacement behavior follows the host OS. |
| `KILL path$` | Remove one file; no wildcard expansion. |
| `MKDIR path$` | Create one directory; does not recursively create parents. |
| `RMDIR path$` | Remove an empty directory. |
| `CHDIR path$` | Change the host process working directory. |
| `CHDRIVE drive$` | Takes the first character and attempts to change to `character:\`; empty string is a no-op. This is a Windows-style path operation, not a portable drive abstraction. |
| `FILES [path$]` | List directory entry names, one per line, in host enumeration order. Defaults to `.`. A non-directory path causes listing of its parent; a wildcard-looking basename is not used as a filter. Entry errors are skipped. |
| `SHELL [command$]` | Run `cmd /c command` on Windows, `sh -c command` otherwise; wait for completion. No argument is a no-op. Nonzero child exit status is ignored; failure to launch raises an error. Child input/output use host streams. |
| `ENVIRON "name=value"` (QB only) | Set an interpreter override read by `ENVIRON$` and passed to `SHELL`; does not alter the host environment. |
| `DATE$ = value$`, `TIME$ = value$` (QB only) | Set interpreter read overrides, not the OS clock. See [builtins](builtins.md). |
| `SLEEP [seconds]` | Truncate to a checked integer and sleep only if positive. Missing, zero, and negative values return immediately; it is not a wait-for-key operation. |

Filesystem errors map to BASIC error codes where implemented; see [error handling](error-handling.md). `COMMAND$()` reads the program arguments described above, excluding the filename and interpreter options; see [builtins](builtins.md).

## Language server

`rice-lsp` speaks LSP over stdin/stdout. Configure an editor with BASIC language support to start the binary. It provides:

- Lexer/parser diagnostics on open and full-document change. It does not execute code or diagnose runtime type, bounds, or file errors. A parse failure currently prevents symbol extraction for that document.
- Keyword, builtin, type, and current-document symbol completions; completion lists are not filtered by dialect or lexical scope.
- Hover documentation for recognized keywords, builtins, and extracted symbols.
- Go-to-definition within the current document using extracted symbols, with UTF-16 cursor positions. It is not a cross-file or scope-aware resolver.

The server derives dialect from `OPTION DIALECT` in the document, otherwise uses QB. It does not inherit a separate `rice --dialect ansi` invocation and exposes no dialect initialization setting. Put a directive in ANSI source for consistent editor diagnostics. There is no formatting, rename, semantic-token, workspace-index, or debugging capability advertised by the server. Editor-specific BASIC grammar/extension support is separate from this server.

### Helix example

Add to `~/.config/helix/languages.toml`, replacing the command with an absolute executable path if it is not on `PATH`:

```toml
[[language]]
name = "basic"
scope = "source.basic"
file-types = ["bas"]
language-servers = ["rice-lsp"]
comment-token = "'"
indent = { tab-width = 4, unit = "    " }

[language-server.rice-lsp]
command = "rice-lsp"
```

### Other editors

Use a stdio language-server configuration pointing at the compiled executable and associate `.bas` with the editor's BASIC language. Merely mapping a file extension does not install an editor grammar or register an arbitrary server. The repository does not ship a Zed extension, VS Code extension, or verified configuration for every editor; the old standalone Zed settings example was not sufficient to establish that integration.

## Library use and verification

The public Rust modules expose `Interpreter::new()`, `Interpreter::with_io(output, input)`, `run_source`, `run_file`, and lower-level lexer/parser APIs. Set `interpreter.dialect` before `run_source` for a default; an explicit source directive can override it. `run_file` reads a UTF-8 file; `run_source` runs a string; `run_program` accepts a parsed AST and does not itself select a lexer/parser dialect. `with_io` is useful for deterministic input and captured output and defaults to noninteractive console behavior.

New interpreters return `""` from `COMMAND$()` and do not inspect the embedding process's arguments. Configure them explicitly with `interpreter.set_command_line_args(["First", "two words"]);`. The setter accepts an iterator of string-like values and replaces the joined argument text. These arguments are local to the interpreter and survive subsequent `run_source`, `run_file`, and BASIC `CLEAR` calls. Passing an empty iterator clears them. A standalone builtin registry has no program context and returns `""` for COMMAND$()/COMMAND() too; both calls require zero arguments.

Useful checks are `cargo test --lib`, `cargo test --test integration`, `cargo test --test cli`, and `cargo test --bin rice-lsp`. Integration fixtures are in [`tests/programs`](../tests/programs), with assertions in [`tests/integration.rs`](../tests/integration.rs); [`tests/cli.rs`](../tests/cli.rs) covers process argument handling. Passing these tests establishes Rice regression behavior, not complete QBasic 1.1 or ANSI conformance.
