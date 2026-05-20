use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

use rice::ast::*;
use rice::lexer::Lexer;
use rice::parser::Parser;
use rice::token::SpannedToken;

// ---------------------------------------------------------------------------
// Document state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct SymbolInfo {
    name: String,
    line: usize, // 1-indexed (from AST)
    detail: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct DocumentSymbols {
    subs: Vec<SymbolInfo>,
    functions: Vec<SymbolInfo>,
    variables: Vec<SymbolInfo>,
    constants: Vec<SymbolInfo>,
    labels: Vec<SymbolInfo>,
}

struct DocumentState {
    tokens: Vec<SpannedToken>,
    diagnostics: Vec<Diagnostic>,
    symbols: DocumentSymbols,
}

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

struct RiceLspBackend {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, DocumentState>>>,
}

impl RiceLspBackend {
    fn new(client: Client) -> Self {
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn analyze(&self, uri: Url, source: String) {
        let state = analyze_source(source);
        let diagnostics = state.diagnostics.clone();
        self.documents.write().await.insert(uri.clone(), state);
        self.client
            .publish_diagnostics(uri, diagnostics, None)
            .await;
    }
}

fn analyze_source(source: String) -> DocumentState {
    let mut diagnostics = Vec::new();
    let mut lexer = Lexer::new(&source);

    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            let (line, col) = lex_error_pos(&e);
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position::new(
                        line.saturating_sub(1) as u32,
                        col.saturating_sub(1) as u32,
                    ),
                    end: Position::new(line.saturating_sub(1) as u32, col as u32),
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("rice".into()),
                message: e.to_string(),
                ..Default::default()
            });
            return DocumentState {
                tokens: vec![],
                diagnostics,
                symbols: DocumentSymbols::default(),
            };
        }
    };

    let mut parser = Parser::new(tokens.clone());
    let program = match parser.parse_program() {
        Ok(p) => Some(p),
        Err(e) => {
            let line = parse_error_line(&e);
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position::new(line.saturating_sub(1) as u32, 0),
                    end: Position::new(line.saturating_sub(1) as u32, u32::MAX),
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("rice".into()),
                message: e.to_string(),
                ..Default::default()
            });
            None
        }
    };

    let symbols = match program {
        Some(prog) => extract_symbols(&prog.statements),
        None => DocumentSymbols::default(),
    };

    DocumentState {
        tokens,
        diagnostics,
        symbols,
    }
}

fn lex_error_pos(e: &rice::error::LexError) -> (usize, usize) {
    match e {
        rice::error::LexError::UnterminatedString { line, col } => (*line, *col),
        rice::error::LexError::UnexpectedChar { line, col, .. } => (*line, *col),
        rice::error::LexError::InvalidNumber { line, col } => (*line, *col),
    }
}

fn parse_error_line(e: &rice::error::ParseError) -> usize {
    match e {
        rice::error::ParseError::Expected { line, .. } => *line,
        rice::error::ParseError::Unexpected { line, .. } => *line,
        rice::error::ParseError::General { line, .. } => *line,
    }
}

// ---------------------------------------------------------------------------
// Symbol extraction
// ---------------------------------------------------------------------------

fn extract_symbols(stmts: &[LabeledStmt]) -> DocumentSymbols {
    let mut syms = DocumentSymbols::default();
    let mut seen_vars = HashSet::new();
    collect_symbols(stmts, &mut syms, &mut seen_vars);
    syms
}

fn collect_symbols(
    stmts: &[LabeledStmt],
    syms: &mut DocumentSymbols,
    seen_vars: &mut HashSet<String>,
) {
    for ls in stmts {
        // Labels
        if let Some(ref label) = ls.label {
            syms.labels.push(SymbolInfo {
                name: label.to_string(),
                line: ls.line,
                detail: Some("Label".into()),
            });
        }

        match &ls.stmt {
            Stmt::SubDef(sub) => {
                let params: Vec<String> = sub.params.iter().map(param_signature).collect();
                syms.subs.push(SymbolInfo {
                    name: sub.name.clone(),
                    line: ls.line,
                    detail: Some(format!("SUB {}({})", sub.name, params.join(", "))),
                });
                collect_symbols(&sub.body, syms, seen_vars);
            }
            Stmt::FunctionDef(func) => {
                let params: Vec<String> = func.params.iter().map(param_signature).collect();
                syms.functions.push(SymbolInfo {
                    name: func.name.clone(),
                    line: ls.line,
                    detail: Some(format!("FUNCTION {}({})", func.name, params.join(", "))),
                });
                collect_symbols(&func.body, syms, seen_vars);
            }
            Stmt::Let { var, .. } => {
                add_variable(syms, var, ls.line, seen_vars);
            }
            Stmt::Dim(decls) | Stmt::Redim { decls, .. } => {
                let tag = if matches!(&ls.stmt, Stmt::Redim { .. }) {
                    "REDIM"
                } else {
                    "DIM"
                };
                for d in decls {
                    let full = d.name.clone();
                    if seen_vars.insert(full.clone()) {
                        syms.variables.push(SymbolInfo {
                            name: full,
                            line: ls.line,
                            detail: Some(tag.into()),
                        });
                    }
                }
            }
            Stmt::Const { name, .. } => {
                syms.constants.push(SymbolInfo {
                    name: name.clone(),
                    line: ls.line,
                    detail: Some("CONST".into()),
                });
            }
            Stmt::For(f) => {
                add_variable(syms, &f.var, ls.line, seen_vars);
                collect_symbols(&f.body, syms, seen_vars);
            }
            Stmt::If(if_stmt) => {
                collect_symbols(&if_stmt.then_body, syms, seen_vars);
                for (_, body) in &if_stmt.elseif_clauses {
                    collect_symbols(body, syms, seen_vars);
                }
                if let Some(ref else_body) = if_stmt.else_body {
                    collect_symbols(else_body, syms, seen_vars);
                }
            }
            Stmt::WhileWend { body, .. } => collect_symbols(body, syms, seen_vars),
            Stmt::DoLoop(d) => collect_symbols(&d.body, syms, seen_vars),
            Stmt::SelectCase(s) => {
                for case in &s.cases {
                    collect_symbols(&case.body, syms, seen_vars);
                }
                if let Some(ref else_body) = s.else_body {
                    collect_symbols(else_body, syms, seen_vars);
                }
            }
            Stmt::Input(input) => {
                for v in &input.vars {
                    add_variable(syms, v, ls.line, seen_vars);
                }
            }
            Stmt::LineInput { var, .. } => add_variable(syms, var, ls.line, seen_vars),
            Stmt::Read(vars) => {
                for v in vars {
                    add_variable(syms, v, ls.line, seen_vars);
                }
            }
            _ => {}
        }
    }
}

