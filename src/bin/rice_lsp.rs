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
    source: String,
    version: i32,
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

    async fn analyze(&self, uri: Url, source: String, version: i32) {
        let state = analyze_source(source, version);
        let diagnostics = state.diagnostics.clone();
        {
            let mut documents = self.documents.write().await;
            if documents
                .get(&uri)
                .is_some_and(|current| current.version > version)
            {
                return;
            }
            documents.insert(uri.clone(), state);
        }
        self.client
            .publish_diagnostics(uri, diagnostics, Some(version))
            .await;
    }
}

fn analyze_source(source: String, version: i32) -> DocumentState {
    let mut diagnostics = Vec::new();
    let dialect = rice::detect_dialect(&source).unwrap_or(rice::DEFAULT_DIALECT);
    let mut lexer = Lexer::with_dialect(&source, dialect);

    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(e) => {
            let (line, col) = lex_error_pos(&e);
            diagnostics.push(Diagnostic {
                range: Range {
                    start: source_position(&source, line, col),
                    end: source_position(&source, line, col.saturating_add(1)),
                },
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("rice".into()),
                message: e.to_string(),
                ..Default::default()
            });
            return DocumentState {
                source,
                version,
                tokens: vec![],
                diagnostics,
                symbols: DocumentSymbols::default(),
            };
        }
    };

    let mut parser = Parser::with_dialect(tokens.clone(), dialect);
    let program = match parser.parse_program() {
        Ok(p) => Some(p),
        Err(e) => {
            let line = parse_error_line(&e);
            diagnostics.push(Diagnostic {
                range: source_line_range(&source, line),
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
        source,
        version,
        tokens,
        diagnostics,
        symbols,
    }
}

/// Convert the lexer's one-based Unicode character columns to LSP UTF-16.
fn source_position(source: &str, line: usize, col: usize) -> Position {
    let text = source
        .split('\n')
        .nth(line.saturating_sub(1))
        .unwrap_or("")
        .trim_end_matches('\r');
    let character = text
        .chars()
        .take(col.saturating_sub(1))
        .map(char::len_utf16)
        .sum::<usize>();
    Position::new(line.saturating_sub(1) as u32, character as u32)
}

fn source_line_range(source: &str, line: usize) -> Range {
    Range {
        start: source_position(source, line, 1),
        end: source_position(source, line, usize::MAX),
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
                let result_type = func
                    .as_type
                    .as_ref()
                    .map(|ty| format!(" AS {}", type_signature(ty)))
                    .unwrap_or_default();
                syms.functions.push(SymbolInfo {
                    name: func.name.clone(),
                    line: ls.line,
                    detail: Some(format!(
                        "FUNCTION {}({}){}",
                        func.name,
                        params.join(", "),
                        result_type
                    )),
                });
                collect_symbols(&func.body, syms, seen_vars);
            }
            Stmt::Let { var, .. } => {
                add_variable(syms, &var.name, ls.line, seen_vars);
            }
            Stmt::MidAssign { target, .. } => {
                add_variable(syms, &target.name, ls.line, seen_vars);
            }
            Stmt::Swap { a, b } => {
                add_variable(syms, &a.name, ls.line, seen_vars);
                add_variable(syms, &b.name, ls.line, seen_vars);
            }
            Stmt::Dim { decls, .. } | Stmt::Redim { decls, .. } => {
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
            Stmt::Const { decls } => {
                for decl in decls {
                    syms.constants.push(SymbolInfo {
                        name: decl.name.clone(),
                        line: ls.line,
                        detail: Some("CONST".into()),
                    });
                }
            }
            Stmt::For(f) => {
                add_variable(syms, &f.var.name, ls.line, seen_vars);
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
            Stmt::WhenException { body, handler } => {
                collect_symbols(body, syms, seen_vars);
                collect_symbols(handler, syms, seen_vars);
            }
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
                    add_variable(syms, &v.name, ls.line, seen_vars);
                }
            }
            Stmt::InputFile(input) => {
                for v in &input.vars {
                    add_variable(syms, &v.name, ls.line, seen_vars);
                }
            }
            Stmt::LineInput { var, .. } | Stmt::LineInputFile { var, .. } => {
                add_variable(syms, &var.name, ls.line, seen_vars);
            }
            Stmt::Read(vars) => {
                for v in vars {
                    add_variable(syms, &v.name, ls.line, seen_vars);
                }
            }
            _ => {}
        }
    }
}

fn add_variable(syms: &mut DocumentSymbols, name: &str, line: usize, seen: &mut HashSet<String>) {
    let full = name.to_string();
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

fn type_signature(ty: &BasicType) -> String {
    match ty {
        BasicType::Numeric => "NUMERIC".into(),
        BasicType::Integer => "INTEGER".into(),
        BasicType::Long => "LONG".into(),
        BasicType::Single => "SINGLE".into(),
        BasicType::Double => "DOUBLE".into(),
        BasicType::String => "STRING".into(),
        BasicType::FixedLengthString(length) => format!("STRING * {length}"),
        BasicType::UserDefined(name) => name.clone(),
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
        (
            "INPUT",
            "Read keyboard or file input into scalars or array elements",
        ),
        (
            "LINE INPUT",
            "Read a complete line into a scalar or array element",
        ),
        ("WRITE", "Write comma-delimited data"),
        // Variables
        ("LET", "Assign a value to a variable"),
        ("DIM", "Declare a variable or array"),
        (
            "CONST",
            "Declare constants left to right: CONST a = 1, b = a + 1",
        ),
        ("SWAP", "Swap scalar variables or indexed array elements"),
        ("OPTION BASE", "Set default array lower bound"),
        ("OPTION EXPLICIT", "Require variable declarations"),
        ("OPTION DIALECT", "Select ANSI or QBasic compatibility mode"),
        ("REDIM", "Redimension an array"),
        ("ERASE", "Erase an array"),
        ("SHARED", "Share variable with main module"),
        ("STATIC", "Preserve local variables between calls"),
        (
            "CLEAR",
            "Clear current variable values and reset DATA cursor",
        ),
        // Control flow
        (
            "IF",
            "Conditional statement; numeric branch targets in QBasic",
        ),
        ("THEN", "Part of IF statement"),
        ("ELSE", "Alternative branch of IF"),
        ("ELSEIF", "Additional conditional branch"),
        ("END IF", "End of block IF"),
        ("FOR", "Begin a FOR loop"),
        ("TO", "Specify FOR loop end value"),
        ("STEP", "Specify FOR loop increment"),
        ("NEXT", "End FOR loops: NEXT [inner [, outer ...]]"),
        ("WHILE", "Begin a WHILE loop"),
        ("WEND", "End a classic WHILE loop"),
        ("END WHILE", "End of WHILE loop"),
        ("DO", "Begin a DO loop"),
        ("LOOP", "End of DO loop"),
        ("UNTIL", "Loop until condition is true"),
        ("SELECT CASE", "Multi-way branch"),
        ("CASE", "Branch of SELECT CASE"),
        ("END SELECT", "End of SELECT CASE"),
        ("GOTO", "Jump to a label"),
        ("GOSUB", "Jump to a subroutine label in QuickBasic mode"),
        (
            "RETURN",
            "Return from GOSUB, optionally to a label, in QuickBasic mode",
        ),
        ("ON GOTO", "Computed GOTO in QuickBasic mode"),
        ("ON GOSUB", "Computed GOSUB in QuickBasic mode"),
        ("ON ERROR", "Classic error handler in QuickBasic mode"),
        ("RESUME", "Resume after classic error handler"),
        ("ERROR", "Raise a classic BASIC error code"),
        ("EXIT FOR", "Exit a FOR loop early"),
        ("EXIT DO", "Exit a DO loop early"),
        (
            "EXIT WHILE",
            "Exit the nearest WHILE loop in this procedure",
        ),
        (
            "EXIT SELECT",
            "Exit the nearest SELECT CASE in this procedure",
        ),
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
        (
            "BYREF",
            "Copy parameter back to a scalar or array element argument",
        ),
        (
            "DEF FN",
            "Define a single-expression or multiline QBasic function",
        ),
        ("DEFINT", "Set default numeric type for variable letters"),
        ("DEFLNG", "Set default numeric type for variable letters"),
        ("DEFSNG", "Set default numeric type for variable letters"),
        ("DEFDBL", "Set default numeric type for variable letters"),
        ("DEFSTR", "Set default string type for variable letters"),
        // Data
        (
            "DATA",
            "Define literal data, including signed numbers and empty fields",
        ),
        ("READ", "Read DATA into scalars or array elements"),
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
        ("OPEN", "Open a file"),
        ("CLOSE", "Close a file"),
        ("SET POINTER", "Set file position"),
        ("ASK POINTER", "Query file position"),
        ("GET", "Read a file value into a scalar or array element"),
        ("PUT", "Write a scalar or array element to a file"),
        ("SEEK", "Set file position (SEEK #n, pos)"),
        ("RESET", "Close all open files"),
        (
            "FIELD",
            "Map string variables to a random-file record buffer",
        ),
        (
            "LSET",
            "Left-align assignment into a fixed-width string field",
        ),
        (
            "RSET",
            "Right-align assignment into a fixed-width string field",
        ),
        // File system
        ("NAME", "Rename a file (NAME old$ AS new$)"),
        ("KILL", "Delete a file"),
        ("MKDIR", "Create a directory"),
        ("RMDIR", "Remove a directory"),
        ("CHDIR", "Change current directory"),
        ("CHDRIVE", "Change current drive"),
        ("FILES", "List directory entries"),
        (
            "ENVIRON",
            "Set a QBasic interpreter-local environment override",
        ),
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
            "Matrix operations; MAT PRINT/INPUT optionally use a file channel",
        ),
        // System
        ("SHELL", "Execute a system command"),
        ("SLEEP", "Pause execution"),
        ("RANDOMIZE", "Seed the random number generator"),
        // Operators
        ("AND", "Bitwise AND in QBasic; logical AND in ANSI"),
        ("OR", "Bitwise OR in QBasic; logical OR in ANSI"),
        ("NOT", "Bitwise NOT in QBasic; logical NOT in ANSI"),
        ("XOR", "Bitwise XOR in QBasic; logical XOR in ANSI"),
        ("EQV", "Bitwise equivalence in QBasic"),
        ("IMP", "Bitwise implication in QBasic"),
        (
            "\\",
            "Integer division in QBasic; rounds operands, truncates quotient",
        ),
        ("MOD", "Modulo operator"),
        // Other
        ("REM", "Comment"),
        ("AS", "Type specifier"),
    ]
    .iter()
    .map(|(kw, doc)| CompletionItem {
        label: kw.to_string(),
        kind: Some(if *kw == "\\" {
            CompletionItemKind::OPERATOR
        } else {
            CompletionItemKind::KEYWORD
        }),
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
        (
            "RND",
            "RND([n]) — Random number [0,1); parentheses required",
        ),
        (
            "ROUND",
            "ROUND(n[, places]) — Round; negative places select tens/hundreds",
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
            "TRUNCATE(n[, places]) — Truncate; negative places select tens/hundreds",
        ),
        ("REMAINDER", "REMAINDER(a, b) — Remainder of a / b"),
        ("MIN", "MIN(n[, n...]) — Minimum of one or more numbers"),
        ("MAX", "MAX(n[, n...]) — Maximum of one or more numbers"),
        ("MAXNUM", "MAXNUM() — Largest finite f64 value"),
        ("PI", "PI() — Value of pi (3.14159...)"),
        // String
        (
            "LEN",
            "LEN(value) — String character count or record binary byte size",
        ),
        ("LEFT$", "LEFT$(s$, n) — Left n characters"),
        ("RIGHT$", "RIGHT$(s$, n) — Right n characters"),
        (
            "MID$",
            "MID$(s$, start[, len]) — Substring; QB scalar/array overwrite statement",
        ),
        ("INSTR", "INSTR([start,] s$, search$) — Find substring"),
        (
            "INSTRREV",
            "INSTRREV(s$, search$[, start]) — Find the last substring match",
        ),
        (
            "REPLACE$",
            "REPLACE$(s$, find$, replacement$[, start[, count]]) — Replace substrings",
        ),
        ("UCASE$", "UCASE$(s$) — Convert to uppercase"),
        ("LCASE$", "LCASE$(s$) — Convert to lowercase"),
        ("LTRIM$", "LTRIM$(s$) — Remove leading Unicode whitespace"),
        ("RTRIM$", "RTRIM$(s$) — Remove trailing Unicode whitespace"),
        ("SPACE$", "SPACE$(n) — String of n spaces"),
        ("STRING$", "STRING$(n, char) — Repeat character n times"),
        ("CHR$", "CHR$(n) — Character U+0000..U+00FF"),
        ("ASC", "ASC(s$) — Unicode scalar value of first character"),
        ("STR$", "STR$(n) — Convert number to string"),
        (
            "VAL",
            "VAL(s$) — Read a numeric prefix, including E/D exponents",
        ),
        ("HEX$", "HEX$(n) — Hexadecimal representation"),
        ("OCT$", "OCT$(n) — Octal representation"),
        ("MKI$", "MKI$(n) — Packed 2-byte integer string"),
        ("MKL$", "MKL$(n) — Packed 4-byte long string"),
        ("MKS$", "MKS$(n) — Packed 4-byte single string"),
        ("MKD$", "MKD$(n) — Packed 8-byte double string"),
        // File
        ("FREEFILE", "FREEFILE — Next available file number"),
        ("EOF", "EOF(n) — End-of-file check"),
        ("LOF", "LOF(n) — Length of file"),
        ("LOC", "LOC(n) — Current position in file"),
        ("SEEK", "SEEK(n) — Next file position (1-based)"),
        ("ERR", "ERR — Last classic BASIC error code"),
        ("ERL", "ERL — Line number of last classic BASIC error"),
        // System
        (
            "ENVIRON$",
            "ENVIRON$(name$ | index) — Read an environment value or entry",
        ),
        (
            "CURDIR$",
            "CURDIR$() — Current process directory; no drive argument",
        ),
        (
            "COMMAND$",
            "COMMAND$() — BASIC program arguments joined with spaces",
        ),
        ("TIMER", "TIMER — Seconds since midnight"),
        ("DATE$", "DATE$ — Current date"),
        ("TIME$", "TIME$ — Current time"),
        // Conversion
        ("CINT", "CINT(n) — Round to nearest integer"),
        ("CLNG", "CLNG(n) — Round to nearest long"),
        ("CSNG", "CSNG(n) — Single-precision value"),
        ("CDBL", "CDBL(n) — Double-precision value"),
        ("CVI", "CVI(s$) — Convert packed integer string"),
        ("CVL", "CVL(s$) — Convert packed long string"),
        ("CVS", "CVS(s$) — Convert packed single string"),
        ("CVD", "CVD(s$) — Convert packed double string"),
        // Array
        ("LBOUND", "LBOUND(array[, dim]) — Lower bound of array"),
        ("UBOUND", "UBOUND(array[, dim]) — Upper bound of array"),
        (
            "DET",
            "DET(array) — Matrix determinant; DET() — Last inverse determinant",
        ),
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
            "DATE$ =",
            "```basic\nDATE$ = \"MM-DD-YYYY\"\n```\nSets the value returned by subsequent DATE$ reads (does not change the host clock).",
        ),
        (
            "TIME$ =",
            "```basic\nTIME$ = \"HH:MM:SS\"\n```\nSets the value returned by subsequent TIME$ reads (does not change the host clock).",
        ),
        (
            "RND",
            "```basic\nRND([n])\n```\nNo argument or positive n advances the generator; zero returns its previous value; negative n reseeds it. Results are in [0, 1). Bare RND is a variable. Rice does not reproduce QBasic's sequence.",
        ),
        (
            "ROUND",
            "```basic\nROUND(n[, places])\n```\nRounds to the nearest value, ties away from zero. Places defaults to zero; an explicit value truncates and must be -308..308. Negative places round to powers of ten: ROUND(125, -1) is 130. Finite overflow in the negative-place result raises overflow; existing nonnegative-place and NaN/infinity behavior is unchanged.",
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
            "```basic\nTRUNCATE(n[, places])\n```\nTruncates toward zero. Places defaults to zero; an explicit value truncates and must be -308..308. Negative places truncate to powers of ten: TRUNCATE(-129, -1) is -120. Available in both Rice dialects.",
        ),
        (
            "REMAINDER",
            "```basic\nREMAINDER(a, b)\n```\nReturns the floating-point remainder using a truncating quotient. Unlike MOD, a nonzero result has the dividend's sign. A zero divisor raises an error.",
        ),
        (
            "MIN",
            "```basic\nMIN(n[, n...])\n```\nReturns the smallest of one or more numeric arguments in either dialect. Strings and records are rejected. NaN propagates; infinities are supported. Parentheses are required.",
        ),
        (
            "MAX",
            "```basic\nMAX(n[, n...])\n```\nReturns the largest of one or more numeric arguments in either dialect. Strings and records are rejected. NaN propagates; infinities are supported. Parentheses are required.",
        ),
        (
            "MAXNUM",
            "```basic\nMAXNUM()\n```\nReturns the largest finite f64 value. Parentheses are required; bare MAXNUM is a variable.",
        ),
        (
            "PI",
            "```basic\nPI()\n```\nReturns the f64 approximation of pi. Parentheses are required; bare PI is a variable.",
        ),
        // String
        (
            "LEN",
            "```basic\nLEN(string_expression)\nLEN(record_expression)\n```\nFor a string, counts Unicode scalar values, not UTF-8 bytes or grapheme clusters. For a record, returns its current QB binary PUT payload size in bytes, including nested fields, fixed string widths, and dynamic string length prefixes; this is a Rice extension in both dialects. Numeric arguments are rejected. Exactly one argument is required.",
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
            "```basic\nMID$(s$, start[, length])\nMID$(lines$(i), start[, length]) = replacement$\n```\nThe function returns a substring in both dialects. The QB-only assignment overwrites a scalar or array element without changing its length. Indices resolve once before start, replacement, target snapshot, then optional length. Uses 1-based Unicode character positions; start must be positive, length nonnegative. Record-member and slice targets are unsupported.",
        ),
        (
            "INSTR",
            "```basic\nINSTR([start,] s$, search$)\n```\nReturns the position of `search$` in `s$`, or 0 if not found.",
        ),
        (
            "INSTRREV",
            "```basic\nINSTRREV(s$, search$[, start])\n```\nReturns the last case-sensitive match's 1-based Unicode character position, or 0. Omitted start or -1 searches the whole string; a positive start limits the final character of a match. Start truncates; 0 or values below -1 are errors. Empty text or start past the end returns 0; an empty search returns the effective start. Rice extension in both dialects.",
        ),
        (
            "REPLACE$",
            "```basic\nREPLACE$(s$, find$, replacement$[, start[, count]])\n```\nReplaces nonoverlapping, case-sensitive matches. Returns only the suffix beginning at the 1-based Unicode character start (default 1); start past the end returns empty. Count defaults to -1 (all), or is nonnegative. Empty find or count 0 keeps the selected suffix; an empty replacement deletes matches. Numeric options truncate and are checked. Rice extension in both dialects.",
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
            "```basic\nLTRIM$(s$)\n```\nRemoves leading Unicode whitespace, including spaces, tabs, and newlines.",
        ),
        (
            "RTRIM$",
            "```basic\nRTRIM$(s$)\n```\nRemoves trailing Unicode whitespace, including spaces, tabs, and newlines.",
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
            "```basic\nCHR$(n)\n```\nReturns character U+0000..U+00FF. The numeric argument is truncated and must be 0..255.",
        ),
        (
            "ASC",
            "```basic\nASC(s$)\n```\nReturns the first character's Unicode scalar value, which may exceed 255. Empty input raises an error.",
        ),
        (
            "STR$",
            "```basic\nSTR$(n)\n```\nConverts number `n` to its string representation.",
        ),
        (
            "VAL",
            "```basic\nVAL(s$)\n```\nConverts the longest valid decimal prefix, including E/e or D/d scientific exponents. For example VAL(\"1D2tail\") is 100 and VAL(\"1.2.3\") is 1.2. An incomplete exponent leaves the mantissa: VAL(\"2E+\") is 2. Existing radix and full-string NaN/infinity behavior is retained; see the builtin reference for limits.",
        ),
        (
            "HEX$",
            "```basic\nHEX$(n)\n```\nReturns the hexadecimal string representation of `n`.",
        ),
        (
            "OCT$",
            "```basic\nOCT$(n)\n```\nReturns the octal string representation of `n`.",
        ),
        (
            "MKI$",
            "```basic\nMKI$(n)\n```\nReturns a 2-byte little-endian packed integer string.",
        ),
        (
            "MKL$",
            "```basic\nMKL$(n)\n```\nReturns a 4-byte little-endian packed long string.",
        ),
        (
            "MKS$",
            "```basic\nMKS$(n)\n```\nReturns a 4-byte little-endian packed single-precision string.",
        ),
        (
            "MKD$",
            "```basic\nMKD$(n)\n```\nReturns an 8-byte little-endian packed double-precision string.",
        ),
        // File
        (
            "FREEFILE",
            "```basic\nFREEFILE\n```\nReturns the lowest unused file number in 1..255, or zero when exhausted. Bare FREEFILE is the supported syntax; FREEFILE() is rejected.",
        ),
        (
            "EOF",
            "```basic\nEOF(n)\n```\nReturns the dialect true value if at end of file `n`, 0 otherwise.",
        ),
        (
            "LOF",
            "```basic\nLOF(n)\n```\nReturns the length in bytes of file `n`.",
        ),
        (
            "LOC",
            "```basic\nLOC(n)\n```\nReturns the current zero-based byte offset, including for RANDOM files. This is not a QBasic record-count position.",
        ),
        // System
        (
            "ENVIRON$",
            "```basic\nENVIRON$(name$)\nENVIRON$(index)\n```\nA string name returns its environment value, or empty string if absent. A numeric index is truncated and must be positive; it selects a 1-based NAME=value entry from the Unicode host environment merged with interpreter-local overrides, sorted by name (uppercased on Windows). Past the end returns empty string. The list is refreshed on each call. Both dialects support reads; only QB supports the ENVIRON setter.",
        ),
        (
            "ENVIRON",
            "```basic\nENVIRON \"name=value\"\n```\nQBasic mode only. Sets an interpreter-local override read by ENVIRON$ and passed to SHELL children; it does not modify the host process environment.",
        ),
        (
            "CURDIR$",
            "```basic\nCURDIR$()\n```\nReturns the host process working directory. No drive argument is supported. Parentheses are required; bare CURDIR$ is a variable.",
        ),
        (
            "COMMAND$",
            "```basic\nCOMMAND$()\n```\nReturns BASIC program arguments joined with single spaces, preserving case and Unicode. For example, `rice program.bas -- one two` returns `one two`. The source filename and Rice options are excluded; `--` allows dash-leading program arguments. Shell quoting is not reconstructed. New library/REPL interpreters default to an empty string; library callers can set arguments explicitly. COMMAND() is an alias; parentheses are required.",
        ),
        (
            "TIMER",
            "```basic\nTIMER\n```\nReturns local seconds since midnight. Non-Windows resolution is whole seconds; Windows includes milliseconds. Bare TIMER is the supported syntax; TIMER() is rejected.",
        ),
        (
            "DATE$",
            "```basic\nDATE$\n```\nReturns the current date as MM-DD-YYYY.",
        ),
        (
            "TIME$",
            "```basic\nTIME$\n```\nReturns the current time as HH:MM:SS.",
        ),
        // Array
        (
            "LBOUND",
            "```basic\nLBOUND(array[, dim])\n```\nReturns the lower bound of `array` for the given dimension (default 1).",
        ),
        (
            "UBOUND",
            "```basic\nUBOUND(array[, dim])\n```\nReturns the upper bound of `array` for the given dimension (default 1).",
        ),
        (
            "DET",
            "```basic\nDET(array)\nDET(array())\nDET()\n```\nWith a named array, computes the determinant of its current square, two-dimensional MAT shape using partial-pivot floating-point elimination. Returns zero for an exactly singular matrix. This Rice extension works in both dialects; indexed elements and other expressions are not matrix references. With no arguments, or as bare DET, returns the cached determinant from the latest successful MAT inverse. DET(array) does not change that cache.",
        ),
        (
            "CINT",
            "```basic\nCINT(n)\n```\nRounds `n` to the nearest integer (round half to even).",
        ),
        (
            "CLNG",
            "```basic\nCLNG(n)\n```\nRounds `n` to the nearest long integer (round half to even).",
        ),
        (
            "CSNG",
            "```basic\nCSNG(n)\n```\nReturns `n` reduced to single precision.",
        ),
        (
            "CDBL",
            "```basic\nCDBL(n)\n```\nReturns `n` as a double-precision value.",
        ),
        (
            "CVI",
            "```basic\nCVI(s$)\n```\nConverts a 2-byte packed integer string produced by `MKI$` back to a number.",
        ),
        (
            "CVL",
            "```basic\nCVL(s$)\n```\nConverts a 4-byte packed long string produced by `MKL$` back to a number.",
        ),
        (
            "CVS",
            "```basic\nCVS(s$)\n```\nConverts a 4-byte packed single string produced by `MKS$` back to a number.",
        ),
        (
            "CVD",
            "```basic\nCVD(s$)\n```\nConverts an 8-byte packed double string produced by `MKD$` back to a number.",
        ),
        (
            "ERR",
            "```basic\nERR\n```\nReturns the most recent classic BASIC error code from an `ON ERROR GOTO` handler.",
        ),
        (
            "ERL",
            "```basic\nERL\n```\nReturns the numbered BASIC line where the most recent classic error occurred, or 0 for unnumbered statements.",
        ),
        (
            "SEEK",
            "```basic\nSEEK(n)\nSEEK #n, position\n```\nFunction returns the 1-based byte position of the next read/write; statement moves the file pointer.",
        ),
    ])
});