fn add_variable(
    syms: &mut DocumentSymbols,
    var: &Variable,
    line: usize,
    seen: &mut HashSet<String>,
) {
    let full = var.name.clone();
    if seen.insert(full.clone()) {
        syms.variables.push(SymbolInfo {
            name: full,
            line,
            detail: None,
        });
    }
}

fn param_signature(p: &Param) -> String {
    if p.is_array {
        format!("{}()", p.name)
    } else {
        p.name.clone()
    }
}

// ---------------------------------------------------------------------------
// Completions (cached via LazyLock)
// ---------------------------------------------------------------------------

static KEYWORD_COMPLETIONS: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    [
        // I/O
        ("PRINT", "Print output to the screen"),
        ("PRINT USING", "Print with format string"),
        ("INPUT", "Read user input"),
        ("LINE INPUT", "Read an entire line of input"),
        ("WRITE", "Write comma-delimited data"),
        // Variables
        ("LET", "Assign a value to a variable"),
        ("DIM", "Declare a variable or array"),
        ("CONST", "Declare a constant"),
        ("SWAP", "Swap two variables"),
        ("OPTION BASE", "Set default array lower bound"),
        ("REDIM", "Redimension an array"),
        ("ERASE", "Erase an array"),
        ("SHARED", "Share variable with main module"),
        ("STATIC", "Preserve local variables between calls"),
        ("CLEAR", "Reset all variables to defaults"),
        // Control flow
        ("IF", "Conditional statement"),
        ("THEN", "Part of IF statement"),
        ("ELSE", "Alternative branch of IF"),
        ("ELSEIF", "Additional conditional branch"),
        ("END IF", "End of block IF"),
        ("FOR", "Begin a FOR loop"),
        ("TO", "Specify FOR loop end value"),
        ("STEP", "Specify FOR loop increment"),
        ("NEXT", "End of FOR loop"),
        ("WHILE", "Begin a WHILE loop"),
        ("WEND", "End of WHILE loop"),
        ("DO", "Begin a DO loop"),
        ("LOOP", "End of DO loop"),
        ("UNTIL", "Loop until condition is true"),
        ("SELECT CASE", "Multi-way branch"),
        ("CASE", "Branch of SELECT CASE"),
        ("END SELECT", "End of SELECT CASE"),
        ("GOTO", "Jump to a label"),
        ("EXIT FOR", "Exit a FOR loop early"),
        ("EXIT DO", "Exit a DO loop early"),
        ("EXIT SUB", "Exit a SUB early"),
        ("EXIT FUNCTION", "Exit a FUNCTION early"),
        ("END", "End program execution"),
        ("STOP", "Stop program execution"),
        ("SYSTEM", "Exit to operating system"),
        // Procedures
        ("SUB", "Define a subroutine"),
        ("END SUB", "End of SUB definition"),
        ("FUNCTION", "Define a function"),
        ("END FUNCTION", "End of FUNCTION definition"),
        ("CALL", "Call a SUB"),
        ("DECLARE", "Forward-declare a SUB or FUNCTION"),
        ("BYVAL", "Pass argument by value"),
        // Data
        ("DATA", "Define inline data"),
        ("READ", "Read from DATA"),
        ("RESTORE", "Reset DATA pointer"),
        // User-defined types
        ("TYPE", "Define a user-defined record type"),
        ("END TYPE", "End of TYPE definition"),
        // Error handling
        (
            "WHEN EXCEPTION IN",
            "Begin guarded block for error handling",
        ),
        ("USE", "Begin error handler block"),
        ("END WHEN", "End of WHEN EXCEPTION block"),
        ("RETRY", "Re-execute the guarded block after error"),
        ("CONTINUE", "Resume after the failed statement"),
        // File I/O
        ("OPEN", "Open a file (ANSI syntax)"),
        ("CLOSE", "Close a file"),
        ("SET POINTER", "Set file position"),
        ("ASK POINTER", "Query file position"),
        ("GET", "Read binary record from file"),
        ("PUT", "Write binary record to file"),
        // File system
        ("NAME", "Rename a file (NAME old$ AS new$)"),
        ("KILL", "Delete a file"),
        ("MKDIR", "Create a directory"),
        ("RMDIR", "Remove a directory"),
        ("CHDIR", "Change current directory"),
        // Console
        ("CLS", "Clear the screen"),
        ("LOCATE", "Move cursor to row, column"),
        ("COLOR", "Set foreground and background colors"),
        ("BEEP", "Sound the terminal bell"),
        ("WIDTH", "Set terminal width"),
        ("VIEW PRINT", "Set scrolling region"),
        // Matrix
        (
            "MAT",
            "Matrix operation (MAT PRINT, MAT READ, MAT +/-/*, etc.)",
        ),
        // System
        ("SHELL", "Execute a system command"),
        ("SLEEP", "Pause execution"),
        ("RANDOMIZE", "Seed the random number generator"),
        // Operators
        ("AND", "Logical AND"),
        ("OR", "Logical OR"),
        ("NOT", "Logical NOT"),
        ("XOR", "Logical exclusive OR"),
        ("MOD", "Modulo operator"),
        // Other
        ("REM", "Comment"),
        ("AS", "Type specifier"),
    ]
    .iter()
    .map(|(kw, doc)| CompletionItem {
        label: kw.to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        detail: Some(doc.to_string()),
        ..Default::default()
    })
    .collect()
});

static BUILTIN_COMPLETIONS: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    [
        // Math
        ("ABS", "ABS(n) — Absolute value"),
        ("INT", "INT(n) — Floor to integer"),
        ("FIX", "FIX(n) — Truncate to integer"),
        ("SGN", "SGN(n) — Sign (-1, 0, 1)"),
        ("SQR", "SQR(n) — Square root"),
        ("SIN", "SIN(n) — Sine (radians)"),
        ("COS", "COS(n) — Cosine (radians)"),
        ("TAN", "TAN(n) — Tangent (radians)"),
        ("ATN", "ATN(n) — Arctangent (radians)"),
        ("EXP", "EXP(n) — e raised to power n"),
        ("LOG", "LOG(n) — Natural logarithm"),
        ("RND", "RND — Random number [0,1)"),
        (
            "ROUND",
            "ROUND(n[, places]) — Round to nearest integer or decimal place",
        ),
        ("ASIN", "ASIN(n) — Arc sine (radians)"),
        ("ACOS", "ACOS(n) — Arc cosine (radians)"),
        ("COT", "COT(n) — Cotangent (radians)"),
        ("CSC", "CSC(n) — Cosecant (radians)"),
        ("SEC", "SEC(n) — Secant (radians)"),
        ("ANGLE", "ANGLE(x, y) — Two-argument arctangent (radians)"),
        ("CEIL", "CEIL(n) — Ceiling (smallest integer >= n)"),
        (
            "TRUNCATE",
            "TRUNCATE(n, places) — Truncate to decimal places",
        ),
        ("REMAINDER", "REMAINDER(a, b) — Remainder of a / b"),
        ("MAXNUM", "MAXNUM — Largest representable number"),
        ("PI", "PI — Value of pi (3.14159...)"),
        // String
        ("LEN", "LEN(s$) — String length"),
        ("LEFT$", "LEFT$(s$, n) — Left n characters"),
        ("RIGHT$", "RIGHT$(s$, n) — Right n characters"),
        ("MID$", "MID$(s$, start[, len]) — Substring"),
        ("INSTR", "INSTR([start,] s$, search$) — Find substring"),
        ("UCASE$", "UCASE$(s$) — Convert to uppercase"),
        ("LCASE$", "LCASE$(s$) — Convert to lowercase"),
        ("LTRIM$", "LTRIM$(s$) — Remove leading spaces"),
        ("RTRIM$", "RTRIM$(s$) — Remove trailing spaces"),
        ("SPACE$", "SPACE$(n) — String of n spaces"),
        ("STRING$", "STRING$(n, char) — Repeat character n times"),
        ("CHR$", "CHR$(n) — Character from ASCII code"),
        ("ASC", "ASC(s$) — ASCII code of first character"),
        ("STR$", "STR$(n) — Convert number to string"),
        ("VAL", "VAL(s$) — Convert string to number"),
        ("HEX$", "HEX$(n) — Hexadecimal representation"),
        ("OCT$", "OCT$(n) — Octal representation"),
        // File
        ("FREEFILE", "FREEFILE — Next available file number"),
        ("EOF", "EOF(n) — End-of-file check"),
        ("LOF", "LOF(n) — Length of file"),
        ("LOC", "LOC(n) — Current position in file"),
        // System
        ("ENVIRON$", "ENVIRON$(name$) — Get environment variable"),
        ("TIMER", "TIMER — Seconds since midnight"),
        ("DATE$", "DATE$ — Current date"),
        ("TIME$", "TIME$ — Current time"),
        // Array (stubs)
        ("LBOUND", "LBOUND(array[, dim]) — Lower bound of array"),
        ("UBOUND", "UBOUND(array[, dim]) — Upper bound of array"),
    ]
    .iter()
    .map(|(name, doc)| CompletionItem {
        label: name.to_string(),
        kind: Some(CompletionItemKind::FUNCTION),
        detail: Some(doc.to_string()),
        ..Default::default()
    })
    .collect()
});