static KEYWORD_HOVER_DOCS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        // I/O
        (
            "PRINT",
            "```basic\nPRINT [expression][;|,] ...\n```\nDisplays output on the screen. Use `;` to suppress spacing, `,` for tab zones. Use `TAB(n)` and `SPC(n)` for positioning. In either dialect, two adjacent quotes inside a string literal represent one quote: `PRINT \"a\"\"b\"` prints `a\"b`.",
        ),
        (
            "PRINT USING",
            "```basic\nPRINT USING format$; expr[, expr...]\n```\nFormatted output. `#` for digits, `.` for decimal, `$$` for currency, `^^^^` for scientific notation.",
        ),
        (
            "INPUT",
            "```basic\nINPUT [\"prompt\";] target[, target...]\nINPUT #channel, target[, target...]\nINPUT values(i, j), names$(i)\n```\nReads into scalar names or array elements in either dialect; record fields and slices are unsupported. Indices are evaluated left to right once before input, retaining destinations across retries; INPUT i, values(i) uses the old i. Values are assigned only after all fields validate. Interactive INPUT retries invalid fields; file INPUT reads comma-delimited fields. Use doubled quotes inside a literal prompt to include a quote.",
        ),
        (
            "LINE INPUT",
            "```basic\nLINE INPUT [\"prompt\";] target\nLINE INPUT #channel, target\nLINE INPUT lines$(i)\n```\nReads a complete line without comma parsing into a scalar name or array element in either dialect; record fields and slices are unsupported. Removes the line ending. The file form requires a string target; the console form assigns text even to a numeric target. Literal prompts accept doubled quotes.",
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
            "```basic\nDIM [SHARED] var[(dims)] [AS type]\n```\nDeclares and initializes values or records array bounds. Explicit array AS selects defaults for untouched elements: numeric zero, empty string, or a fresh record, overriding suffix/DEFSTR. Existing elements survive DIM and assignments remain permissive. All numeric kinds use f64; recorded bounds are not enforced. Array metadata is interpreter-wide, not scoped to procedures.",
        ),
        (
            "CONST",
            "```basic\nCONST name = expression[, name = expression ...]\n```\nDefines constants left to right at runtime; later expressions may use earlier names. Duplicate names in the current scope raise an error. Earlier definitions and expression side effects survive a later failure. No AS clauses or trailing commas are accepted. Available in both dialects.",
        ),
        (
            "SWAP",
            "```basic\nSWAP target1, target2\nSWAP values(i, j), replacement\n```\nExchanges scalar variables or array elements in either dialect. Both destinations resolve from left to right with each index evaluated once, before reading or writing either value. Missing elements honor explicit array AS defaults, otherwise the ordinary suffix/DEFSTR defaults. A target error leaves both destinations unwritten, apart from index-expression side effects. Record fields and slices are unsupported; no bounds or rank checks are added.",
        ),
        (
            "OPTION BASE",
            "```basic\nOPTION BASE {0|1}\n```\nSets the default lower bound for subsequent array declarations. Rice currently defaults to 1 in both dialects; select 0 explicitly for QBasic code that assumes a zero base.",
        ),
        (
            "OPTION EXPLICIT",
            "```basic\nOPTION EXPLICIT\n```\nQBasic mode only. Checks declarations at runtime; DIM, SHARED, STATIC, CONST, parameters, suffixes, and DEFtype can declare names. Existing ancestor values and some statement paths bypass checks.",
        ),
        (
            "OPTION DIALECT",
            "```basic\nOPTION DIALECT \"ANSI\"\nOPTION DIALECT \"QB\"\n```\nSelects ANSI mode or the default QBasic-compatible mode for source code and subsequent immediate REPL input.",
        ),
        (
            "REDIM",
            "```basic\nREDIM [PRESERVE] var(dims) [AS type]\n```\nUpdates array bounds and explicit AS defaults. Omitting AS clears the previous declared element type and restores suffix/DEFSTR defaults. Without PRESERVE, clears local element values. PRESERVE retains all existing values without conversion, even out-of-range elements; ordinary bounds remain unchecked.",
        ),
        (
            "ERASE",
            "```basic\nERASE array[, array...]\n```\nClears local flattened array elements and sets each base variable to numeric zero. Bounds and declared array type metadata remain, so future untouched elements retain their defaults.",
        ),
        (
            "SHARED",
            "```basic\nSHARED var[, var...]\n```\nRoutes named scalar/record variables to the root environment. Sharing an array base name does not share its flattened element keys.",
        ),
        (
            "STATIC",
            "```basic\nSTATIC var[, var...]\n```\nPreserves named scalar/record values between procedure calls. STATIC array syntax does not preserve elements or establish bounds; static state survives CLEAR.",
        ),
        (
            "CLEAR",
            "```basic\nCLEAR\n```\nClears variable values in the current environment and resets the DATA cursor. Constants, declarations, array metadata, procedures, static storage, and open files remain.",
        ),
        // Control flow
        (
            "IF",
            "```basic\nIF condition THEN\n  ...\n[ELSEIF condition THEN\n  ...]\n[ELSE\n  ...]\nEND IF\n```\nConditional execution. Also supports single-line form: `IF cond THEN stmt [ELSE stmt]`. In QBasic mode, `IF cond THEN 100 ELSE 200` jumps to the selected numeric line label. Either branch may use a statement instead of a line number; ANSI requires explicit GOTO.",
        ),
        (
            "THEN",
            "```basic\nIF condition THEN statement [ELSE statement]\nIF condition THEN 100 [ELSE 200]\n```\nBegins the true branch of IF. A numeric branch target is shorthand for GOTO in QBasic mode only. A newline immediately after THEN begins a block IF.",
        ),
        (
            "ELSE",
            "```basic\nIF condition THEN statement ELSE statement\nIF condition THEN 100 ELSE 200\n```\nBegins the false branch of IF. Single-line numeric targets are shorthand for GOTO in QBasic mode only.",
        ),
        (
            "\\",
            "```basic\nPRINT 7 \\ 2\n```\nQBasic mode only. Rounds each numeric operand to the nearest integer, ties to even, then divides signed 64-bit integers and truncates the quotient toward zero. Returns an f64. A divisor that rounds to zero raises division by zero; nonfinite or out-of-range operands and quotient overflow raise overflow. Multiplication and / bind more tightly; MOD binds less tightly.",
        ),
        (
            "EQV",
            "```basic\nresult = a EQV b\n```\nQBasic mode only. Complements the bitwise XOR of the operands after signed 64-bit casts that truncate fractions, saturate out-of-range values, and map NaN to zero. Returns an f64; both operands are always evaluated. Binds less tightly than XOR and more tightly than IMP; repeated EQV associates left to right.",
        ),
        (
            "IMP",
            "```basic\nresult = a IMP b\n```\nQBasic mode only. Combines the bitwise complement of the left operand with the right using OR, after signed 64-bit casts that truncate fractions, saturate out-of-range values, and map NaN to zero. Returns an f64; both operands are always evaluated. Has the lowest operator precedence, below EQV; repeated IMP associates left to right.",
        ),
        (
            "FOR",
            "```basic\nFOR var = start TO end [STEP inc]\n  ...\nNEXT [var [, outer ...]]\n```\nCounted loop. Default STEP is 1. NEXT may close several nested FOR loops, listing their variables from innermost to outermost, in either Rice dialect.",
        ),
        (
            "NEXT",
            "```basic\nNEXT\nNEXT i\nNEXT k, j, i\n```\nCloses one FOR loop when bare, or the named nested FOR loops from innermost to outermost. Each name must match the corresponding loop variable. A list cannot cross an unfinished IF, DO, WHILE, or procedure block. Supported in both Rice dialects.",
        ),
        (
            "WHILE",
            "```basic\nWHILE condition\n  ...\nWEND\n```\nLoop while condition is true. END WHILE is also accepted. EXIT WHILE leaves the nearest active WHILE loop in this procedure, without another condition check; supported as a Rice extension in both modes.",
        ),
        (
            "WEND",
            "```basic\nWHILE condition\n  ...\nWEND\n```\nEnds a classic QuickBasic-style WHILE loop.",
        ),
        (
            "DO",
            "```basic\nDO [{WHILE|UNTIL} condition]\n  ...\nLOOP [{WHILE|UNTIL} condition]\n```\nFlexible loop with condition at top or bottom.",
        ),
        (
            "SELECT",
            "```basic\nSELECT CASE expression\n  CASE value[, value...]\n    ...\n  CASE ELSE\n    ...\nEND SELECT\n```\nMulti-way branch based on expression value. EXIT SELECT leaves the nearest active SELECT CASE in this procedure, including from CASE ELSE; supported as a Rice extension in both modes.",
        ),
        (
            "EXIT",
            "```basic\nEXIT {FOR|DO|WHILE|SELECT|SUB|FUNCTION}\n```\nLeaves the matching construct. EXIT WHILE and EXIT SELECT are Rice extensions in both dialects: they propagate through intervening blocks, are consumed by the nearest matching active loop/selection, and cannot escape a SUB or FUNCTION. Unmatched uses raise a runtime error.",
        ),
        (
            "GOTO",
            "```basic\nGOTO label\n```\nTransfers execution to the specified label or line number.",
        ),
        (
            "GOSUB",
            "```basic\nGOSUB label\n...\nRETURN\n```\nQuickBasic compatibility mode only. Calls a label using the GOSUB return stack.",
        ),
        (
            "RETURN",
            "```basic\nRETURN [label_or_line]\n```\nQuickBasic compatibility mode only. Ends the most recent GOSUB, then continues after its call or jumps to the optional label in the caller's active scope. RETURN with a label still requires an active GOSUB and unwinds exactly one call. Procedure-local use is a Rice extension; it cannot cross a SUB or FUNCTION boundary.",
        ),
        (
            "ON",
            "```basic\nON expr GOTO label1, label2\nON expr GOSUB label1, label2\nON ERROR GOTO handler\n```\nQuickBasic compatibility mode only. Performs computed jumps/calls or installs a classic error handler.",
        ),
        (
            "ON GOTO",
            "```basic\nON expr GOTO label1, label2[, label3...]\n```\nQuickBasic compatibility mode only. Jumps to the label selected by the 1-based numeric expression.",
        ),
        (
            "ON GOSUB",
            "```basic\nON expr GOSUB label1, label2[, label3...]\n```\nQuickBasic compatibility mode only. Calls the label selected by the 1-based numeric expression.",
        ),
        (
            "ERROR",
            "```basic\nERROR code\n```\nQuickBasic compatibility mode only. Raises a classic BASIC error code that can be trapped with `ON ERROR GOTO`.",
        ),
        (
            "RESUME",
            "```basic\nRESUME\nRESUME NEXT\nRESUME label\n```\nQuickBasic compatibility mode only. Resumes execution after a classic `ON ERROR GOTO` handler. Resume is exact at top-level scope.",
        ),
        (
            "END",
            "```basic\nEND\n```\nEnds the current program, including when used in a SUB. Function evaluation currently discards this control flow, so END inside a FUNCTION does not stop its caller.",
        ),
        (
            "STOP",
            "```basic\nSTOP\n```\nEnds execution like END; no resumable debugger break is implemented. Function evaluation currently discards this control flow.",
        ),
        (
            "SYSTEM",
            "```basic\nSYSTEM\n```\nEnds execution like END; QUIT is an alias. A stored REPL RUN returns to the prompt. Function evaluation currently discards this control flow.",
        ),
        // Procedures
        (
            "SUB",
            "```basic\nSUB name (params)\n  ...\nEND SUB\n```\nDefines a subroutine. Called with `CALL name(args)` or just `name args`.",
        ),
        (
            "FUNCTION",
            "```basic\nFUNCTION name(params) [AS type] [STATIC]\n  name = return_value\nEND FUNCTION\n```\nReturns the value assigned to its exact name. AS initializes a fresh result on every call: empty string, numeric zero, or a recursively initialized record. Explicit AS overrides the suffix; without AS, a $ name starts empty and other names start at zero. Assignments remain permissive; AS does not enforce the final value. Record fields may be set directly through the function name.",
        ),
        (
            "CALL",
            "```basic\nCALL name(args)\n```\nCalls a SUB with the given arguments.",
        ),
        (
            "DECLARE",
            "```basic\nDECLARE SUB name (params)\nDECLARE FUNCTION name (params)\n```\nOptional declaration with no runtime validation or external linking. DECLARE FUNCTION does not accept a return AS clause.",
        ),
        (
            "BYVAL",
            "```basic\nSUB name (BYVAL x AS NUMERIC)\n```\nPasses a parameter by value. This is the default in ANSI mode.",
        ),
        (
            "BYREF",
            "```basic\nSUB name (BYREF x AS NUMERIC)\nCALL name(values(i))\n```\nCopies a value in and writes it back on exit for a scalar variable or array element argument, including record-valued elements. Array indices are evaluated once when the argument is read. Constants, function results, fields, parentheses, and other expressions are not written back. This is copy-in/copy-out, not a live reference; later aliased parameters write back last, including when the procedure errors. Default in QBasic mode; explicit BYREF is supported in ANSI mode. Array parameters declared with () still do not bind whole arrays.",
        ),
        (
            "DEF",
            "```basic\nDEF FNname[(params)] = expression\n```\nQBasic mode only. Also accepts a multiline body ending in END DEF. FN is a convention, not a required prefix.",
        ),
        (
            "DEFSTR",
            "```basic\nDEFSTR A-Z\n```\nQuickBasic compatibility mode only. Subsequently auto-initialized unsuffixed variables beginning with the listed letters default to string. DIM has its own defaults and does not inherit DEFSTR. DEFINT, DEFLNG, DEFSNG, and DEFDBL are accepted for numeric compatibility.",
        ),
        (
            "DEFINT",
            "```basic\nDEFINT A-Z\n```\nQuickBasic compatibility mode only. Accepted for default numeric type compatibility; numeric values are still stored as doubles internally.",
        ),
        (
            "DEFLNG",
            "```basic\nDEFLNG A-Z\n```\nQuickBasic compatibility mode only. Accepted for default numeric type compatibility; numeric values are still stored as doubles internally.",
        ),
        (
            "DEFSNG",
            "```basic\nDEFSNG A-Z\n```\nQuickBasic compatibility mode only. Accepted for default numeric type compatibility; numeric values are still stored as doubles internally.",
        ),
        (
            "DEFDBL",
            "```basic\nDEFDBL A-Z\n```\nQuickBasic compatibility mode only. Accepted for default numeric type compatibility; numeric values are still stored as doubles internally.",
        ),
        // Data
        (
            "DATA",
            "```basic\nDATA value[, value...]\nDATA +42, -3, , \"She said \"\"hi\"\"\",\n```\nDefines literal data in either dialect. Numeric literals accept one leading + or -; expressions are not allowed. Omitted fields between commas or at either end are empty items; DATA , supplies two, while bare DATA supplies none. READ converts an empty item to zero or an empty string according to the target's current/default type; MAT READ uses zero. Doubled quotes inside a string literal represent one quote.",
        ),
        (
            "READ",
            "```basic\nREAD target[, target...]\nREAD values(i, j), names$(i)\n```\nReads DATA into scalar names or array elements in either dialect; record fields and slices are unsupported. Targets are resolved and assigned left to right; READ i, values(i) uses the newly read i. Explicit DATA values are not coerced. Empty DATA fields become zero or an empty string according to the current/default target type; a record target rejects an empty item without consuming it. Earlier assignments and consumed DATA are retained if a later target fails.",
        ),
        (
            "RESTORE",
            "```basic\nRESTORE [label]\n```\nResets the DATA pointer to the beginning or to a label attached directly to DATA. Unknown DATA labels silently rewind.",
        ),
        // User-defined types
        (
            "TYPE",
            "```basic\nTYPE name\n  field AS type\n  ...\nEND TYPE\n```\nDefines a record in either dialect. Supports nested nonrecursive records and STRING * literal fields. Types supply defaults/QBasic binary layouts; field assignments are not type-checked or fixed-width in memory.",
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
            "```basic\nCONTINUE\n```\nResumes the protected body after the failed direct statement, under the same handler. If a nested construct failed, skips that whole direct construct.",
        ),
        // File I/O
        (
            "OPEN",
            "```basic\nOPEN #n: NAME file$, ACCESS {INPUT|OUTPUT|OUTIN}, ORGANIZATION {SEQUENTIAL|STREAM}\nOPEN file$ FOR {INPUT|OUTPUT|APPEND|BINARY|RANDOM} AS #n\n```\nBoth forms are accepted in both dialects. GET/PUT use typed binary records in QBasic mode and a raw-string path in ANSI mode.",
        ),
        ("CLOSE", "```basic\nCLOSE #n\n```\nCloses an open file."),
        (
            "GET",
            "```basic\nGET #channel [, position [, target]]\nGET #1, , records(i)\n```\nReads into a scalar or array element; members, slices, and whole-array operands are not supported. Channel, position, and indices are evaluated once before file I/O. QB uses typed binary serialization, including record arrays; ANSI retains raw string transfers. GET checks writable destinations before reading. Omitting the operand retains FIELD-buffer behavior.",
        ),
        (
            "PUT",
            "```basic\nPUT #channel [, position [, source]]\nPUT #1, , values%(i, j)\n```\nWrites a scalar or array element; members, slices, and whole-array operands are not supported. Channel, position, and indices are evaluated once before file I/O. QB binary types use the original array name's suffix and record metadata, not the flattened element key. PUT permits constant sources. ANSI retains raw string transfers; omitted operands retain FIELD-buffer behavior.",
        ),
        (
            "SET POINTER",
            "```basic\nSET #n: POINTER position\n```\nSets the file position for the next read or write.",
        ),
        (
            "ASK POINTER",
            "```basic\nASK #n: POINTER var\n```\nQueries the current file position into a variable.",
        ),
        (
            "FIELD",
            "```basic\nFIELD #n, width AS var$[, width AS var$...]\n```\nQuickBasic compatibility mode only. Maps string variables to byte slices of a channel record buffer. RANDOM mode is not enforced; ordinary assignment does not update the buffer.",
        ),
        (
            "LSET",
            "```basic\nLSET var$ = expr$\n```\nQuickBasic compatibility mode only. Left-aligns into a FIELD slot or the existing string value's current byte width, padding/truncating as needed. AS STRING * length is not retained for this operation.",
        ),
        (
            "RSET",
            "```basic\nRSET var$ = expr$\n```\nQuickBasic compatibility mode only. Right-aligns into a FIELD slot or the existing string value's current byte width, padding/truncating as needed. AS STRING * length is not retained for this operation.",
        ),
        // File system
        ("NAME", "```basic\nNAME old$ AS new$\n```\nRenames a file."),
        ("KILL", "```basic\nKILL file$\n```\nDeletes a file."),
        ("MKDIR", "```basic\nMKDIR dir$\n```\nCreates a directory."),
        ("RMDIR", "```basic\nRMDIR dir$\n```\nRemoves a directory."),
        (
            "RESET",
            "```basic\nRESET\n```\nFlushes and closes all open files.",
        ),
        (
            "CHDIR",
            "```basic\nCHDIR dir$\n```\nChanges the current working directory.",
        ),
        (
            "CHDRIVE",
            "```basic\nCHDRIVE drive$\n```\nChanges the current drive on platforms where drive roots are available.",
        ),
        (
            "FILES",
            "```basic\nFILES [path$]\n```\nLists directory entries to the program output.",
        ),
        // Console
        ("CLS", "```basic\nCLS\n```\nClears the screen."),
        (
            "LOCATE",
            "```basic\nLOCATE row[, col]\n```\nMoves the cursor to the specified row and column (1-based).",
        ),
        (
            "COLOR",
            "```basic\nCOLOR foreground[, background]\n```\nSets terminal colors: foreground and background 0..15. Rice does not record attributes for SCREEN queries.",
        ),
        ("BEEP", "```basic\nBEEP\n```\nSounds the terminal bell."),
        (
            "WIDTH",
            "```basic\nWIDTH columns\n```\nUpdates Rice's logical screen bounds, not the physical terminal size. WIDTH columns[, rows] is accepted; tracked output does not automatically wrap.",
        ),
        (
            "VIEW PRINT",
            "```basic\nVIEW PRINT [top TO bottom]\n```\nSets the scrolling region. Without arguments, resets to full screen.",
        ),
        // Matrix
        (
            "MAT",
            "```basic\nMAT C = A + B\nMAT C = A * B\nMAT B = INV(A)\nMAT B = TRN(A)\nMAT A = ZER\nMAT A = CON\nMAT A = IDN\nMAT READ A\nMAT PRINT A\nMAT PRINT #1, A\nMAT INPUT #2, A\nPRINT DET(A)\n```\nTwo-dimensional numeric matrix operations in both dialects. Declare destination bounds to match the result; MAT does not resize metadata. DET(A) computes a square matrix determinant without inversion. MAT READ converts empty DATA fields to zero. File MAT PRINT writes comma-separated rows; file MAT INPUT reads numeric fields in row-major order without console prompts. Each channel expression is evaluated once. Input replaces the matrix only after all fields succeed; errors leave consumed fields consumed. Console MAT PRINT uses spaces.",
        ),
        // System
        (
            "SHELL",
            "```basic\nSHELL command$\n```\nExecutes a system command.",
        ),
        (
            "SLEEP",
            "```basic\nSLEEP [seconds]\n```\nSleeps for positive whole seconds after truncation. No argument, zero, and negative values are no-ops; this does not wait for a keypress.",
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
    let line = state.source.split('\n').nth(pos.line as usize)?;
    let mut utf16_col = 0;
    let (byte_col, char_col) =
        line.char_indices()
            .enumerate()
            .find_map(|(char_col, (byte_col, ch))| {
                let at_cursor = utf16_col == pos.character as usize;
                utf16_col += ch.len_utf16();
                at_cursor.then_some((byte_col, char_col))
            })?;
    let token = state
        .tokens
        .iter()
        .take_while(|token| token.span.line <= pos.line as usize + 1)
        .filter(|token| token.span.line == pos.line as usize + 1 && token.span.col <= char_col + 1)
        .last()?;
    let name = token_name(&token.token)?;
    let start = line.char_indices().nth(token.span.col.saturating_sub(1))?.0;
    if matches!(token.token, rice::token::Token::Backslash) {
        return (byte_col == start).then_some(name);
    }
    let text = &line[start..];

    // A preceding token does not own trailing whitespace or comments. Canonical
    // names can differ from source spelling (QUIT/SYSTEM, compound keywords).
    let word_len = |text: &str| {
        text.bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || b"_$%!#&.".contains(b))
            .count()
    };
    let mut len = word_len(text);
    for word in name.split_whitespace().skip(1) {
        let rest = text[len..].trim_start_matches([' ', '\t']);
        let next_len = word_len(rest);
        if !rest[..next_len].eq_ignore_ascii_case(word) {
            break;
        }
        len = text.len() - rest.len() + next_len;
    }
    (byte_col < start + len && !line.as_bytes()[byte_col].is_ascii_whitespace()).then_some(name)
}