static TYPE_COMPLETIONS: LazyLock<Vec<CompletionItem>> = LazyLock::new(|| {
    ["NUMERIC", "STRING"]
        .iter()
        .map(|t| CompletionItem {
            label: t.to_string(),
            kind: Some(CompletionItemKind::TYPE_PARAMETER),
            detail: Some("Data type".into()),
            ..Default::default()
        })
        .collect()
});

// ---------------------------------------------------------------------------
// Hover docs
// ---------------------------------------------------------------------------

static BUILTIN_HOVER_DOCS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        // Math
        (
            "ABS",
            "```basic\nABS(n)\n```\nReturns the absolute value of `n`.",
        ),
        (
            "INT",
            "```basic\nINT(n)\n```\nReturns the largest integer ≤ `n` (floor).",
        ),
        ("FIX", "```basic\nFIX(n)\n```\nTruncates `n` toward zero."),
        (
            "SGN",
            "```basic\nSGN(n)\n```\nReturns -1, 0, or 1 based on the sign of `n`.",
        ),
        (
            "SQR",
            "```basic\nSQR(n)\n```\nReturns the square root of `n`.",
        ),
        (
            "SIN",
            "```basic\nSIN(n)\n```\nReturns the sine of `n` (radians).",
        ),
        (
            "COS",
            "```basic\nCOS(n)\n```\nReturns the cosine of `n` (radians).",
        ),
        (
            "TAN",
            "```basic\nTAN(n)\n```\nReturns the tangent of `n` (radians).",
        ),
        (
            "ATN",
            "```basic\nATN(n)\n```\nReturns the arctangent of `n` (radians).",
        ),
        (
            "EXP",
            "```basic\nEXP(n)\n```\nReturns e raised to the power `n`.",
        ),
        (
            "LOG",
            "```basic\nLOG(n)\n```\nReturns the natural logarithm of `n`.",
        ),
        (
            "RND",
            "```basic\nRND[(n)]\n```\nReturns a random number in [0, 1).",
        ),
        (
            "ROUND",
            "```basic\nROUND(n[, places])\n```\nRounds `n` to the nearest integer, or to `places` decimal places.",
        ),
        (
            "ASIN",
            "```basic\nASIN(n)\n```\nReturns the arc sine of `n` in radians.",
        ),
        (
            "ACOS",
            "```basic\nACOS(n)\n```\nReturns the arc cosine of `n` in radians.",
        ),
        (
            "COT",
            "```basic\nCOT(n)\n```\nReturns the cotangent of `n` (radians).",
        ),
        (
            "CSC",
            "```basic\nCSC(n)\n```\nReturns the cosecant of `n` (radians).",
        ),
        (
            "SEC",
            "```basic\nSEC(n)\n```\nReturns the secant of `n` (radians).",
        ),
        (
            "ANGLE",
            "```basic\nANGLE(x, y)\n```\nReturns the angle in radians from the positive x-axis to the point (x, y).",
        ),
        (
            "CEIL",
            "```basic\nCEIL(n)\n```\nReturns the smallest integer ≥ `n` (ceiling).",
        ),
        (
            "TRUNCATE",
            "```basic\nTRUNCATE(n, places)\n```\nTruncates `n` to `places` decimal places.",
        ),
        (
            "REMAINDER",
            "```basic\nREMAINDER(a, b)\n```\nReturns the IEEE remainder of `a` divided by `b`.",
        ),
        (
            "MAXNUM",
            "```basic\nMAXNUM\n```\nReturns the largest representable numeric value.",
        ),
        (
            "PI",
            "```basic\nPI\n```\nReturns the value of pi (3.14159265358979...).",
        ),
        // String
        (
            "LEN",
            "```basic\nLEN(s$)\n```\nReturns the length of string `s$`.",
        ),
        (
            "LEFT$",
            "```basic\nLEFT$(s$, n)\n```\nReturns the leftmost `n` characters of `s$`.",
        ),
        (
            "RIGHT$",
            "```basic\nRIGHT$(s$, n)\n```\nReturns the rightmost `n` characters of `s$`.",
        ),
        (
            "MID$",
            "```basic\nMID$(s$, start[, length])\n```\nReturns a substring starting at position `start`. If `length` is omitted, returns from `start` to end.",
        ),
        (
            "INSTR",
            "```basic\nINSTR([start,] s$, search$)\n```\nReturns the position of `search$` in `s$`, or 0 if not found.",
        ),
        (
            "UCASE$",
            "```basic\nUCASE$(s$)\n```\nConverts `s$` to uppercase.",
        ),
        (
            "LCASE$",
            "```basic\nLCASE$(s$)\n```\nConverts `s$` to lowercase.",
        ),
        (
            "LTRIM$",
            "```basic\nLTRIM$(s$)\n```\nRemoves leading spaces from `s$`.",
        ),
        (
            "RTRIM$",
            "```basic\nRTRIM$(s$)\n```\nRemoves trailing spaces from `s$`.",
        ),
        (
            "SPACE$",
            "```basic\nSPACE$(n)\n```\nReturns a string of `n` spaces.",
        ),
        (
            "STRING$",
            "```basic\nSTRING$(n, char)\n```\nReturns a string of `n` copies of `char`.",
        ),
        (
            "CHR$",
            "```basic\nCHR$(n)\n```\nReturns the character with ASCII code `n`.",
        ),
        (
            "ASC",
            "```basic\nASC(s$)\n```\nReturns the ASCII code of the first character of `s$`.",
        ),
        (
            "STR$",
            "```basic\nSTR$(n)\n```\nConverts number `n` to its string representation.",
        ),
        (
            "VAL",
            "```basic\nVAL(s$)\n```\nConverts string `s$` to a number.",
        ),
        (
            "HEX$",
            "```basic\nHEX$(n)\n```\nReturns the hexadecimal string representation of `n`.",
        ),
        (
            "OCT$",
            "```basic\nOCT$(n)\n```\nReturns the octal string representation of `n`.",
        ),
        // File
        (
            "FREEFILE",
            "```basic\nFREEFILE\n```\nReturns the next available file number.",
        ),
        (
            "EOF",
            "```basic\nEOF(n)\n```\nReturns 1 (true) if at end of file `n`, 0 otherwise.",
        ),
        (
            "LOF",
            "```basic\nLOF(n)\n```\nReturns the length in bytes of file `n`.",
        ),
        (
            "LOC",
            "```basic\nLOC(n)\n```\nReturns the current byte position in file `n`.",
        ),
        // System
        (
            "ENVIRON$",
            "```basic\nENVIRON$(name$)\n```\nReturns the value of the environment variable `name$`.",
        ),
        (
            "TIMER",
            "```basic\nTIMER\n```\nReturns the number of seconds elapsed since midnight.",
        ),
        (
            "DATE$",
            "```basic\nDATE$\n```\nReturns the current date as MM-DD-YYYY.",
        ),
        (
            "TIME$",
            "```basic\nTIME$\n```\nReturns the current time as HH:MM:SS.",
        ),
        // Array (stubs)
        (
            "LBOUND",
            "```basic\nLBOUND(array[, dim])\n```\nReturns the lower bound of `array`. *(Stub — not fully implemented.)*",
        ),
        (
            "UBOUND",
            "```basic\nUBOUND(array[, dim])\n```\nReturns the upper bound of `array`. *(Stub — not fully implemented.)*",
        ),
    ])
});