fn token_name(tok: &rice::token::Token) -> Option<String> {
    match tok {
        rice::token::Token::Identifier(name) => Some(name.clone()),
        rice::token::Token::NumericLiteral(n) => Some(n.to_string()),
        rice::token::Token::LineNumber(n) => Some(n.to_string()),
        rice::token::Token::Backslash => Some("\\".into()),
        rice::token::Token::KwPrint => Some("PRINT".into()),
        rice::token::Token::KwLen => Some("LEN".into()),
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
        rice::token::Token::KwExplicit => Some("EXPLICIT".into()),
        rice::token::Token::KwRandomize => Some("RANDOMIZE".into()),
        rice::token::Token::KwTimer => Some("TIMER".into()),
        rice::token::Token::KwSystem => Some("SYSTEM".into()),
        rice::token::Token::KwStop => Some("STOP".into()),
        rice::token::Token::KwAnd => Some("AND".into()),
        rice::token::Token::KwOr => Some("OR".into()),
        rice::token::Token::KwNot => Some("NOT".into()),
        rice::token::Token::KwXor => Some("XOR".into()),
        rice::token::Token::KwEqv => Some("EQV".into()),
        rice::token::Token::KwImp => Some("IMP".into()),
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
        rice::token::Token::KwByRef => Some("BYREF".into()),
        rice::token::Token::KwSleep => Some("SLEEP".into()),
        rice::token::Token::KwClear => Some("CLEAR".into()),
        rice::token::Token::KwName => Some("NAME".into()),
        rice::token::Token::KwKill => Some("KILL".into()),
        rice::token::Token::KwMkdir => Some("MKDIR".into()),
        rice::token::Token::KwRmdir => Some("RMDIR".into()),
        rice::token::Token::KwChdir => Some("CHDIR".into()),
        rice::token::Token::KwChdrive => Some("CHDRIVE".into()),
        rice::token::Token::KwFiles => Some("FILES".into()),
        rice::token::Token::KwShell => Some("SHELL".into()),
        rice::token::Token::KwLset => Some("LSET".into()),
        rice::token::Token::KwRset => Some("RSET".into()),
        rice::token::Token::KwDef => Some("DEF".into()),
        rice::token::Token::KwDefInt => Some("DEFINT".into()),
        rice::token::Token::KwDefLng => Some("DEFLNG".into()),
        rice::token::Token::KwDefSng => Some("DEFSNG".into()),
        rice::token::Token::KwDefDbl => Some("DEFDBL".into()),
        rice::token::Token::KwDefStr => Some("DEFSTR".into()),
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
        rice::token::Token::KwSeek => Some("SEEK".into()),
        rice::token::Token::KwReset => Some("RESET".into()),
        rice::token::Token::KwField => Some("FIELD".into()),
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
        self.analyze(
            params.text_document.uri,
            params.text_document.text,
            params.text_document.version,
        )
        .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            self.analyze(
                params.text_document.uri,
                change.text,
                params.text_document.version,
            )
            .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.documents
            .write()
            .await
            .remove(&params.text_document.uri);
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;
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
            return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                uri: uri.clone(),
                range: source_line_range(&state.source, sym.line),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn const_lists_register_every_symbol_in_both_dialects() {
        for dialect in ["QB", "ANSI"] {
            let source = format!(
                "OPTION DIALECT \"{dialect}\"\nCONST First = 1, Second = First + 1, Title$ = \"ok\"\nSUB Worker\nCONST LocalOne = 2, LocalTwo = LocalOne + 1\nEND SUB\nPRINT Second; Title$\n"
            );
            let state = analyze_source(source, 1);
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            assert_eq!(state.symbols.constants.len(), 5);
            for (name, line) in [
                ("FIRST", 2),
                ("SECOND", 2),
                ("TITLE$", 2),
                ("LOCALONE", 4),
                ("LOCALTWO", 4),
            ] {
                let symbol = find_symbol(&state.symbols, name).unwrap();
                assert_eq!(symbol.line, line);
                assert_eq!(symbol.detail.as_deref(), Some("CONST"));
            }
        }
        let state = analyze_source("CONST first = 1,\n".into(), 1);
        assert!(!state.diagnostics.is_empty());
    }

    #[test]
    fn indexed_swap_registers_targets_and_preserves_index_references() {
        for dialect in ["QB", "ANSI"] {
            let state = analyze_source(
                format!(
                    "OPTION DIALECT \"{dialect}\"\nDIM index\nSWAP first(index, 2), second$(index)\n"
                ),
                1,
            );
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            for (column, name, definition_line) in
                [(5, "FIRST", 3), (11, "INDEX", 2), (22, "SECOND$", 3)]
            {
                assert_eq!(
                    resolve_token_name(&state, Position::new(2, column)).as_deref(),
                    Some(name)
                );
                assert_eq!(
                    find_symbol(&state.symbols, name).unwrap().line,
                    definition_line
                );
            }
            assert_eq!(state.symbols.variables.len(), 3);
            for invalid in ["SWAP a(), b", "SWAP a(1:2), b", "SWAP a.x, b"] {
                let state = analyze_source(format!("OPTION DIALECT \"{dialect}\"\n{invalid}\n"), 1);
                assert_eq!(state.diagnostics.len(), 1, "{invalid}");
                assert_eq!(state.diagnostics[0].range.start.line, 1);
            }
        }
        assert!(
            keyword_hover("SWAP")
                .unwrap()
                .contains("each index evaluated once")
        );
        assert!(
            KEYWORD_COMPLETIONS
                .iter()
                .find(|item| item.label == "SWAP")
                .unwrap()
                .detail
                .as_ref()
                .unwrap()
                .contains("array elements")
        );
    }

    #[test]
    fn data_fields_and_return_targets_have_diagnostics_and_hover() {
        for dialect in ["QB", "ANSI"] {
            let state = analyze_source(
                format!("OPTION DIALECT \"{dialect}\"\nDATA +2, -3, , \"😀\",\nREAD n, m, s$\n"),
                1,
            );
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            for data in ["DATA +", "DATA 1 + 2", "DATA --1"] {
                let state = analyze_source(format!("OPTION DIALECT \"{dialect}\"\n{data}\n"), 1);
                assert_eq!(state.diagnostics.len(), 1, "{data}");
            }
        }
        let state = analyze_source(
            "RETURN destination\nRETURN 30\ndestination: PRINT 1\n30 END\n".into(),
            1,
        );
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        for (line, name, definition_line) in [(0, "DESTINATION", 3), (1, "30", 4)] {
            assert_eq!(
                resolve_token_name(&state, Position::new(line, 7)).as_deref(),
                Some(name)
            );
            assert_eq!(
                find_symbol(&state.symbols, name).unwrap().line,
                definition_line
            );
        }
        let ansi = analyze_source("OPTION DIALECT \"ANSI\"\nRETURN destination\n".into(), 1);
        assert_eq!(ansi.diagnostics.len(), 1);
        assert_eq!(ansi.diagnostics[0].range.start.line, 1);
        assert!(
            keyword_hover("RETURN")
                .unwrap()
                .contains("unwinds exactly one call")
        );
        assert!(
            keyword_hover("DATA")
                .unwrap()
                .contains("bare DATA supplies none")
        );
        assert!(
            keyword_hover("READ")
                .unwrap()
                .contains("without consuming it")
        );
    }

    #[test]
    fn record_len_and_matrix_det_have_builtin_hover_and_completion() {
        let line = "PRINT \"😀\"; LEN(record), DET(matrix), DET(matrix()) ' LEN DET";
        for dialect in ["QB", "ANSI"] {
            let state = analyze_source(format!("OPTION DIALECT \"{dialect}\"\n{line}\n"), 1);
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            for (name, expected) in [
                ("LEN", "record_expression"),
                ("DET", "two-dimensional MAT shape"),
            ] {
                let column = line[..line.find(name).unwrap()].encode_utf16().count() as u32;
                assert_eq!(
                    resolve_token_name(&state, Position::new(1, column)).as_deref(),
                    Some(name)
                );
                assert!(builtin_hover(name).unwrap().contains(expected));
                assert_eq!(
                    BUILTIN_COMPLETIONS
                        .iter()
                        .find(|item| item.label == name)
                        .unwrap()
                        .kind,
                    Some(CompletionItemKind::FUNCTION)
                );
                let comment_column =
                    line[..line.rfind(name).unwrap()].encode_utf16().count() as u32;
                assert_eq!(
                    resolve_token_name(&state, Position::new(1, comment_column)),
                    None
                );
            }
        }
    }

    #[test]
    fn byref_element_arguments_preserve_array_and_index_references() {
        for dialect in ["QB", "ANSI"] {
            let state = analyze_source(
                format!(
                    "OPTION DIALECT \"{dialect}\"\nDIM values(2), index\nSUB Update(BYREF item)\nitem = 3\nEND SUB\nCALL Update(values(index))\n"
                ),
                1,
            );
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            for (column, name) in [(12, "VALUES"), (19, "INDEX")] {
                assert_eq!(
                    resolve_token_name(&state, Position::new(5, column)).as_deref(),
                    Some(name)
                );
                assert_eq!(find_symbol(&state.symbols, name).unwrap().line, 2);
            }
        }
        let hover = keyword_hover("BYREF").unwrap();
        assert!(hover.contains("including record-valued elements"));
        assert!(hover.contains("evaluated once"));
        assert!(
            KEYWORD_COMPLETIONS
                .iter()
                .find(|item| item.label == "BYREF")
                .unwrap()
                .detail
                .as_ref()
                .unwrap()
                .contains("array element")
        );
    }

    #[test]
    fn cursor_resolution_uses_utf16_and_excludes_comments_and_whitespace() {
        let state = analyze_source("PRINT \"😀\"; VALUE ' comment\n".into(), 1);
        assert_eq!(
            resolve_token_name(&state, Position::new(0, 12)).as_deref(),
            Some("VALUE")
        );
        assert_eq!(
            resolve_token_name(&state, Position::new(0, 16)).as_deref(),
            Some("VALUE")
        );
        for character in [5, 17, 19, 23, 100] {
            assert_eq!(
                resolve_token_name(&state, Position::new(0, character)),
                None,
                "{character}"
            );
        }
    }

    #[test]
    fn cursor_resolution_preserves_compound_and_alias_keywords() {
        let state = analyze_source("END\tIF\nQUIT\n".into(), 1);
        assert_eq!(
            resolve_token_name(&state, Position::new(0, 4)).as_deref(),
            Some("END IF")
        );
        assert_eq!(
            resolve_token_name(&state, Position::new(1, 3)).as_deref(),
            Some("SYSTEM")
        );
        assert_eq!(resolve_token_name(&state, Position::new(1, 4)), None);
    }

    #[test]
    fn diagnostics_and_line_ranges_use_actual_utf16_columns() {
        let state = analyze_source("PRINT \"😀\"; @".into(), 1);
        assert_eq!(
            state.diagnostics[0].range,
            Range::new(Position::new(0, 12), Position::new(0, 13))
        );
        assert_eq!(
            source_line_range("PRINT \"😀\"\r\n", 1),
            Range::new(Position::new(0, 0), Position::new(0, 10))
        );
        let state = analyze_source("PRINT (".into(), 1);
        assert_eq!(state.diagnostics[0].range.end, Position::new(0, 7));
    }

    #[test]
    fn symbols_include_exception_bodies_and_handlers() {
        let state = analyze_source(
            "WHEN EXCEPTION IN\n  inside = 1\nUSE\n  recovered = 1\nEND WHEN\n".into(),
            1,
        );
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        assert!(find_symbol(&state.symbols, "INSIDE").is_some());
        assert!(find_symbol(&state.symbols, "RECOVERED").is_some());
    }

    #[test]
    fn integer_division_hover_uses_its_punctuation_span() {
        let state = analyze_source("PRINT \"😀\"; 7 \\ 2 ' \\ comment\n".into(), 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        let name = resolve_token_name(&state, Position::new(0, 14)).unwrap();
        assert_eq!(name, "\\");
        let hover = keyword_hover(&name).unwrap();
        assert!(hover.contains("ties to even"));
        assert!(hover.contains("64-bit"));
        for character in [13, 15, 20, 23] {
            assert_eq!(
                resolve_token_name(&state, Position::new(0, character)),
                None,
                "{character}"
            );
        }
        let completion = KEYWORD_COMPLETIONS
            .iter()
            .find(|item| item.label == "\\")
            .unwrap();
        assert_eq!(completion.kind, Some(CompletionItemKind::OPERATOR));
    }

    #[test]
    fn numeric_if_targets_resolve_to_numbered_labels() {
        let source = "10 IF MIN(3, MAX(1, 2)) THEN 30 ELSE 40\n30 END\n40 END\n";
        let state = analyze_source(source.into(), 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        for (branch, offset, expected_line) in [("THEN", 5, 2), ("ELSE", 5, 3)] {
            let col = source.find(branch).unwrap() + offset;
            let name = resolve_token_name(&state, Position::new(0, col as u32)).unwrap();
            assert_eq!(
                find_symbol(&state.symbols, &name).unwrap().line,
                expected_line
            );
            assert!(keyword_hover(branch).unwrap().contains("QBasic"));
        }
        assert!(keyword_hover("IF").unwrap().contains("THEN 100 ELSE 200"));
    }

    #[test]
    fn min_max_have_completions_and_hover_in_both_dialects() {
        for dialect in ["QB", "ANSI"] {
            let source = format!("OPTION DIALECT \"{dialect}\"\nPRINT MIN(3, MAX(1, 2))\n");
            let state = analyze_source(source, 1);
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            for (expected_name, character) in [("MIN", 6), ("MAX", 13)] {
                let name = resolve_token_name(&state, Position::new(1, character)).unwrap();
                assert_eq!(name, expected_name);
                let hover = builtin_hover(&name).unwrap();
                assert!(hover.contains("one or more numeric arguments"));
                assert!(hover.contains("NaN propagates"));
                let completion = BUILTIN_COMPLETIONS
                    .iter()
                    .find(|item| item.label == name)
                    .unwrap();
                assert_eq!(completion.kind, Some(CompletionItemKind::FUNCTION));
            }
        }
    }

    #[test]
    fn qbasic_expression_and_branch_syntax_produce_ansi_diagnostics() {
        for statement in ["PRINT 7 \\ 2", "IF 1 THEN 30 ELSE 40"] {
            let source = format!("OPTION DIALECT \"ANSI\"\n{statement}\n30 END\n40 END\n");
            let state = analyze_source(source, 1);
            assert_eq!(state.diagnostics.len(), 1, "{statement}");
            assert_eq!(
                state.diagnostics[0].severity,
                Some(DiagnosticSeverity::ERROR)
            );
            assert_eq!(state.diagnostics[0].range.start.line, 1);
        }
    }

    #[test]
    fn indexed_input_targets_and_subscripts_resolve_to_symbols() {
        for dialect in ["QB", "ANSI"] {
            let source = format!(
                "OPTION DIALECT \"{dialect}\"\nDIM index\nREAD table(index, 2)\nINPUT values(index)\nINPUT #1, filevalues$(index), count\nLINE INPUT lines$(index)\nLINE INPUT #1, filelines$(index)\n"
            );
            let state = analyze_source(source.clone(), 1);
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            for (name, expected_line) in [
                ("TABLE", 3),
                ("VALUES", 4),
                ("FILEVALUES$", 5),
                ("COUNT", 5),
                ("LINES$", 6),
                ("FILELINES$", 7),
            ] {
                assert_eq!(
                    find_symbol(&state.symbols, name).unwrap().line,
                    expected_line
                );
            }
            for (line_number, line) in source.lines().enumerate().skip(2) {
                let character = line.find("(index").unwrap() + 1;
                let name =
                    resolve_token_name(&state, Position::new(line_number as u32, character as u32))
                        .unwrap();
                assert_eq!(name, "INDEX");
                assert_eq!(find_symbol(&state.symbols, &name).unwrap().line, 2);
            }
            assert_eq!(state.symbols.variables.len(), 7);
        }
        for keyword in ["READ", "INPUT", "LINE INPUT"] {
            assert!(keyword_hover(keyword).unwrap().contains("array element"));
            assert!(KEYWORD_COMPLETIONS.iter().any(|item| item.label == keyword));
        }
    }

    #[test]
    fn eqv_imp_have_hover_completions_and_dialect_diagnostics() {
        for operator in ["EQV", "IMP"] {
            let source = format!("PRINT 3 {operator} 5\n");
            let state = analyze_source(source.clone(), 1);
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            let name = resolve_token_name(&state, Position::new(0, 8)).unwrap();
            assert_eq!(name, operator);
            let hover = keyword_hover(&name).unwrap();
            assert!(hover.contains("64-bit"));
            assert!(hover.contains("both operands are always evaluated"));
            assert!(
                KEYWORD_COMPLETIONS
                    .iter()
                    .any(|item| item.label == operator)
            );

            let ansi = analyze_source(format!("OPTION DIALECT \"ANSI\"\n{source}"), 1);
            assert_eq!(ansi.diagnostics.len(), 1);
            assert_eq!(ansi.diagnostics[0].range.start.line, 1);
        }
    }

    #[test]
    fn doubled_quotes_preserve_unicode_cursor_and_comment_boundaries() {
        let source = r#"PRINT "é ""' REM 😀"""; marker ' 日本語"#;
        for dialect in ["QB", "ANSI"] {
            let state = analyze_source(format!("OPTION DIALECT \"{dialect}\"\n{source}\n"), 1);
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            let character =
                |text: &str| source[..source.find(text).unwrap()].encode_utf16().count() as u32;
            assert_eq!(
                resolve_token_name(&state, Position::new(1, character("marker"))).as_deref(),
                Some("MARKER")
            );
            for text in ["REM", "😀", "日本語"] {
                assert_eq!(
                    resolve_token_name(&state, Position::new(1, character(text))),
                    None
                );
            }
            assert!(state.tokens.iter().any(|token| matches!(
                &token.token,
                rice::token::Token::StringLiteral(value) if value == "é \"' REM 😀\""
            )));
        }
    }

    #[test]
    fn escaped_directive_text_does_not_select_the_lsp_dialect() {
        let state = analyze_source(
            "PRINT \"OPTION DIALECT \"\"ANSI\"\"\"\nPRINT 1 EQV 1\n".into(),
            1,
        );
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
    }

    #[test]
    fn next_lists_preserve_counter_references_and_comma_spans() {
        for dialect in ["QB", "ANSI"] {
            let source = format!(
                "OPTION DIALECT \"{dialect}\"\nFOR outer = 1 TO 2\nFOR inner = 1 TO 2\nPRINT inner\nNEXT inner, outer\n"
            );
            let state = analyze_source(source, 1);
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            for (column, name, definition_line) in [(5, "INNER", 3), (12, "OUTER", 2)] {
                assert_eq!(
                    resolve_token_name(&state, Position::new(4, column)).as_deref(),
                    Some(name)
                );
                assert_eq!(
                    find_symbol(&state.symbols, name).unwrap().line,
                    definition_line
                );
            }
            assert_eq!(resolve_token_name(&state, Position::new(4, 10)), None);
            assert!(state.tokens.iter().any(|token| {
                token.span.line == 5 && matches!(token.token, rice::token::Token::Comma)
            }));
            assert!(
                keyword_hover("NEXT")
                    .unwrap()
                    .contains("innermost to outermost")
            );

            let invalid = analyze_source(
                format!(
                    "OPTION DIALECT \"{dialect}\"\nFOR outer = 1 TO 2\nFOR inner = 1 TO 2\nNEXT outer, inner\n"
                ),
                1,
            );
            assert_eq!(invalid.diagnostics.len(), 1);
            assert_eq!(invalid.diagnostics[0].range.start.line, 3);
        }
    }

    #[test]
    fn indexed_mid_and_core_builtins_have_editor_support() {
        let source = "CONST size = 2, last = size\nMID$(chunks$(last), 1) = REPLACE$(\"abc\", \"b\", \"X\")\nPRINT INSTRREV(\"abc\", \"c\"); ROUND(125, -1); TRUNCATE(129, -1)\n";
        let state = analyze_source(source.into(), 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        assert_eq!(find_symbol(&state.symbols, "CHUNKS$").unwrap().line, 2);
        assert_eq!(find_symbol(&state.symbols, "LAST").unwrap().line, 1);
        assert_eq!(
            resolve_token_name(&state, Position::new(1, 6)).as_deref(),
            Some("CHUNKS$")
        );
        for name in ["REPLACE$", "INSTRREV"] {
            assert!(BUILTIN_COMPLETIONS.iter().any(|item| item.label == name));
            assert!(builtin_hover(name).is_some());
            assert!(rice::builtins::BuiltinRegistry::new().exists(name));
        }
        let invalid = analyze_source(format!("OPTION DIALECT \"ANSI\"\n{source}"), 1);
        assert_eq!(invalid.diagnostics.len(), 1);
        assert_eq!(invalid.diagnostics[0].range.start.line, 2);
        assert!(invalid.diagnostics[0].message.contains("MID$ assignment"));
    }

    #[test]
    fn completion_and_hover_cover_language_completeness_additions() {
        for dialect in ["QB", "ANSI"] {
            let source = format!(
                "OPTION DIALECT \"{dialect}\"\nDIM values(2)\nWHILE 1\nSELECT CASE 1\nCASE 1\nGET #1, , values(1)\nPUT #1, , values(2)\nEXIT SELECT\nEND SELECT\nEXIT WHILE\nWEND\nFUNCTION TextValue() AS STRING\nEND FUNCTION\nPRINT VAL(\"1D2tail\"); ENVIRON$(1)\n"
            );
            let state = analyze_source(source, 1);
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            assert_eq!(
                find_symbol(&state.symbols, "TEXTVALUE")
                    .unwrap()
                    .detail
                    .as_deref(),
                Some("FUNCTION TEXTVALUE() AS STRING")
            );
            assert_eq!(
                resolve_token_name(&state, Position::new(3, 7)).as_deref(),
                Some("CASE")
            );
        }
        for label in ["EXIT WHILE", "EXIT SELECT", "GET", "PUT", "FUNCTION"] {
            assert!(KEYWORD_COMPLETIONS.iter().any(|item| item.label == label));
        }
        for label in ["VAL", "ENVIRON$"] {
            assert!(BUILTIN_COMPLETIONS.iter().any(|item| item.label == label));
        }
        assert!(keyword_hover("EXIT").unwrap().contains("cannot escape"));
        assert!(keyword_hover("FUNCTION").unwrap().contains("fresh result"));
        assert!(keyword_hover("GET").unwrap().contains("array element"));
        assert!(keyword_hover("PUT").unwrap().contains("array element"));
        assert!(builtin_hover("VAL").unwrap().contains("1D2tail"));
        assert!(builtin_hover("ENVIRON$").unwrap().contains("NAME=value"));
    }
}