static KEYWORD_HOVER_DOCS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        // I/O
        (
            "PRINT",
            "```basic\nPRINT [expression][;|,] ...\n```\nDisplays output on the screen. Use `;` to suppress spacing, `,` for tab zones. Use `TAB(n)` and `SPC(n)` for positioning.",
        ),
        (
            "PRINT USING",
            "```basic\nPRINT USING format$: expr[, expr...]\n```\nFormatted output. `#` for digits, `.` for decimal, `$$` for currency, `^^^^` for scientific notation.",
        ),
        (
            "INPUT",
            "```basic\nINPUT [\"prompt\";] var[, var...]\n```\nReads values from the keyboard into variables.",
        ),
        (
            "LINE INPUT",
            "```basic\nLINE INPUT [\"prompt\";] var$\n```\nReads an entire line of input into a string variable (no comma parsing).",
        ),
        (
            "WRITE",
            "```basic\nWRITE [expression][, expression...]\n```\nWrites comma-delimited data with strings in quotes.",
        ),
        // Variables
        (
            "LET",
            "```basic\nLET var = expression\n```\nAssigns a value to a variable. The `LET` keyword is optional.",
        ),
        (
            "DIM",
            "```basic\nDIM var[(dims)] [AS type]\n```\nDeclares a variable or array with optional type and dimensions.",
        ),
        (
            "CONST",
            "```basic\nCONST name = expression\n```\nDefines a named constant that cannot be reassigned.",
        ),
        (
            "SWAP",
            "```basic\nSWAP var1, var2\n```\nExchanges the values of two variables.",
        ),
        (
            "OPTION BASE",
            "```basic\nOPTION BASE {0|1}\n```\nSets the default lower bound for arrays. Default is 1 (ANSI convention).",
        ),
        (
            "REDIM",
            "```basic\nREDIM [PRESERVE] var(dims) [AS type]\n```\nRedimensions an array. Use `PRESERVE` to keep existing contents.",
        ),
        (
            "ERASE",
            "```basic\nERASE array[, array...]\n```\nResets arrays to their default values.",
        ),
        (
            "SHARED",
            "```basic\nSHARED var[, var...]\n```\nMakes variables in a SUB/FUNCTION refer to the main module's variables.",
        ),
        (
            "STATIC",
            "```basic\nSTATIC var[, var...]\n```\nPreserves local variable values between calls to a SUB/FUNCTION.",
        ),
        (
            "CLEAR",
            "```basic\nCLEAR\n```\nResets all variables to their default values (0 or \"\").",
        ),
        // Control flow
        (
            "IF",
            "```basic\nIF condition THEN\n  ...\n[ELSEIF condition THEN\n  ...]\n[ELSE\n  ...]\nEND IF\n```\nConditional execution. Also supports single-line form: `IF cond THEN stmt [ELSE stmt]`.",
        ),
        (
            "FOR",
            "```basic\nFOR var = start TO end [STEP inc]\n  ...\nNEXT [var]\n```\nCounted loop. Default STEP is 1.",
        ),
        (
            "WHILE",
            "```basic\nWHILE condition\n  ...\nWEND\n```\nLoop while condition is true.",
        ),
        (
            "DO",
            "```basic\nDO [{WHILE|UNTIL} condition]\n  ...\nLOOP [{WHILE|UNTIL} condition]\n```\nFlexible loop with condition at top or bottom.",
        ),
        (
            "SELECT",
            "```basic\nSELECT CASE expression\n  CASE value[, value...]\n    ...\n  CASE ELSE\n    ...\nEND SELECT\n```\nMulti-way branch based on expression value.",
        ),
        (
            "GOTO",
            "```basic\nGOTO label\n```\nTransfers execution to the specified label or line number.",
        ),
        ("END", "```basic\nEND\n```\nEnds program execution."),
        ("STOP", "```basic\nSTOP\n```\nStops program execution."),
        (
            "SYSTEM",
            "```basic\nSYSTEM\n```\nExits to the operating system.",
        ),
        // Procedures
        (
            "SUB",
            "```basic\nSUB name (params)\n  ...\nEND SUB\n```\nDefines a subroutine. Called with `CALL name(args)` or just `name args`.",
        ),
        (
            "FUNCTION",
            "```basic\nFUNCTION name[$] (params)\n  name = return_value\nEND FUNCTION\n```\nDefines a function that returns a value.",
        ),
        (
            "CALL",
            "```basic\nCALL name(args)\n```\nCalls a SUB with the given arguments.",
        ),
        (
            "DECLARE",
            "```basic\nDECLARE SUB name (params)\nDECLARE FUNCTION name (params)\n```\nForward-declares a SUB or FUNCTION.",
        ),
        // Data
        (
            "DATA",
            "```basic\nDATA value[, value...]\n```\nDefines inline data to be read with READ.",
        ),
        (
            "READ",
            "```basic\nREAD var[, var...]\n```\nReads values from DATA statements into variables.",
        ),
        (
            "RESTORE",
            "```basic\nRESTORE [label]\n```\nResets the DATA pointer to the beginning or to a specific label.",
        ),
        // User-defined types
        (
            "TYPE",
            "```basic\nTYPE name\n  field AS type\n  ...\nEND TYPE\n```\nDefines a user-defined record type with named fields. Access fields with dot notation.",
        ),
        // Error handling
        (
            "WHEN EXCEPTION IN",
            "```basic\nWHEN EXCEPTION IN\n  ...\nUSE\n  PRINT EXTYPE; EXTEXT$\nEND WHEN\n```\nStructured exception handling. Code in the guarded block is protected; the USE block handles errors.",
        ),
        (
            "RETRY",
            "```basic\nRETRY\n```\nRe-executes the guarded block from the beginning after an error in a WHEN EXCEPTION handler.",
        ),
        (
            "CONTINUE",
            "```basic\nCONTINUE\n```\nResumes execution after the statement that caused the error in a WHEN EXCEPTION handler.",
        ),
        // File I/O
        (
            "OPEN",
            "```basic\nOPEN #n: NAME file$, ACCESS {INPUT|OUTPUT|OUTIN}, ORGANIZATION {SEQUENTIAL|STREAM}\n```\nOpens a file for I/O using ANSI Full BASIC syntax.",
        ),
        ("CLOSE", "```basic\nCLOSE #n\n```\nCloses an open file."),
        (
            "SET POINTER",
            "```basic\nSET #n: POINTER position\n```\nSets the file position for the next read or write.",
        ),
        (
            "ASK POINTER",
            "```basic\nASK #n: POINTER var\n```\nQueries the current file position into a variable.",
        ),
        // File system
        ("NAME", "```basic\nNAME old$ AS new$\n```\nRenames a file."),
        ("KILL", "```basic\nKILL file$\n```\nDeletes a file."),
        ("MKDIR", "```basic\nMKDIR dir$\n```\nCreates a directory."),
        ("RMDIR", "```basic\nRMDIR dir$\n```\nRemoves a directory."),
        (
            "CHDIR",
            "```basic\nCHDIR dir$\n```\nChanges the current working directory.",
        ),
        // Console
        ("CLS", "```basic\nCLS\n```\nClears the screen."),
        (
            "LOCATE",
            "```basic\nLOCATE row[, col]\n```\nMoves the cursor to the specified row and column (1-based).",
        ),
        (
            "COLOR",
            "```basic\nCOLOR foreground[, background]\n```\nSets the text foreground and background colors (0-255).",
        ),
        ("BEEP", "```basic\nBEEP\n```\nSounds the terminal bell."),
        (
            "WIDTH",
            "```basic\nWIDTH columns\n```\nSets the terminal width for PRINT output.",
        ),
        (
            "VIEW PRINT",
            "```basic\nVIEW PRINT [top TO bottom]\n```\nSets the scrolling region. Without arguments, resets to full screen.",
        ),
        // Matrix
        (
            "MAT",
            "```basic\nMAT C = A + B\nMAT C = A * B\nMAT B = INV(A)\nMAT B = TRN(A)\nMAT A = ZER / CON / IDN\nMAT PRINT A\n```\nMatrix operations on numeric arrays.",
        ),
        // System
        (
            "SHELL",
            "```basic\nSHELL command$\n```\nExecutes a system command.",
        ),
        (
            "SLEEP",
            "```basic\nSLEEP [seconds]\n```\nPauses execution. Without an argument, pauses indefinitely.",
        ),
        (
            "RANDOMIZE",
            "```basic\nRANDOMIZE [seed | TIMER]\n```\nSeeds the random number generator. Use a fixed seed for reproducible sequences.",
        ),
    ])
});

fn builtin_hover(name: &str) -> Option<&'static str> {
    // Try exact match, then with $ appended (handles bare names like LEFT -> LEFT$)
    BUILTIN_HOVER_DOCS
        .get(name)
        .or_else(|| {
            let with_dollar = format!("{}$", name);
            BUILTIN_HOVER_DOCS.get(with_dollar.as_str())
        })
        .copied()
}

fn keyword_hover(name: &str) -> Option<&'static str> {
    KEYWORD_HOVER_DOCS.get(name).copied()
}

// ---------------------------------------------------------------------------
// Token-at-cursor helper
// ---------------------------------------------------------------------------

fn resolve_token_name(state: &DocumentState, pos: Position) -> Option<String> {
    let line_1 = pos.line as usize + 1;
    let col_1 = pos.character as usize + 1;

    // Find the last token on this line whose column <= cursor column
    let mut best: Option<&SpannedToken> = None;
    for t in &state.tokens {
        if t.span.line == line_1 && t.span.col <= col_1 {
            best = Some(t);
        }
        if t.span.line > line_1 {
            break;
        }
    }

    token_name(&best?.token)
}

fn token_name(tok: &rice::token::Token) -> Option<String> {
    match tok {
        rice::token::Token::Identifier(name) => Some(name.clone()),
        rice::token::Token::NumericLiteral(n) => Some(n.to_string()),
        rice::token::Token::LineNumber(n) => Some(n.to_string()),
        rice::token::Token::KwPrint => Some("PRINT".into()),
        rice::token::Token::KwInput => Some("INPUT".into()),
        rice::token::Token::KwLineInput => Some("LINE INPUT".into()),
        rice::token::Token::KwDim => Some("DIM".into()),
        rice::token::Token::KwConst => Some("CONST".into()),
        rice::token::Token::KwIf => Some("IF".into()),
        rice::token::Token::KwThen => Some("THEN".into()),
        rice::token::Token::KwElse => Some("ELSE".into()),
        rice::token::Token::KwElseIf => Some("ELSEIF".into()),
        rice::token::Token::KwEndIf => Some("END IF".into()),
        rice::token::Token::KwFor => Some("FOR".into()),
        rice::token::Token::KwTo => Some("TO".into()),
        rice::token::Token::KwStep => Some("STEP".into()),
        rice::token::Token::KwNext => Some("NEXT".into()),
        rice::token::Token::KwWhile => Some("WHILE".into()),
        rice::token::Token::KwWend => Some("WEND".into()),
        rice::token::Token::KwDo => Some("DO".into()),
        rice::token::Token::KwLoop => Some("LOOP".into()),
        rice::token::Token::KwUntil => Some("UNTIL".into()),
        rice::token::Token::KwGoto => Some("GOTO".into()),
        rice::token::Token::KwGosub => Some("GOSUB".into()),
        rice::token::Token::KwReturn => Some("RETURN".into()),
        rice::token::Token::KwSelect => Some("SELECT".into()),
        rice::token::Token::KwCase => Some("CASE".into()),
        rice::token::Token::KwEnd => Some("END".into()),
        rice::token::Token::KwSub => Some("SUB".into()),
        rice::token::Token::KwFunction => Some("FUNCTION".into()),
        rice::token::Token::KwCall => Some("CALL".into()),
        rice::token::Token::KwDeclare => Some("DECLARE".into()),
        rice::token::Token::KwData => Some("DATA".into()),
        rice::token::Token::KwRead => Some("READ".into()),
        rice::token::Token::KwRestore => Some("RESTORE".into()),
        rice::token::Token::KwSwap => Some("SWAP".into()),
        rice::token::Token::KwOpen => Some("OPEN".into()),
        rice::token::Token::KwClose => Some("CLOSE".into()),
        rice::token::Token::KwOn => Some("ON".into()),
        rice::token::Token::KwError => Some("ERROR".into()),
        rice::token::Token::KwResume => Some("RESUME".into()),
        rice::token::Token::KwRem => Some("REM".into()),
        rice::token::Token::KwLet => Some("LET".into()),
        rice::token::Token::KwExit => Some("EXIT".into()),
        rice::token::Token::KwFreefile => Some("FREEFILE".into()),
        rice::token::Token::KwGet => Some("GET".into()),
        rice::token::Token::KwPut => Some("PUT".into()),
        rice::token::Token::KwWrite => Some("WRITE".into()),
        rice::token::Token::KwUsing => Some("USING".into()),
        rice::token::Token::KwRedim => Some("REDIM".into()),
        rice::token::Token::KwErase => Some("ERASE".into()),
        rice::token::Token::KwOption => Some("OPTION".into()),
        rice::token::Token::KwRandomize => Some("RANDOMIZE".into()),
        rice::token::Token::KwSystem => Some("SYSTEM".into()),
        rice::token::Token::KwStop => Some("STOP".into()),
        rice::token::Token::KwAnd => Some("AND".into()),
        rice::token::Token::KwOr => Some("OR".into()),
        rice::token::Token::KwNot => Some("NOT".into()),
        rice::token::Token::KwXor => Some("XOR".into()),
        rice::token::Token::KwMod => Some("MOD".into()),
        rice::token::Token::KwEndSub => Some("END SUB".into()),
        rice::token::Token::KwEndFunction => Some("END FUNCTION".into()),
        rice::token::Token::KwEndSelect => Some("END SELECT".into()),
        rice::token::Token::KwEndType => Some("END TYPE".into()),
        rice::token::Token::KwEndWhile => Some("END WHILE".into()),
        rice::token::Token::KwEndWhen => Some("END WHEN".into()),
        rice::token::Token::KwShared => Some("SHARED".into()),
        rice::token::Token::KwStatic => Some("STATIC".into()),
        rice::token::Token::KwByVal => Some("BYVAL".into()),
        rice::token::Token::KwSleep => Some("SLEEP".into()),
        rice::token::Token::KwClear => Some("CLEAR".into()),
        rice::token::Token::KwName => Some("NAME".into()),
        rice::token::Token::KwKill => Some("KILL".into()),
        rice::token::Token::KwMkdir => Some("MKDIR".into()),
        rice::token::Token::KwRmdir => Some("RMDIR".into()),
        rice::token::Token::KwChdir => Some("CHDIR".into()),
        rice::token::Token::KwShell => Some("SHELL".into()),
        rice::token::Token::KwCls => Some("CLS".into()),
        rice::token::Token::KwBeep => Some("BEEP".into()),
        rice::token::Token::KwLocate => Some("LOCATE".into()),
        rice::token::Token::KwColor => Some("COLOR".into()),
        rice::token::Token::KwWidth => Some("WIDTH".into()),
        rice::token::Token::KwView => Some("VIEW PRINT".into()),
        rice::token::Token::KwWhen => Some("WHEN EXCEPTION IN".into()),
        rice::token::Token::KwRetry => Some("RETRY".into()),
        rice::token::Token::KwContinue => Some("CONTINUE".into()),
        rice::token::Token::KwMat => Some("MAT".into()),
        rice::token::Token::KwType => Some("TYPE".into()),
        rice::token::Token::KwSet => Some("SET POINTER".into()),
        rice::token::Token::KwAsk => Some("ASK POINTER".into()),
        _ => None,
    }
}

/// Push completions from a symbol list.
fn push_symbol_completions(
    items: &mut Vec<CompletionItem>,
    symbols: &[SymbolInfo],
    kind: CompletionItemKind,
) {
    for sym in symbols {
        items.push(CompletionItem {
            label: sym.name.clone(),
            kind: Some(kind),
            detail: sym.detail.clone(),
            ..Default::default()
        });
    }
}

/// Find the first matching symbol by name across all categories.
fn find_symbol<'a>(symbols: &'a DocumentSymbols, name: &str) -> Option<&'a SymbolInfo> {
    symbols
        .subs
        .iter()
        .chain(symbols.functions.iter())
        .chain(symbols.variables.iter())
        .chain(symbols.constants.iter())
        .chain(symbols.labels.iter())
        .find(|s| s.name == name)
}

// ---------------------------------------------------------------------------
// LSP trait implementation
// ---------------------------------------------------------------------------

#[tower_lsp::async_trait]
impl LanguageServer for RiceLspBackend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec![".".into(), "$".into()]),
                    ..Default::default()
                }),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "RICE BASIC LSP server initialized")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.analyze(params.text_document.uri, params.text_document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            self.analyze(params.text_document.uri, change.text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .write()
            .await
            .remove(&params.text_document.uri);
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let mut items = Vec::new();

        items.extend_from_slice(&KEYWORD_COMPLETIONS);
        items.extend_from_slice(&BUILTIN_COMPLETIONS);
        items.extend_from_slice(&TYPE_COMPLETIONS);

        let uri = params.text_document_position.text_document.uri;
        let docs = self.documents.read().await;
        if let Some(state) = docs.get(&uri) {
            push_symbol_completions(
                &mut items,
                &state.symbols.subs,
                CompletionItemKind::FUNCTION,
            );
            push_symbol_completions(
                &mut items,
                &state.symbols.functions,
                CompletionItemKind::FUNCTION,
            );
            push_symbol_completions(
                &mut items,
                &state.symbols.variables,
                CompletionItemKind::VARIABLE,
            );
            push_symbol_completions(
                &mut items,
                &state.symbols.constants,
                CompletionItemKind::CONSTANT,
            );
            push_symbol_completions(
                &mut items,
                &state.symbols.labels,
                CompletionItemKind::REFERENCE,
            );
        }

        Ok(Some(CompletionResponse::Array(items)))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let state = match docs.get(&uri) {
            Some(s) => s,
            None => return Ok(None),
        };

        let name = match resolve_token_name(state, pos) {
            Some(n) => n,
            None => return Ok(None),
        };
        let upper = name.to_uppercase();

        // Try builtin docs
        if let Some(doc) = builtin_hover(&upper) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.to_string(),
                }),
                range: None,
            }));
        }

        // Try keyword docs
        if let Some(doc) = keyword_hover(&upper) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: doc.to_string(),
                }),
                range: None,
            }));
        }

        // Try user-defined symbols (including labels)
        if let Some(sym) = find_symbol(&state.symbols, &upper) {
            let detail = sym.detail.as_deref().unwrap_or(&sym.name);
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: format!("```basic\n{}\n```", detail),
                }),
                range: None,
            }));
        }

        Ok(None)
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let docs = self.documents.read().await;
        let state = match docs.get(&uri) {
            Some(s) => s,
            None => return Ok(None),
        };

        let name = match resolve_token_name(state, pos) {
            Some(n) => n,
            None => return Ok(None),
        };
        let upper = name.to_uppercase();

        if let Some(sym) = find_symbol(&state.symbols, &upper) {
            let def_line = sym.line.saturating_sub(1) as u32;
            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                uri: uri.clone(),
                range: Range {
                    start: Position::new(def_line, 0),
                    end: Position::new(def_line, u32::MAX),
                },
            })));
        }

        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(RiceLspBackend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
