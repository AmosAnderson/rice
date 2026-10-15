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
    let mut lexer = Lexer::new(&source);

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

    let mut parser = Parser::new(tokens.clone());
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
                syms.subs.retain(|symbol| symbol.name != sub.name);
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
                    .clone()
                    .or_else(|| BasicType::from_suffix(&func.name))
                    .as_ref()
                    .map(|ty| format!(" AS {}", type_signature(ty)))
                    .unwrap_or_default();
                syms.functions.retain(|symbol| symbol.name != func.name);
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
            Stmt::Declare(decl) => {
                let kind = if decl.is_function { "FUNCTION" } else { "SUB" };
                let params = decl
                    .params
                    .iter()
                    .map(param_signature)
                    .collect::<Vec<_>>()
                    .join(", ");
                let result_type = decl
                    .as_type
                    .clone()
                    .or_else(|| {
                        decl.is_function
                            .then(|| BasicType::from_suffix(&decl.name))
                            .flatten()
                    })
                    .map(|ty| format!(" AS {}", type_signature(&ty)))
                    .unwrap_or_default();
                let symbols = if decl.is_function {
                    &mut syms.functions
                } else {
                    &mut syms.subs
                };
                if !symbols.iter().any(|symbol| symbol.name == decl.name) {
                    symbols.push(SymbolInfo {
                        name: decl.name.clone(),
                        line: ls.line,
                        detail: Some(format!(
                            "DECLARE {kind} {}({params}){result_type}",
                            decl.name
                        )),
                    });
                }
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
            Stmt::Dim { decls, .. } | Stmt::Redim { decls, .. } | Stmt::Static(decls) => {
                let tag = match &ls.stmt {
                    Stmt::Redim { .. } => "REDIM",
                    Stmt::Static(_) => "STATIC",
                    Stmt::Dim { shared: true, .. } => "DIM SHARED",
                    _ => "DIM",
                };
                for d in decls {
                    let detail = declaration_signature(
                        tag,
                        &d.name,
                        d.dimensions.is_some(),
                        d.as_type.as_ref(),
                    );
                    add_declared_variable(syms, &d.name, ls.line, detail, seen_vars);
                }
            }
            Stmt::Shared(decls) | Stmt::Common { decls, .. } => {
                let tag = match &ls.stmt {
                    Stmt::Common { shared: true, .. } => "COMMON SHARED",
                    Stmt::Common { .. } => "COMMON",
                    _ => "SHARED",
                };
                for decl in decls {
                    let detail = declaration_signature(
                        tag,
                        &decl.name,
                        decl.is_array,
                        decl.as_type.as_ref(),
                    );
                    add_declared_variable(syms, &decl.name, ls.line, detail, seen_vars);
                }
            }
            Stmt::Const { decls } => {
                for decl in decls {
                    syms.constants.push(SymbolInfo {
                        name: decl.name.clone(),
                        line: ls.line,
                        detail: Some(declaration_signature(
                            "CONST",
                            &decl.name,
                            false,
                            decl.as_type.as_ref(),
                        )),
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
            detail: BasicType::from_suffix(name)
                .map(|ty| format!("{name} AS {}", type_signature(&ty))),
        });
    }
}

fn add_declared_variable(
    syms: &mut DocumentSymbols,
    name: &str,
    line: usize,
    detail: String,
    seen: &mut HashSet<String>,
) {
    if let Some(existing) = syms.variables.iter_mut().find(|symbol| symbol.name == name) {
        existing.line = line;
        existing.detail = Some(detail);
    } else {
        seen.insert(name.to_owned());
        syms.variables.push(SymbolInfo {
            name: name.to_owned(),
            line,
            detail: Some(detail),
        });
    }
}

fn declaration_signature(tag: &str, name: &str, is_array: bool, ty: Option<&BasicType>) -> String {
    let mut signature = format!("{tag} {name}{}", if is_array { "()" } else { "" });
    if let Some(ty) = ty.cloned().or_else(|| BasicType::from_suffix(name)) {
        signature.push_str(&format!(" AS {}", type_signature(&ty)));
    }
    signature
}

fn param_signature(p: &Param) -> String {
    let mut signature = if p.is_array {
        format!("{}()", p.name)
    } else {
        p.name.clone()
    };
    if p.by_val {
        signature.insert_str(0, "BYVAL ");
    }
    if let Some(ty) = p
        .as_type
        .clone()
        .or_else(|| BasicType::from_suffix(&p.name))
    {
        signature.push_str(&format!(" AS {}", type_signature(&ty)));
    }
    signature
}

fn type_signature(ty: &BasicType) -> String {
    match ty {
        BasicType::Null => "NULL".into(),
        BasicType::Numeric => "NUMERIC".into(),
        BasicType::Integer => "INTEGER".into(),
        BasicType::Long => "LONG".into(),
        BasicType::Single => "SINGLE".into(),
        BasicType::Double => "DOUBLE".into(),
        BasicType::Int64 => "INT64".into(),
        BasicType::Bool => "BOOL".into(),
        BasicType::Byte => "BYTE".into(),
        BasicType::Bytes => "BYTES".into(),
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
        ("CONST", "Declare typed constants: CONST n AS INTEGER = 1"),
        ("SWAP", "Swap scalar variables or indexed array elements"),
        ("OPTION BASE", "Set default array lower bound"),
        ("OPTION EXPLICIT", "Require variable declarations"),
        ("REDIM", "Redimension an array"),
        ("ERASE", "Erase an array"),
        ("SHARED", "Share a typed variable with the main module"),
        ("COMMON", "Declare shared variables with optional types"),
        ("STATIC", "Preserve local variables between calls"),
        (
            "CLEAR",
            "Clear current variable values, DATA cursor, and database resources",
        ),
        // Control flow
        ("IF", "Conditional statement; numeric branch targets"),
        ("THEN", "Part of IF statement"),
        ("ELSE", "Alternative branch of IF"),
        ("ELSEIF", "Additional conditional branch"),
        ("END IF", "End of block IF"),
        ("FOR", "Begin a FOR loop"),
        ("TO", "Specify FOR loop end value"),
        ("STEP", "Specify FOR loop increment"),
        ("NEXT", "End FOR loops: NEXT [inner [, outer ...]]"),
        ("WHILE", "Begin a WHILE loop"),
        ("WEND", "End a WHILE loop"),
        ("END WHILE", "End of WHILE loop"),
        ("DO", "Begin a DO loop"),
        ("LOOP", "End of DO loop"),
        ("UNTIL", "Loop until condition is true"),
        ("SELECT CASE", "Multi-way branch"),
        ("CASE", "Branch of SELECT CASE"),
        ("END SELECT", "End of SELECT CASE"),
        ("GOTO", "Jump to a label"),
        ("GOSUB", "Jump to a subroutine label"),
        ("RETURN", "Return from GOSUB, optionally to a label"),
        ("ON GOTO", "Computed GOTO"),
        ("ON GOSUB", "Computed GOSUB"),
        ("ON ERROR", "ON ERROR handler"),
        ("RESUME", "Resume after ON ERROR handler"),
        ("ERROR", "Raise a Rice error code"),
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
        ("CALL", "Call a SUB or database builtin"),
        (
            "DECLARE",
            "Declare and validate a SUB or FUNCTION signature",
        ),
        ("BYVAL", "Pass argument by value"),
        (
            "BYREF",
            "Copy parameter back to a scalar or array element argument",
        ),
        ("DEF FN", "Define a single-expression or multiline function"),
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
            "Left-align assignment into a string field or STRING variable",
        ),
        (
            "RSET",
            "Right-align assignment into a string field or STRING variable",
        ),
        // File system
        ("NAME", "Rename a file (NAME old$ AS new$)"),
        ("KILL", "Delete a file"),
        ("MKDIR", "Create a directory"),
        ("RMDIR", "Remove a directory"),
        ("CHDIR", "Change current directory"),
        ("CHDRIVE", "Change current drive"),
        ("FILES", "List directory entries"),
        ("ENVIRON", "Set an interpreter-local environment override"),
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
        ("AND", "Bitwise numeric AND or boolean AND"),
        ("OR", "Bitwise numeric OR or boolean OR"),
        ("NOT", "Bitwise numeric NOT or boolean NOT"),
        ("XOR", "Bitwise numeric XOR or boolean XOR"),
        ("EQV", "Bitwise equivalence"),
        ("IMP", "Bitwise implication"),
        ("LIKE", "Match a string against a wildcard pattern"),
        (
            "\\",
            "Integer division; rounds operands, truncates quotient",
        ),
        ("MOD", "Modulo operator"),
        // Other
        ("REM", "Comment"),
        ("AS", "Type specifier"),
    ]
    .iter()
    .map(|(kw, doc)| CompletionItem {
        label: kw.to_string(),
        kind: Some(if matches!(*kw, "\\" | "LIKE") {
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
            "LEN(value) — String character count, buffer length, or record byte size",
        ),
        ("LEFT$", "LEFT$(s$, n) — Left n characters"),
        ("RIGHT$", "RIGHT$(s$, n) — Right n characters"),
        (
            "MID$",
            "MID$(s$, start[, len]) — Substring; scalar/array overwrite statement",
        ),
        ("INSTR", "INSTR([start,] s$, search$) — Find substring"),
        (
            "STRCOMP",
            "STRCOMP(a$, b$[, compare]) — String ordering; 0 binary, 1 Unicode lowercase",
        ),
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
        ("ERR", "ERR — Last Rice error code"),
        ("ERL", "ERL — Line number of last ON ERROR error"),
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
        ("CINT", "CINT(n) — Checked 16-bit INTEGER, ties to even"),
        ("CLNG", "CLNG(n) — Checked 32-bit LONG, ties to even"),
        ("CSNG", "CSNG(n) — Single-precision value"),
        ("CDBL", "CDBL(n) — Double-precision value"),
        (
            "CINT64",
            "CINT64(value) — Exact signed 64-bit integer conversion",
        ),
        ("CBOOL", "CBOOL(value) — Explicit boolean conversion"),
        (
            "CBYTE",
            "CBYTE(value) — Exact unsigned byte conversion (0..255)",
        ),
        ("TRUE", "TRUE() — Boolean true"),
        ("FALSE", "FALSE() — Boolean false"),
        ("TYPEOF$", "TYPEOF$(value) — Runtime type name"),
        ("CVI", "CVI(s$) — Convert packed integer string"),
        ("CVL", "CVL(s$) — Convert packed long string"),
        ("CVS", "CVS(s$) — Convert packed single string"),
        ("CVD", "CVD(s$) — Convert packed double string"),
        // Native byte buffers
        ("BYTES", "BYTES([byte, ...]) — Construct a byte buffer"),
        (
            "BYTEBUFFER",
            "BYTEBUFFER(length[, fill]) — Allocate a byte buffer",
        ),
        ("BYTELEN", "BYTELEN(buffer) — Number of bytes"),
        (
            "BYTEAT",
            "BYTEAT(buffer, index) — Read a byte at a 1-based index",
        ),
        (
            "BYTESET",
            "BYTESET(buffer, index, byte) — Return an updated buffer copy",
        ),
        (
            "BYTESLICE",
            "BYTESLICE(buffer, start[, count]) — Copy a byte range",
        ),
        (
            "UTF8ENCODE",
            "UTF8ENCODE(text$) — Encode text as UTF-8 bytes",
        ),
        (
            "UTF8DECODE$",
            "UTF8DECODE$(buffer) — Decode valid UTF-8 bytes",
        ),
        (
            "HEXENCODE$",
            "HEXENCODE$(buffer) — Encode bytes as hexadecimal text",
        ),
        (
            "HEXDECODE",
            "HEXDECODE(text$) — Decode hexadecimal text into bytes",
        ),
        // SQLite databases and missing values
        (
            "DBOPEN",
            "DBOPEN(path$[, mode$[, timeoutMs]]) — Open a SQLite database",
        ),
        (
            "DBCLOSE",
            "DBCLOSE(db) — Close SQLite and invalidate its results",
        ),
        (
            "DBEXEC",
            "DBEXEC(db, sql$[, parameter, ...]) — Execute SQL with bound values",
        ),
        (
            "DBQUERY",
            "DBQUERY(db, sql$[, parameter, ...]) — Materialize SQLite result rows",
        ),
        (
            "DBNEXT",
            "DBNEXT(result) — Advance to a row; BOOL false at end",
        ),
        (
            "DBVALUE",
            "DBVALUE(result, column) — Read the current SQLite value",
        ),
        (
            "DBISNULL",
            "DBISNULL(result, column) — Test the current SQL value for NULL",
        ),
        (
            "DBCOLUMNCOUNT",
            "DBCOLUMNCOUNT(result) — Number of result columns",
        ),
        (
            "DBCOLUMNNAME$",
            "DBCOLUMNNAME$(result, index) — Result column name",
        ),
        (
            "DBCOLUMNTYPE$",
            "DBCOLUMNTYPE$(result, column) — Current SQLite storage class",
        ),
        ("DBROWCOUNT", "DBROWCOUNT(result) — Total materialized rows"),
        ("DBFREE", "DBFREE(result) — Release a SQLite result"),
        (
            "DBCHANGES",
            "DBCHANGES(db) — SQLite connection change counter",
        ),
        (
            "DBLASTID",
            "DBLASTID(db) — Last inserted SQLite rowid as INT64",
        ),
        (
            "DBBEGIN",
            "DBBEGIN(db[, mode$]) — Begin a SQLite transaction",
        ),
        ("DBCOMMIT", "DBCOMMIT(db) — Commit a SQLite transaction"),
        (
            "DBROLLBACK",
            "DBROLLBACK(db) — Roll back a SQLite transaction",
        ),
        (
            "DBINTRANSACTION",
            "DBINTRANSACTION(db) — Test whether SQLite autocommit is disabled",
        ),
        (
            "DBTIMEOUT",
            "DBTIMEOUT(db, timeoutMs) — Set the SQLite busy timeout",
        ),
        ("DBVERSION$", "DBVERSION$() — Bundled SQLite version"),
        (
            "NULL",
            "NULL() — Missing value, distinct from zero and empty values",
        ),
        ("ISNULL", "ISNULL(value) — BOOL test for a NULL value"),
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
    [
        "NUMERIC", "INTEGER", "LONG", "SINGLE", "DOUBLE", "STRING", "INT64", "BOOL", "BOOLEAN",
        "BYTE", "BYTES",
    ]
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
        (
            "DBOPEN",
            "```basic\nDBOPEN(path$[, mode$[, timeoutMs]])\n```\nReturns an INT64 connection handle. Modes rwc (default: read/write/create), rw (existing read/write), and ro (read-only) are case-insensitive. Relative paths use process cwd; :memory: creates a private memory database. An empty path opens a temporary database. Other filenames are literal, including file: prefixes and URI option text. Foreign keys start enabled; the default busy timeout is 5000 ms. Timeout is an exact integer in 0..2147483647. Connections persist between immediate submissions; CLEAR, fresh RUN, or interpreter destruction closes them.",
        ),
        (
            "DBCLOSE",
            "```basic\nDBCLOSE(db)\n```\nCloses a SQLite connection, rolls back uncommitted work, and invalidates its result handles. Returns BOOL TRUE. Invalid or already closed handles raise an error. CALL DBCLOSE(db) discards the result.",
        ),
        (
            "DBEXEC",
            "```basic\nDBEXEC(db, sql$[, parameter, ...])\n```\nExecutes exactly one SQLite statement with no result columns. Returns INT64 changed rows for DML, or zero for non-DML. Rejects row-returning SQL before stepping; PRAGMA preparation can still change settings even for a rejected call; use DBQUERY for SELECT, row-returning PRAGMA, or RETURNING. Bind data using ? placeholders and trailing values; the supplied parameter count must match SQLite indexed slots exactly. CALL DBEXEC(...) discards its result.",
        ),
        (
            "DBQUERY",
            "```basic\nDBQUERY(db, sql$[, parameter, ...])\n```\nExecutes exactly one row-returning SQLite statement and returns an INT64 result handle, positioned before its first row. All rows are materialized during this call; RETURNING writes also happen now. Bind ? placeholders with trailing values; parameter count must match exactly. Invalid UTF-8 column names raise a database error, including for empty results. A materialization error can follow successful SQL writes; use an explicit transaction for rollback. PRAGMAs can change settings during preparation even if validation later fails. Use DBNEXT, DBVALUE, and DBFREE; use SQL filtering or LIMIT for large results.",
        ),
        (
            "DBNEXT",
            "```basic\nDBNEXT(result)\n```\nAdvances a materialized SQLite result and returns BOOL TRUE for a row or FALSE at end. The first call advances from before the first row. Once exhausted, subsequent calls remain FALSE. DBVALUE and current-value type access fail before the first row and after exhaustion.",
        ),
        (
            "DBVALUE",
            "```basic\nDBVALUE(result, column)\n```\nReads the current row. Column is an exact 1-based index independent of OPTION BASE, or a unique name matched without ASCII case distinctions. Ambiguous names raise an error. SQLite INTEGER returns INT64, REAL NUMERIC, TEXT STRING, BLOB BYTES, and NULL the native NULL value. Requires a successful DBNEXT. Check ISNULL or DBISNULL before assigning a nullable value to a declared type. Invalid UTF-8 TEXT raises a database error; select CAST(column AS BLOB) for raw bytes.",
        ),
        (
            "DBISNULL",
            "```basic\nDBISNULL(result, column)\n```\nReturns BOOL indicating whether the current row value is SQLite NULL. Column is an exact 1-based index or a unique name matched without ASCII case distinctions. Requires a successful DBNEXT; empty text and empty BLOB are not NULL.",
        ),
        (
            "DBCOLUMNCOUNT",
            "```basic\nDBCOLUMNCOUNT(result)\n```\nReturns the INT64 number of result columns. Available before DBNEXT and for an empty result. A freed or invalid result handle raises an error.",
        ),
        (
            "DBCOLUMNNAME$",
            "```basic\nDBCOLUMNNAME$(result, index)\n```\nReturns a result column name at an exact 1-based index, independent of OPTION BASE. Available before DBNEXT and for an empty result. Names need not be unique; use aliases or numeric indices to avoid ambiguous name lookup.",
        ),
        (
            "DBCOLUMNTYPE$",
            "```basic\nDBCOLUMNTYPE$(result, column)\n```\nReturns the current value's SQLite storage class: NULL, INTEGER, REAL, TEXT, or BLOB. This describes the value, not the column's SQL declaration. Column is an exact 1-based index or unique name matched without ASCII case distinctions. Requires a successful DBNEXT.",
        ),
        (
            "DBROWCOUNT",
            "```basic\nDBROWCOUNT(result)\n```\nReturns the INT64 number of materialized rows, including before the first DBNEXT. DBQUERY already executed the complete statement; counting or iterating does not rerun it.",
        ),
        (
            "DBFREE",
            "```basic\nDBFREE(result)\n```\nFrees the materialized rows and invalidates the result handle. Returns BOOL TRUE. Handles are never reused within an interpreter; using a freed handle raises an error. CALL DBFREE(result) discards the result.",
        ),
        (
            "DBCHANGES",
            "```basic\nDBCHANGES(db)\n```\nReturns SQLite's connection change counter as INT64. Non-DML can leave the previous count or expose a count from internal SQLite writes. Use DBEXEC's return value for the submitted statement's count, which is zero for non-DML.",
        ),
        (
            "DBLASTID",
            "```basic\nDBLASTID(db)\n```\nReturns SQLite's last inserted rowid for this connection as an exact INT64, initially zero. SQLite rowid semantics apply; this is connection state, not a sequence generator.",
        ),
        (
            "DBBEGIN",
            "```basic\nDBBEGIN(db[, mode$])\n```\nBegins a transaction and returns BOOL TRUE. Modes DEFERRED (default), IMMEDIATE, and EXCLUSIVE are case-insensitive. Nested BEGIN raises a SQLite error; use SQL SAVEPOINT for nesting. CALL DBBEGIN(...) discards the result.",
        ),
        (
            "DBCOMMIT",
            "```basic\nDBCOMMIT(db)\n```\nCommits an active SQLite transaction and returns BOOL TRUE. A failed or busy commit may leave the transaction active for retry or rollback; inspect DBINTRANSACTION. Calling without an active transaction raises a SQLite error.",
        ),
        (
            "DBROLLBACK",
            "```basic\nDBROLLBACK(db)\n```\nRolls back an active SQLite transaction and returns BOOL TRUE. Calling without an active transaction raises a SQLite error. Error handlers do not automatically roll back; inspect DBINTRANSACTION before recovery.",
        ),
        (
            "DBINTRANSACTION",
            "```basic\nDBINTRANSACTION(db)\n```\nReturns BOOL TRUE while SQLite autocommit is disabled, otherwise FALSE. Reflects SQL transaction commands and DBBEGIN/DBCOMMIT/DBROLLBACK on the same connection. Handling an error does not automatically roll back a transaction.",
        ),
        (
            "DBTIMEOUT",
            "```basic\nDBTIMEOUT(db, timeoutMs)\n```\nSets SQLite's busy timeout in milliseconds and returns BOOL TRUE. Requires an exact integer in 0..2147483647; zero disables waiting for locks. New connections default to 5000 ms.",
        ),
        (
            "DBVERSION$",
            "```basic\nDBVERSION$()\n```\nReturns the bundled SQLite library version as STRING. Parentheses are required. Database functions may also be used through CALL to discard their result; arguments pass by value without copy-back.",
        ),
        (
            "NULL",
            "```basic\nNULL()\n```\nReturns the native missing NULL value. There is no AS NULL declaration; declared types reject NULL, while unsuffixed implicit variables may hold it. ISNULL tests for it and TYPEOF$ returns NULL. Rice equality treats NULL as equal only to NULL; other operators, conversions, and conditions reject it. SQL expressions follow SQLite's own NULL rules. Parentheses are required.",
        ),
        (
            "ISNULL",
            "```basic\nISNULL(value)\n```\nReturns BOOL TRUE only for the native NULL value. Zero, FALSE, an empty STRING, and an empty BYTES buffer return FALSE. Use before assigning a nullable SQLite result to a declared type.",
        ),
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
            "```basic\nRND([n])\n```\nNo argument or positive n advances the generator; zero returns its previous value; negative n reseeds it. Results are in [0, 1). Bare RND is a variable.",
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
            "```basic\nTRUNCATE(n[, places])\n```\nTruncates toward zero. Places defaults to zero; an explicit value truncates and must be -308..308. Negative places truncate to powers of ten: TRUNCATE(-129, -1) is -120.",
        ),
        (
            "REMAINDER",
            "```basic\nREMAINDER(a, b)\n```\nReturns the floating-point remainder using a truncating quotient. Unlike MOD, a nonzero result has the dividend's sign. A zero divisor raises an error.",
        ),
        (
            "MIN",
            "```basic\nMIN(n[, n...])\n```\nReturns the smallest of one or more numeric arguments. Strings and records are rejected. NaN propagates; infinities are supported. Parentheses are required.",
        ),
        (
            "MAX",
            "```basic\nMAX(n[, n...])\n```\nReturns the largest of one or more numeric arguments. Strings and records are rejected. NaN propagates; infinities are supported. Parentheses are required.",
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
            "```basic\nLEN(string_expression)\nLEN(buffer_expression)\nLEN(record_expression)\n```\nFor a string, counts Unicode scalar values, not UTF-8 bytes or grapheme clusters. For BYTES, counts bytes. For a record, returns its current binary PUT payload size in bytes, including nested fields, fixed string widths, and dynamic length prefixes. Numeric arguments are rejected. Exactly one argument is required.",
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
            "```basic\nMID$(s$, start[, length])\nMID$(lines$(i), start[, length]) = replacement$\n```\nThe function returns a substring. The assignment overwrites a scalar or array element without changing its length. Indices resolve once before start, replacement, target snapshot, then optional length. Uses 1-based Unicode character positions; start must be positive, length nonnegative. Record-member and slice targets are unsupported.",
        ),
        (
            "INSTR",
            "```basic\nINSTR([start,] s$, search$)\n```\nReturns the position of `search$` in `s$`, or 0 if not found.",
        ),
        (
            "INSTRREV",
            "```basic\nINSTRREV(s$, search$[, start])\n```\nReturns the last case-sensitive match's 1-based Unicode character position, or 0. Omitted start or -1 searches the whole string; a positive start limits the final character of a match. Start truncates; 0 or values below -1 are errors. Empty text or start past the end returns 0; an empty search returns the effective start.",
        ),
        (
            "STRCOMP",
            "```basic\nSTRCOMP(a$, b$[, compare])\n```\nReturns -1, 0, or 1 when a$ sorts before, equal to, or after b$, independent of numeric truth values. Mode 0 (default) compares case-sensitive Unicode scalar order; mode 1 lowercases both complete strings before comparing. Mode must be exactly numeric 0 or 1. No locale collation, full case folding, or Unicode normalization. Both inputs must be strings.",
        ),
        (
            "REPLACE$",
            "```basic\nREPLACE$(s$, find$, replacement$[, start[, count]])\n```\nReplaces nonoverlapping, case-sensitive matches. Returns only the suffix beginning at the 1-based Unicode character start (default 1); start past the end returns empty. Count defaults to -1 (all), or is nonnegative. Empty find or count 0 keeps the selected suffix; an empty replacement deletes matches. Numeric options truncate and are checked.",
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
            "```basic\nMKI$(n)\n```\nPacks an exact signed 16-bit integer as a two-character binary string in little-endian order. Fractions and values outside the INTEGER range raise errors.",
        ),
        (
            "MKL$",
            "```basic\nMKL$(n)\n```\nPacks an exact signed 32-bit integer as a four-character binary string in little-endian order. Fractions and values outside the LONG range raise errors.",
        ),
        (
            "MKS$",
            "```basic\nMKS$(n)\n```\nPacks a SINGLE value as a four-character binary string in little-endian order. Values round to f32; finite overflow raises an error.",
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
            "```basic\nEOF(n)\n```\nReturns -1 if at end of file `n`, 0 otherwise.",
        ),
        (
            "LOF",
            "```basic\nLOF(n)\n```\nReturns the length in bytes of file `n`.",
        ),
        (
            "LOC",
            "```basic\nLOC(n)\n```\nReturns the current zero-based byte offset, including for RANDOM files.",
        ),
        // System
        (
            "ENVIRON$",
            "```basic\nENVIRON$(name$)\nENVIRON$(index)\n```\nA string name returns its environment value, or empty string if absent. A numeric index is truncated and must be positive; it selects a 1-based NAME=value entry from the Unicode host environment merged with interpreter-local overrides, sorted by name (uppercased on Windows). Past the end returns empty string. The list is refreshed on each call. The ENVIRON statement sets interpreter-local overrides.",
        ),
        (
            "ENVIRON",
            "```basic\nENVIRON \"name=value\"\n```\nSets an interpreter-local override read by ENVIRON$ and passed to SHELL children; it does not modify the host process environment.",
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
            "```basic\nDET(array)\nDET(array())\nDET()\n```\nWith a named array, computes the determinant of its current square, two-dimensional MAT shape using partial-pivot floating-point elimination. Returns zero for an exactly singular matrix. Indexed elements and other expressions are not matrix references. With no arguments, or as bare DET, returns the cached determinant from the latest successful MAT inverse. DET(array) does not change that cache. A rejected inverse destination also leaves the cache unchanged.",
        ),
        (
            "CINT",
            "```basic\nCINT(n)\n```\nReturns a signed 16-bit INTEGER. Rounds numeric input to the nearest integer, ties to even, then checks -32768..32767. Nonfinite values and overflow raise errors; booleans convert to 1 or 0.",
        ),
        (
            "CLNG",
            "```basic\nCLNG(n)\n```\nReturns a signed 32-bit LONG. Rounds numeric input to the nearest integer, ties to even, then checks -2147483648..2147483647. Nonfinite values and overflow raise errors; booleans convert to 1 or 0.",
        ),
        (
            "CSNG",
            "```basic\nCSNG(n)\n```\nReturns a 32-bit SINGLE, rounding to single precision. Finite values that overflow to infinity raise overflow; existing NaN and infinity are preserved. Booleans convert to 1 or 0.",
        ),
        (
            "CDBL",
            "```basic\nCDBL(n)\n```\nReturns a double-precision NUMERIC value. This explicit conversion may round a large INT64; booleans become 1 or 0.",
        ),
        (
            "CINT64",
            "```basic\nCINT64(value)\nCINT64(\"9223372036854775807\")\n```\nReturns a native Rice signed 64-bit INT64. Accepts an exact integral number, boolean (0 or 1), or a complete decimal integer string with optional surrounding whitespace. Use a string for large exact values; ordinary numeric literals use floating point. Fractions, nonfinite numbers, and out-of-range values raise an error.",
        ),
        (
            "CBOOL",
            "```basic\nCBOOL(value)\n```\nReturns a native Rice BOOL from a boolean or finite number: zero is false and nonzero is true. Strings and byte buffers are rejected. Comparisons of NUMERIC values return -1 or 0; BOOL remains a distinct native type.",
        ),
        (
            "CBYTE",
            "```basic\nCBYTE(value)\n```\nReturns a native Rice BYTE. Accepts an exact integral number in 0..255 or a boolean (0 or 1). Fractions, strings, nonfinite numbers, and out-of-range values raise an error.",
        ),
        (
            "TRUE",
            "```basic\nTRUE()\n```\nReturns the native Rice BOOL value true. Parentheses force builtin lookup; an existing variable named TRUE remains a variable.",
        ),
        (
            "FALSE",
            "```basic\nFALSE()\n```\nReturns the native Rice BOOL value false. Parentheses force builtin lookup; an existing variable named FALSE remains a variable.",
        ),
        (
            "TYPEOF$",
            "```basic\nTYPEOF$(value)\n```\nReturns the runtime type name: NUMERIC, INTEGER, LONG, SINGLE, INT64, BOOL, BYTE, BYTES, STRING, NULL, or the declared record type's name. DOUBLE shares the NUMERIC representation. Reports the stored value's type.",
        ),
        (
            "BYTES",
            "```basic\nDIM data AS BYTES\ndata = BYTES(0, 127, 255)\n```\nNative Rice binary buffer, distinct from Unicode STRING. AS BYTES defaults to an empty buffer and requires a buffer on assignment. BYTES([byte, ...]) constructs a buffer; BYTES() is empty. Each argument must convert exactly to a byte in 0..255. Use UTF8ENCODE/UTF8DECODE$ or HEXENCODE$/HEXDECODE for explicit text conversion.",
        ),
        (
            "BYTEBUFFER",
            "```basic\nBYTEBUFFER(length[, fill])\n```\nReturns a new BYTES buffer of the requested nonnegative integral length, filled with zero or an explicit byte in 0..255. Allocation failures raise a BASIC error.",
        ),
        (
            "BYTELEN",
            "```basic\nBYTELEN(buffer)\n```\nReturns the number of bytes in a BYTES buffer. LEN(buffer) returns the same length. Strings are rejected.",
        ),
        (
            "BYTEAT",
            "```basic\nBYTEAT(buffer, index)\n```\nReturns a BYTE from a BYTES buffer. Indices are exact integers, 1-based, independent of OPTION BASE, and must refer to an existing byte.",
        ),
        (
            "BYTESET",
            "```basic\ndata = BYTESET(data, index, byte)\n```\nReturns a copy of a BYTES buffer with one byte replaced. The original buffer is unchanged until assigned. Index is an exact 1-based integer within the buffer; byte must convert exactly to 0..255.",
        ),
        (
            "BYTESLICE",
            "```basic\nBYTESLICE(buffer, start[, count])\n```\nReturns a copy of a byte range. Start is an exact 1-based integer in 1..length+1. Omitted count means the remaining bytes; a nonnegative integral count is clipped to the remaining length. Starting at length+1 returns an empty buffer.",
        ),
        (
            "UTF8ENCODE",
            "```basic\nUTF8ENCODE(text$)\n```\nEncodes a Unicode STRING as a native BYTES buffer containing UTF-8 bytes, including embedded zero bytes.",
        ),
        (
            "UTF8DECODE$",
            "```basic\nUTF8DECODE$(buffer)\n```\nDecodes a BYTES buffer as UTF-8 text. Invalid UTF-8 raises an error; bytes are never silently replaced.",
        ),
        (
            "HEXENCODE$",
            "```basic\nHEXENCODE$(buffer)\n```\nReturns two uppercase hexadecimal digits for each byte in a BYTES buffer. An empty buffer produces an empty string.",
        ),
        (
            "HEXDECODE",
            "```basic\nHEXDECODE(text$)\n```\nConverts an even number of hexadecimal digits to a BYTES buffer. Accepts uppercase or lowercase digits and empty text; prefixes, whitespace, odd lengths, and nonhexadecimal characters raise an error.",
        ),
        (
            "CVI",
            "```basic\nCVI(s$)\n```\nDecodes the first two characters of a packed binary string as a signed 16-bit INTEGER in little-endian order. Requires at least two characters.",
        ),
        (
            "CVL",
            "```basic\nCVL(s$)\n```\nDecodes the first four characters of a packed binary string as a signed 32-bit LONG in little-endian order. Requires at least four characters.",
        ),
        (
            "CVS",
            "```basic\nCVS(s$)\n```\nDecodes the first four characters of a packed binary string as a 32-bit SINGLE in little-endian order. Requires at least four characters.",
        ),
        (
            "CVD",
            "```basic\nCVD(s$)\n```\nConverts an 8-byte packed double string produced by `MKD$` back to a number.",
        ),
        (
            "ERR",
            "```basic\nERR\n```\nReturns the most recent Rice error code from an `ON ERROR GOTO` handler.",
        ),
        (
            "ERL",
            "```basic\nERL\n```\nReturns the numbered BASIC line where the most recent ON ERROR error occurred, or 0 for unnumbered statements.",
        ),
        (
            "SEEK",
            "```basic\nSEEK(n)\nSEEK #n, position\n```\nFunction returns the 1-based byte position of the next read/write; statement flushes pending output before repositioning both reader and writer. Flush failure prevents the requested seek.",
        ),
    ])
});

static KEYWORD_HOVER_DOCS: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        (
            "NUMERIC",
            "```basic\nDIM value AS NUMERIC\n```\nRice 64-bit floating-point type, also spelled DOUBLE. Defaults to zero and accepts numeric values. An INT64 must be exactly representable; use CDBL for explicit rounding. The # suffix selects this type.",
        ),
        (
            "INTEGER",
            "```basic\nDIM value AS INTEGER\n```\nRice signed 16-bit integer, -32768..32767, default zero. Assignments require exact integral values and reject fractions or overflow. The % suffix selects this type; CINT performs explicit nearest-even rounding.",
        ),
        (
            "LONG",
            "```basic\nDIM value AS LONG\n```\nRice signed 32-bit integer, -2147483648..2147483647, default zero. Assignments require exact integral values and reject fractions or overflow. The & suffix selects this type; CLNG performs explicit nearest-even rounding.",
        ),
        (
            "SINGLE",
            "```basic\nDIM value AS SINGLE\n```\nRice 32-bit floating-point type, default zero. Assignment rounds to single precision and rejects finite values that overflow to infinity. Existing NaN/infinity are retained. The ! suffix selects this type.",
        ),
        (
            "DOUBLE",
            "```basic\nDIM value AS DOUBLE\n```\nRice alias for NUMERIC, the 64-bit floating-point type. Defaults to zero; TYPEOF$ reports NUMERIC. The # suffix selects the same representation.",
        ),
        (
            "STRING",
            "```basic\nDIM text AS STRING\nDIM code AS STRING * 8\n```\nRice Unicode text. STRING accepts only strings and defaults to empty. STRING * n always contains n Unicode scalar characters, truncating or space-padding assignment and initializing to spaces. The $ suffix selects variable-length STRING.",
        ),
        (
            "COMMON",
            "```basic\nCOMMON [SHARED] var[()] [AS type][, var[()] [AS type] ...]\n```\nDeclares names shared with the root environment, with optional enforced types. Existing values are checked against AS declarations. Array element reads and writes use the shared root binding; empty array markers do not allocate dimensions. COMMON does not load modules or transfer state to another program.",
        ),
        (
            "INT64",
            "```basic\nDIM count AS INT64\ncount = CINT64(\"9223372036854775807\")\n```\nRice signed 64-bit integer, -9223372036854775808..9223372036854775807, default zero. Declarations enforce exact integral assignment and range checks. Use CINT64 with decimal text to enter values beyond floating-point precision. INTEGER and LONG provide signed 16-bit and 32-bit storage.",
        ),
        (
            "BOOL",
            "```basic\nDIM enabled AS BOOL\nenabled = TRUE()\n```\nNative Rice boolean; BOOLEAN is an alias. Defaults to false. Explicit declarations accept booleans or finite numbers (zero is false, nonzero is true), and reject strings and byte buffers. Comparisons of NUMERIC values return -1 for true or 0 for false.",
        ),
        (
            "BOOLEAN",
            "```basic\nDIM enabled AS BOOLEAN\n```\nAlias for the native Rice BOOL type. Defaults to false and enforces boolean or finite numeric assignment. TRUE(), FALSE(), and CBOOL(value) construct boolean values.",
        ),
        (
            "BYTE",
            "```basic\nDIM octet AS BYTE\noctet = CBYTE(255)\n```\nNative Rice unsigned 8-bit integer in 0..255, default zero. Explicit declarations enforce exact integral assignment and range checks. Use BYTES for a buffer of bytes.",
        ),
        (
            "BYTES",
            "```basic\nDIM payload AS BYTES\npayload = BYTES(0, 255)\n```\nRice binary buffer, distinct from Unicode STRING. Defaults to empty and accepts only byte buffers on assignment. Use explicit UTF-8 or hexadecimal functions to convert text.",
        ),
        // I/O
        (
            "PRINT",
            "```basic\nPRINT [expression][;|,] ...\n```\nDisplays output on the screen. Use `;` to suppress spacing, `,` for tab zones. Use `TAB(n)` and `SPC(n)` for positioning. Two adjacent quotes inside a string literal represent one quote: `PRINT \"a\"\"b\"` prints `a\"b`.",
        ),
        (
            "PRINT USING",
            "```basic\nPRINT USING format$; expr[, expr...]\n```\nFormatted output. `#` for digits, `.` for decimal, `$$` for currency, `^^^^` for scientific notation. Fixed-point fields preserve every INTEGER, LONG, INT64, and BYTE digit, including the signed 64-bit limits. Finite floating-point values retain their magnitude across the full range, including subnormal scientific notation and values larger than INT64. Scientific fields require exact conversion to floating point; use explicit CDBL if rounding a large integer is intended.",
        ),
        (
            "INPUT",
            "```basic\nINPUT [\"prompt\";] target[, target...]\nINPUT #channel, target[, target...]\nINPUT values(i, j), names$(i)\n```\nReads into scalar names or array elements; record fields and slices are unsupported. Indices are evaluated left to right once before input, retaining destinations across retries; INPUT i, values(i) uses the old i. Values are assigned only after all fields validate. Interactive INPUT retries invalid fields; file INPUT reads comma-delimited fields. Use doubled quotes inside a literal prompt to include a quote.",
        ),
        (
            "LINE INPUT",
            "```basic\nLINE INPUT [\"prompt\";] target\nLINE INPUT #channel, target\nLINE INPUT lines$(i)\n```\nReads a complete line into a scalar or array element, removing the line ending. Declared targets must accept a STRING; fixed strings pad or truncate. Record fields and slices are unsupported. The file form requires a string target. Literal prompts accept doubled quotes.",
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
            "```basic\nDIM [SHARED] var[(dims)] [AS type]\n```\nDeclares typed variables or arrays. All declarations enforce their type on assignment. Explicit AS overrides a name suffix; % is INTEGER, & LONG, ! SINGLE, # NUMERIC, and $ STRING. Without AS or a suffix, DIM declares NUMERIC regardless of DEFtype directives. Fixed STRING * n values pad or truncate to n Unicode characters. Existing array elements must convert to the declared type before a declaration succeeds. Recorded bounds are not enforced; array metadata is interpreter-wide.",
        ),
        (
            "CONST",
            "```basic\nCONST name [AS type] = expression[, name [AS type] = expression ...]\n```\nDefines constants left to right at runtime; later expressions may use earlier names. Explicit AS or a suffix selects a checked type; otherwise the value retains its expression type. Duplicate names raise an error. Earlier definitions and expression side effects survive a later failure.",
        ),
        (
            "SWAP",
            "```basic\nSWAP target1, target2\nSWAP values(i, j), replacement\n```\nExchanges scalar variables or array elements. Both destinations resolve from left to right with each index evaluated once, before reading or writing either value. Missing elements honor explicit array AS defaults, otherwise the ordinary suffix/DEFSTR defaults. A target error leaves both destinations unwritten, apart from index-expression side effects. Record fields and slices are unsupported; no bounds or rank checks are added.",
        ),
        (
            "OPTION BASE",
            "```basic\nOPTION BASE {0|1}\n```\nSets the default lower bound for subsequent array declarations. Rice defaults to 1; select 0 explicitly for zero-based arrays.",
        ),
        (
            "OPTION EXPLICIT",
            "```basic\nOPTION EXPLICIT\n```\nChecks declarations at runtime; DIM, SHARED, STATIC, CONST, parameters, suffixes, and DEFtype can declare names. Existing ancestor values and some statement paths bypass checks.",
        ),
        (
            "REDIM",
            "```basic\nREDIM [PRESERVE] var(dims) [AS type]\n```\nUpdates array bounds and element types. Without PRESERVE, clears stored elements and uses the explicit AS or suffix/DEFtype default. PRESERVE retains the declared element type when AS is omitted, validates retained values, and rejects a type change. Bounds remain unchecked for ordinary indexing; stored out-of-range elements may survive PRESERVE.",
        ),
        (
            "ERASE",
            "```basic\nERASE array[, array...]\n```\nClears local flattened array elements and sets each base variable to numeric zero. Bounds and declared array type metadata remain, so future untouched elements retain their defaults.",
        ),
        (
            "SHARED",
            "```basic\nSHARED var[()] [AS type][, var[()] [AS type] ...]\n```\nRoutes named variables and array element reads and writes to the root environment. Explicit AS enforces the shared value's type and validates existing values. Empty array markers do not allocate dimensions. A local declaration can shadow an inherited shared binding.",
        ),
        (
            "STATIC",
            "```basic\nSTATIC var[(dims)] [AS type][, var[(dims)] [AS type] ...]\n```\nPreserves scalar/record values and array elements, bounds, defaults, and declared types between procedure calls. All type annotations enforce assignment. Saved metadata remains available when a later call skips STATIC, including after CLEAR inside the procedure. Changes made during calls that skip STATIC are saved for subsequent calls.",
        ),
        (
            "CLEAR",
            "```basic\nCLEAR\n```\nClears variable values in the current environment and resets the DATA cursor. Constants, declarations, array metadata, procedures, static storage, and ordinary open files remain. Closes all SQLite connections and frees their results, rolling back uncommitted transactions.",
        ),
        // Control flow
        (
            "IF",
            "```basic\nIF condition THEN\n  ...\n[ELSEIF condition THEN\n  ...]\n[ELSE\n  ...]\nEND IF\n```\nConditional execution. Also supports single-line form: `IF cond THEN stmt [ELSE stmt]`. `IF cond THEN 100 ELSE 200` jumps to the selected numeric line label. Either branch may use a statement instead of a line number.",
        ),
        (
            "THEN",
            "```basic\nIF condition THEN statement [ELSE statement]\nIF condition THEN 100 [ELSE 200]\n```\nBegins the true branch of IF. A numeric branch target is shorthand for GOTO. A newline immediately after THEN begins a block IF.",
        ),
        (
            "ELSE",
            "```basic\nIF condition THEN statement ELSE statement\nIF condition THEN 100 ELSE 200\n```\nBegins the false branch of IF. Single-line numeric targets are shorthand for GOTO.",
        ),
        (
            "\\",
            "```basic\nPRINT 7 \\ 2\n```\nDivides signed 64-bit integers and truncates the quotient toward zero. When INTEGER, LONG, INT64, or BYTE participates, both operands must be exact integers and the result is INT64. NUMERIC operands round to the nearest integer, ties to even, and return NUMERIC. A zero divisor raises division by zero; out-of-range operands and quotient overflow raise overflow. Multiplication and / bind more tightly; MOD binds less tightly.",
        ),
        (
            "EQV",
            "```basic\nresult = a EQV b\n```\nComplements the bitwise XOR of numeric operands. When INTEGER, LONG, INT64, or BYTE participates, both operands must be exact integers and the result is INT64. NUMERIC operands use signed 64-bit casts that truncate fractions, saturate out-of-range values, and map NaN to zero, then return NUMERIC. With BOOL, evaluates logical equivalence; both operands are always evaluated. Binds less tightly than XOR and more tightly than IMP; repeated EQV associates left to right.",
        ),
        (
            "IMP",
            "```basic\nresult = a IMP b\n```\nCombines the bitwise complement of the left numeric operand with the right using OR. When INTEGER, LONG, INT64, or BYTE participates, both operands must be exact integers and the result is INT64. NUMERIC operands use signed 64-bit casts that truncate fractions, saturate out-of-range values, and map NaN to zero, then return NUMERIC. With BOOL, evaluates logical implication; both operands are always evaluated. Has the lowest operator precedence, below EQV; repeated IMP associates left to right.",
        ),
        (
            "LIKE",
            "```basic\nmatched = text$ LIKE pattern$\n```\nCase-sensitive whole-string wildcard matching. Supports * (any sequence), ? (one Unicode scalar), # (one ASCII digit), [abc], [a-z], and [!abc]. Use [*], [?], [#], or [[] for literal wildcard characters; ] is literal outside a class. Empty, unclosed, or reversed-range classes raise illegal function call. Both operands must be strings and are evaluated once, left to right. Comparison precedence; true is -1 and false is 0. No locale collation or Unicode normalization is applied.",
        ),
        (
            "FOR",
            "```basic\nFOR var = start TO end [STEP inc]\n  ...\nNEXT [var [, outer ...]]\n```\nCounted loop. Default STEP is 1. NEXT may close several nested FOR loops, listing their variables from innermost to outermost.",
        ),
        (
            "NEXT",
            "```basic\nNEXT\nNEXT i\nNEXT k, j, i\n```\nCloses one FOR loop when bare, or the named nested FOR loops from innermost to outermost. Each name must match the corresponding loop variable. A list cannot cross an unfinished IF, DO, WHILE, or procedure block.",
        ),
        (
            "WHILE",
            "```basic\nWHILE condition\n  ...\nWEND\n```\nLoop while condition is true. END WHILE is also accepted. EXIT WHILE leaves the nearest active WHILE loop in this procedure, without another condition check.",
        ),
        (
            "WEND",
            "```basic\nWHILE condition\n  ...\nWEND\n```\nEnds a WHILE loop.",
        ),
        (
            "DO",
            "```basic\nDO [{WHILE|UNTIL} condition]\n  ...\nLOOP [{WHILE|UNTIL} condition]\n```\nFlexible loop with condition at top or bottom.",
        ),
        (
            "SELECT",
            "```basic\nSELECT CASE expression\n  CASE value[, value...]\n    ...\n  CASE ELSE\n    ...\nEND SELECT\n```\nMulti-way branch based on expression value. EXIT SELECT leaves the nearest active SELECT CASE in this procedure, including from CASE ELSE.",
        ),
        (
            "EXIT",
            "```basic\nEXIT {FOR|DO|WHILE|SELECT|SUB|FUNCTION}\n```\nLeaves the matching construct. EXIT WHILE and EXIT SELECT propagate through intervening blocks, are consumed by the nearest matching active loop/selection, and cannot escape a SUB or FUNCTION. Unmatched uses raise a runtime error.",
        ),
        (
            "GOTO",
            "```basic\nGOTO label\n```\nTransfers execution to the specified label or line number.",
        ),
        (
            "GOSUB",
            "```basic\nGOSUB label\n...\nRETURN\n```\nCalls a label using the GOSUB return stack.",
        ),
        (
            "RETURN",
            "```basic\nRETURN [label_or_line]\n```\nEnds the most recent GOSUB, then continues after its call or jumps to the optional label in the caller's active scope. RETURN with a label still requires an active GOSUB and unwinds exactly one call. Procedure-local use cannot cross a SUB or FUNCTION boundary.",
        ),
        (
            "ON",
            "```basic\nON expr GOTO label1, label2\nON expr GOSUB label1, label2\nON ERROR GOTO handler\n```\nPerforms computed jumps/calls or installs an ON ERROR handler.",
        ),
        (
            "ON GOTO",
            "```basic\nON expr GOTO label1, label2[, label3...]\n```\nJumps to the label selected by the 1-based numeric expression.",
        ),
        (
            "ON GOSUB",
            "```basic\nON expr GOSUB label1, label2[, label3...]\n```\nCalls the label selected by the 1-based numeric expression.",
        ),
        (
            "ERROR",
            "```basic\nERROR code\n```\nRaises a Rice error code that can be trapped with `ON ERROR GOTO`.",
        ),
        (
            "RESUME",
            "```basic\nRESUME\nRESUME NEXT\nRESUME label\n```\nResumes execution after an `ON ERROR GOTO` handler. Resume is exact at top-level scope.",
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
            "```basic\nFUNCTION name(params) [AS type] [STATIC]\n  name = return_value\nEND FUNCTION\n```\nReturns the value assigned to its exact name. Each call creates a fresh result with its declared type, and all result declarations enforce conversion. Explicit AS overrides the suffix. Without either, an unsuffixed result starts at NUMERIC zero and remains dynamic; DEFtype does not select function results. Fixed STRING * n results start padded to n characters. Record fields may be set directly through the function name.",
        ),
        (
            "CALL",
            "```basic\nCALL name(args)\n```\nCalls a SUB with the given arguments. DB... database builtins also support CALL, which discards their result and uses value arguments without BYREF copy-back. Other expression functions cannot be called this way.",
        ),
        (
            "DECLARE",
            "```basic\nDECLARE SUB name (params)\nDECLARE FUNCTION name (params) [AS type]\n```\nDeclares a procedure signature. Before execution, Rice validates matching definitions for procedure kind, parameter count, BYVAL/BYREF modes, array markers, parameter types, and function result type. A missing definition may be supplied in a later REPL submission. Parameter names may differ; NUMERIC and DOUBLE are aliases. DECLARE does not link external code.",
        ),
        (
            "BYVAL",
            "```basic\nSUB name (BYVAL x AS NUMERIC)\n```\nPasses a parameter by value. Use BYREF for copy-in/copy-out parameters.",
        ),
        (
            "BYREF",
            "```basic\nSUB name (BYREF x AS NUMERIC)\nCALL name(values(i))\n```\nCopies a value in and writes it back on exit for a scalar variable or array element argument, including record-valued elements. Array indices are evaluated once when the argument is read. Constants, function results, fields, parentheses, and other expressions are not written back. This is copy-in/copy-out, not a live reference; later aliased parameters write back last, including when the procedure errors. BYREF is the default. Array parameters declared with () still do not bind whole arrays.",
        ),
        (
            "DEF",
            "```basic\nDEF FNname[(params)] = expression\n```\nAlso accepts a multiline body ending in END DEF. FN is a convention, not a required prefix.",
        ),
        (
            "DEFSTR",
            "```basic\nDEFSTR A-Z\n```\nSets STRING as the default type for subsequently declared or auto-created unsuffixed names with the listed initials. Explicit AS or a suffix takes precedence; existing declarations retain their type.",
        ),
        (
            "DEFINT",
            "```basic\nDEFINT A-Z\n```\nSets signed 16-bit INTEGER as the default type for subsequently declared or auto-created unsuffixed names with the listed initials. Explicit AS or a suffix takes precedence; existing declarations retain their type.",
        ),
        (
            "DEFLNG",
            "```basic\nDEFLNG A-Z\n```\nSets signed 32-bit LONG as the default type for subsequently declared or auto-created unsuffixed names with the listed initials. Explicit AS or a suffix takes precedence; existing declarations retain their type.",
        ),
        (
            "DEFSNG",
            "```basic\nDEFSNG A-Z\n```\nSets 32-bit SINGLE as the default type for subsequently declared or auto-created unsuffixed names with the listed initials. Explicit AS or a suffix takes precedence; existing declarations retain their type.",
        ),
        (
            "DEFDBL",
            "```basic\nDEFDBL A-Z\n```\nSets NUMERIC/DOUBLE (64-bit floating point) as the default type for subsequently declared or auto-created unsuffixed names with the listed initials. Explicit AS or a suffix takes precedence; existing declarations retain their type.",
        ),
        // Data
        (
            "DATA",
            "```basic\nDATA value[, value...]\nDATA +42, -3, , \"She said \"\"hi\"\"\",\n```\nDefines literal data. Numeric literals accept one leading + or -; expressions are not allowed. Omitted fields between commas or at either end are empty items; DATA , supplies two, while bare DATA supplies none. READ converts an empty item to zero or an empty string according to the target's current/default type; MAT READ uses zero. Doubled quotes inside a string literal represent one quote.",
        ),
        (
            "READ",
            "```basic\nREAD target[, target...]\nREAD values(i, j), names$(i)\n```\nReads DATA into scalar or array element destinations, applying declared type conversion. Integer targets require exact in-range values; fixed strings pad or truncate. Targets resolve left to right, so READ i, values(i) uses the new i. Empty fields supply the typed default; record targets reject an empty item without consuming it. Earlier assignments persist after a later failure; fields and slices are unsupported.",
        ),
        (
            "RESTORE",
            "```basic\nRESTORE [label]\n```\nResets the DATA pointer to the beginning or to a label attached directly to DATA. Unknown DATA labels silently rewind.",
        ),
        // User-defined types
        (
            "TYPE",
            "```basic\nTYPE name\n  field AS type\n  ...\nEND TYPE\n```\nDefines a Rice record with nested nonrecursive records. All fields enforce their declared type. STRING * n fields pad or truncate to n Unicode characters in memory. Record assignments require the same declared type. Field declarations also define binary serialization widths.",
        ),
        // Error handling
        (
            "WHEN EXCEPTION IN",
            "```basic\nWHEN EXCEPTION IN\n  ...\nUSE\n  PRINT EXTYPE; EXTEXT$\nEND WHEN\n```\nStructured exception handling. Code in the guarded block is protected; the USE block handles errors. Local GOTO targets resolve within the protected body or USE handler before propagating outward.",
        ),
        (
            "RETRY",
            "```basic\nRETRY\n```\nRestarts the original guarded block from the beginning after an error, including statements before any earlier CONTINUE.",
        ),
        (
            "CONTINUE",
            "```basic\nCONTINUE\n```\nResumes the protected body after the failed direct statement, under the same handler. If a nested construct failed, skips that whole direct construct.",
        ),
        // File I/O
        (
            "OPEN",
            "```basic\nOPEN #n: NAME file$, ACCESS {INPUT|OUTPUT|OUTIN}, ORGANIZATION {SEQUENTIAL|STREAM}\nOPEN file$ FOR {INPUT|OUTPUT|APPEND|BINARY|RANDOM} AS #n\n```\nBoth forms are accepted. GET/PUT use typed binary serialization, with raw transfers for BYTES buffers.",
        ),
        ("CLOSE", "```basic\nCLOSE #n\n```\nCloses an open file."),
        (
            "GET",
            "```basic\nGET #channel [, position [, target]]\nGET #1, , records(i)\n```\nReads a scalar or array element using its declared binary layout: INTEGER 2 bytes, LONG/SINGLE 4, NUMERIC/DOUBLE/INT64 8, BOOL/BYTE 1, all little-endian. Explicit AS overrides suffixes. Records serialize fields recursively; BYTES buffers read their current length as raw bytes. Channel, position, and indices evaluate once; target validation occurs before reading. Members, slices, and whole arrays are unsupported. An omitted operand fills a FIELD record, padding only bytes beyond actual end-of-file with spaces.",
        ),
        (
            "PUT",
            "```basic\nPUT #channel [, position [, source]]\nPUT #1, , values%(i, j)\n```\nWrites a scalar or array element using its declared binary layout; explicit AS overrides a name suffix. INTEGER uses 2 bytes, LONG/SINGLE 4, NUMERIC/DOUBLE/INT64 8, BOOL/BYTE 1, all little-endian. Records serialize fields recursively; top-level BYTES writes raw bytes. Channel, position, and indices evaluate once. Constant sources are permitted; members, slices, and whole arrays are unsupported. An omitted operand uses FIELD buffers.",
        ),
        (
            "SET POINTER",
            "```basic\nSET #n: POINTER position\n```\nFlushes pending output before setting the file position for the next read or write. Flush failure prevents the requested seek.",
        ),
        (
            "ASK POINTER",
            "```basic\nASK #n: POINTER var\n```\nQueries the current file position into a variable.",
        ),
        (
            "FIELD",
            "```basic\nFIELD #n, width AS stringVar[, width AS stringVar ...]\n```\nMaps string variables to byte slices of a channel record buffer. Names may use $ or a STRING declaration. GET fills a complete record across buffered reads; only actual end-of-file leaves space padding. RANDOM mode is not enforced; ordinary assignment does not update the buffer.",
        ),
        (
            "LSET",
            "```basic\nLSET var$ = expr$\n```\nLeft-aligns into a FIELD slot or the existing string value's current byte width, padding/truncating as needed. A missing variable-length string uses the replacement width; an existing empty string stays empty. Fixed strings retain their declared character width.",
        ),
        (
            "RSET",
            "```basic\nRSET var$ = expr$\n```\nRight-aligns into a FIELD slot or the existing string value's current byte width, padding/truncating as needed. A missing variable-length string uses the replacement width; an existing empty string stays empty. Fixed strings retain their declared character width.",
        ),
        // File system
        ("NAME", "```basic\nNAME old$ AS new$\n```\nRenames a file."),
        ("KILL", "```basic\nKILL file$\n```\nDeletes a file."),
        ("MKDIR", "```basic\nMKDIR dir$\n```\nCreates a directory."),
        ("RMDIR", "```basic\nRMDIR dir$\n```\nRemoves a directory."),
        (
            "RESET",
            "```basic\nRESET\n```\nFlushes and closes all open files. Flush errors propagate and leave handles available for retry.",
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
            "```basic\nMAT C = A + B\nMAT C = A * B\nMAT B = INV(A)\nMAT B = TRN(A)\nMAT A = ZER\nMAT A = CON\nMAT A = IDN\nMAT READ A\nMAT PRINT A\nMAT PRINT #1, A\nMAT INPUT #2, A\nPRINT DET(A)\n```\nTwo-dimensional numeric matrix operations. Declare destination bounds to match the result; MAT does not resize metadata. DET(A) computes a square matrix determinant without inversion. MAT READ converts empty DATA fields to zero. File MAT PRINT writes comma-separated rows; file MAT INPUT reads numeric fields in row-major order without console prompts. Each channel expression is evaluated once. Input replaces the matrix only after all fields succeed; errors leave consumed fields consumed. Console MAT PRINT uses spaces. Small matrices support bounds across the signed 64-bit index range; unrepresentable dimensions or destination indices raise overflow.",
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
            // A type keyword such as STRING takes precedence over an omitted-$ alias.
            if KEYWORD_HOVER_DOCS.contains_key(name) {
                return None;
            }
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
        rice::token::Token::KwLike => Some("LIKE".into()),
        rice::token::Token::KwMod => Some("MOD".into()),
        rice::token::Token::KwEndSub => Some("END SUB".into()),
        rice::token::Token::KwEndFunction => Some("END FUNCTION".into()),
        rice::token::Token::KwEndSelect => Some("END SELECT".into()),
        rice::token::Token::KwEndType => Some("END TYPE".into()),
        rice::token::Token::KwEndWhile => Some("END WHILE".into()),
        rice::token::Token::KwEndWhen => Some("END WHEN".into()),
        rice::token::Token::KwShared => Some("SHARED".into()),
        rice::token::Token::KwCommon => Some("COMMON".into()),
        rice::token::Token::KwInteger => Some("INTEGER".into()),
        rice::token::Token::KwLong => Some("LONG".into()),
        rice::token::Token::KwSingle => Some("SINGLE".into()),
        rice::token::Token::KwDouble => Some("DOUBLE".into()),
        rice::token::Token::KwString => Some("STRING".into()),
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
    fn native_types_have_completion_hover_and_typed_signatures() {
        for (name, canonical) in [
            ("NUMERIC", "NUMERIC"),
            ("INTEGER", "INTEGER"),
            ("LONG", "LONG"),
            ("SINGLE", "SINGLE"),
            ("DOUBLE", "DOUBLE"),
            ("STRING", "STRING"),
            ("INT64", "INT64"),
            ("BOOL", "BOOL"),
            ("BOOLEAN", "BOOL"),
            ("BYTE", "BYTE"),
            ("BYTES", "BYTES"),
        ] {
            let state = analyze_source(
                format!(
                    "DIM item AS {name}\nFUNCTION Typed(value AS {name}) AS {name}\nEND FUNCTION\n"
                ),
                1,
            );
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            assert_eq!(
                resolve_token_name(&state, Position::new(0, 12)).as_deref(),
                Some(name)
            );
            assert_eq!(
                TYPE_COMPLETIONS
                    .iter()
                    .find(|item| item.label == name)
                    .unwrap()
                    .kind,
                Some(CompletionItemKind::TYPE_PARAMETER)
            );
            let hover = builtin_hover(name).or_else(|| keyword_hover(name)).unwrap();
            assert!(hover.contains("Rice"), "{name}");
            assert!(hover.contains(&format!("AS {name}")), "{name}");
            let signature = format!("FUNCTION TYPED(VALUE AS {canonical}) AS {canonical}");
            assert_eq!(
                find_symbol(&state.symbols, "TYPED")
                    .unwrap()
                    .detail
                    .as_deref(),
                Some(signature.as_str())
            );
        }
        assert!(keyword_hover("DIM").unwrap().contains("enforce"));
        assert!(keyword_hover("FUNCTION").unwrap().contains("enforce"));
        assert!(keyword_hover("TYPE").unwrap().contains("enforce"));
    }

    #[test]
    fn native_value_builtins_have_completion_and_hover() {
        for (name, call, expected) in [
            ("CINT64", "CINT64(\"9223372036854775807\")", "signed 64-bit"),
            ("CBOOL", "CBOOL(1)", "boolean"),
            ("CBYTE", "CBYTE(255)", "0..255"),
            ("TRUE", "TRUE()", "BOOL"),
            ("FALSE", "FALSE()", "BOOL"),
            ("TYPEOF$", "TYPEOF$(1)", "runtime type"),
            ("BYTES", "BYTES(0, 255)", "binary buffer"),
            ("BYTEBUFFER", "BYTEBUFFER(3, 255)", "length"),
            ("BYTELEN", "BYTELEN(BYTES())", "number of bytes"),
            ("BYTEAT", "BYTEAT(BYTES(255), 1)", "1-based"),
            ("BYTESET", "BYTESET(BYTES(0), 1, 255)", "copy"),
            ("BYTESLICE", "BYTESLICE(BYTES(1, 2), 1, 1)", "1-based"),
            ("UTF8ENCODE", "UTF8ENCODE(\"é😀\")", "UTF-8"),
            ("UTF8DECODE$", "UTF8DECODE$(BYTES())", "Invalid UTF-8"),
            ("HEXENCODE$", "HEXENCODE$(BYTES(255))", "uppercase"),
            ("HEXDECODE", "HEXDECODE(\"00ff\")", "even number"),
        ] {
            let state = analyze_source(format!("PRINT {call}\n"), 1);
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            assert_eq!(
                resolve_token_name(&state, Position::new(0, 6)).as_deref(),
                Some(name)
            );
            assert!(builtin_hover(name).unwrap().contains(expected), "{name}");
            assert_eq!(
                BUILTIN_COMPLETIONS
                    .iter()
                    .find(|item| item.label == name)
                    .unwrap()
                    .kind,
                Some(CompletionItemKind::FUNCTION)
            );
        }
        assert!(
            builtin_hover("LEN")
                .unwrap()
                .contains("For BYTES, counts bytes")
        );
    }

    #[test]
    fn sqlite_functions_have_completion_hover_and_call_syntax() {
        for (name, call, expected) in [
            ("DBOPEN", "DBOPEN(\":memory:\")", "connection handle"),
            ("DBCLOSE", "DBCLOSE(db)", "rolls back"),
            (
                "DBEXEC",
                "DBEXEC(db, \"DELETE FROM sample WHERE id = ?\", 1)",
                "exactly one",
            ),
            (
                "DBQUERY",
                "DBQUERY(db, \"SELECT ?\", NULL())",
                "materialized",
            ),
            ("DBNEXT", "DBNEXT(rows)", "before the first row"),
            ("DBVALUE", "DBVALUE(rows, \"name\")", "1-based"),
            ("DBISNULL", "DBISNULL(rows, 1)", "SQLite NULL"),
            ("DBCOLUMNCOUNT", "DBCOLUMNCOUNT(rows)", "empty result"),
            ("DBCOLUMNNAME$", "DBCOLUMNNAME$(rows, 1)", "OPTION BASE"),
            ("DBCOLUMNTYPE$", "DBCOLUMNTYPE$(rows, 1)", "storage class"),
            ("DBROWCOUNT", "DBROWCOUNT(rows)", "materialized rows"),
            ("DBFREE", "DBFREE(rows)", "invalidates"),
            ("DBCHANGES", "DBCHANGES(db)", "Non-DML"),
            ("DBLASTID", "DBLASTID(db)", "exact INT64"),
            ("DBBEGIN", "DBBEGIN(db, \"IMMEDIATE\")", "DEFERRED"),
            ("DBCOMMIT", "DBCOMMIT(db)", "retry or rollback"),
            ("DBROLLBACK", "DBROLLBACK(db)", "do not automatically"),
            ("DBINTRANSACTION", "DBINTRANSACTION(db)", "autocommit"),
            ("DBTIMEOUT", "DBTIMEOUT(db, 0)", "2147483647"),
            ("DBVERSION$", "DBVERSION$()", "Parentheses are required"),
        ] {
            let state = analyze_source(format!("PRINT {call}\nCALL {call}\n"), 1);
            assert!(
                state.diagnostics.is_empty(),
                "{name}: {:?}",
                state.diagnostics
            );
            assert_eq!(
                resolve_token_name(&state, Position::new(0, 6)).as_deref(),
                Some(name)
            );
            assert_eq!(
                resolve_token_name(&state, Position::new(1, 5)).as_deref(),
                Some(name)
            );
            assert!(builtin_hover(name).unwrap().contains(expected), "{name}");
            assert_eq!(
                BUILTIN_COMPLETIONS
                    .iter()
                    .find(|item| item.label == name)
                    .unwrap()
                    .kind,
                Some(CompletionItemKind::FUNCTION)
            );
        }
        assert!(keyword_hover("CALL").unwrap().contains("without BYREF"));
        assert!(
            keyword_hover("CLEAR")
                .unwrap()
                .contains("Closes all SQLite")
        );
    }

    #[test]
    fn null_functions_have_completion_and_hover_without_a_declaration_type() {
        for (name, call) in [("NULL", "NULL()"), ("ISNULL", "ISNULL(NULL())")] {
            let state = analyze_source(format!("PRINT {call}\n"), 1);
            assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
            assert_eq!(
                resolve_token_name(&state, Position::new(0, 6)).as_deref(),
                Some(name)
            );
            assert!(BUILTIN_COMPLETIONS.iter().any(|item| item.label == name));
            assert!(builtin_hover(name).unwrap().contains("NULL"));
        }
        assert!(builtin_hover("NULL").unwrap().contains("no AS NULL"));
        assert!(builtin_hover("TYPEOF$").unwrap().contains("STRING, NULL"));
        assert!(!TYPE_COMPLETIONS.iter().any(|item| item.label == "NULL"));
    }

    #[test]
    fn const_lists_register_every_symbol() {
        let source = "CONST First = 1, Second = First + 1, Title$ = \"ok\"\nSUB Worker\nCONST LocalOne = 2, LocalTwo = LocalOne + 1\nEND SUB\nPRINT Second; Title$\n".to_string();
        let state = analyze_source(source, 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        assert_eq!(state.symbols.constants.len(), 5);
        for (name, line) in [
            ("FIRST", 1),
            ("SECOND", 1),
            ("TITLE$", 1),
            ("LOCALONE", 3),
            ("LOCALTWO", 3),
        ] {
            let symbol = find_symbol(&state.symbols, name).unwrap();
            assert_eq!(symbol.line, line);
            let expected = if name == "TITLE$" {
                "CONST TITLE$ AS STRING".to_owned()
            } else {
                format!("CONST {name}")
            };
            assert_eq!(symbol.detail.as_deref(), Some(expected.as_str()));
        }
        let state = analyze_source("CONST first = 1,\n".into(), 1);
        assert!(!state.diagnostics.is_empty());
    }

    #[test]
    fn typed_constants_scope_declarations_and_static_variables_keep_symbol_types() {
        let source = "CONST limit AS INTEGER = 4, caption AS STRING * 3 = \"hi\"\nCOMMON SHARED total AS LONG, samples() AS SINGLE\nSUB Worker\nSHARED sharedValue AS DOUBLE\nSTATIC calls AS INTEGER, cache AS STRING * 4\nEND SUB\nDIM count%\nDIM SHARED result(2) AS LONG\n";
        let state = analyze_source(source.into(), 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        for (name, line, detail) in [
            ("LIMIT", 1, "CONST LIMIT AS INTEGER"),
            ("CAPTION", 1, "CONST CAPTION AS STRING * 3"),
            ("TOTAL", 2, "COMMON SHARED TOTAL AS LONG"),
            ("SAMPLES", 2, "COMMON SHARED SAMPLES() AS SINGLE"),
            ("SHAREDVALUE", 4, "SHARED SHAREDVALUE AS DOUBLE"),
            ("CALLS", 5, "STATIC CALLS AS INTEGER"),
            ("CACHE", 5, "STATIC CACHE AS STRING * 4"),
            ("COUNT%", 7, "DIM COUNT% AS INTEGER"),
            ("RESULT", 8, "DIM SHARED RESULT() AS LONG"),
        ] {
            let symbol = find_symbol(&state.symbols, name).unwrap();
            assert_eq!(symbol.line, line, "{name}");
            assert_eq!(symbol.detail.as_deref(), Some(detail), "{name}");
        }
        assert!(
            KEYWORD_COMPLETIONS
                .iter()
                .any(|item| item.label == "COMMON")
        );
        assert!(keyword_hover("COMMON").unwrap().contains("AS type"));
        assert_eq!(
            resolve_token_name(&state, Position::new(1, 0)).as_deref(),
            Some("COMMON")
        );
        let upgraded = analyze_source("count = 1\nDIM count AS LONG\n".into(), 1);
        assert_eq!(
            find_symbol(&upgraded.symbols, "COUNT")
                .unwrap()
                .detail
                .as_deref(),
            Some("DIM COUNT AS LONG")
        );
    }

    #[test]
    fn declared_procedures_preserve_typed_signatures_and_definitions_take_precedence() {
        let state = analyze_source("DECLARE FUNCTION Convert(BYVAL item AS INTEGER) AS LONG\nDECLARE SUB Update(value AS STRING * 4)\n".into(), 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        assert_eq!(
            find_symbol(&state.symbols, "CONVERT")
                .unwrap()
                .detail
                .as_deref(),
            Some("DECLARE FUNCTION CONVERT(BYVAL ITEM AS INTEGER) AS LONG")
        );
        assert_eq!(
            find_symbol(&state.symbols, "UPDATE")
                .unwrap()
                .detail
                .as_deref(),
            Some("DECLARE SUB UPDATE(VALUE AS STRING * 4)")
        );
        let state = analyze_source("DECLARE FUNCTION Convert(BYVAL n AS INTEGER) AS LONG\nFUNCTION Convert(BYVAL n AS INTEGER) AS LONG\nEND FUNCTION\n".into(), 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        let symbol = find_symbol(&state.symbols, "CONVERT").unwrap();
        assert_eq!(symbol.line, 2);
        assert_eq!(
            symbol.detail.as_deref(),
            Some("FUNCTION CONVERT(BYVAL N AS INTEGER) AS LONG")
        );
        assert_eq!(state.symbols.functions.len(), 1);
        assert!(keyword_hover("DECLARE").unwrap().contains("validates"));
    }

    #[test]
    fn typed_conversion_and_assignment_docs_describe_enforcement() {
        for (name, expected) in [
            ("CINT", "16-bit INTEGER"),
            ("CLNG", "32-bit LONG"),
            ("CSNG", "32-bit SINGLE"),
            ("TYPEOF$", "DOUBLE shares"),
            ("CVI", "16-bit INTEGER"),
            ("CVL", "32-bit LONG"),
            ("CVS", "32-bit SINGLE"),
        ] {
            assert!(builtin_hover(name).unwrap().contains(expected), "{name}");
        }
        for (name, expected) in [
            ("DIM", "All declarations enforce"),
            ("TYPE", "All fields enforce"),
            ("STRING", "Unicode scalar"),
            ("GET", "Explicit AS overrides"),
            ("PUT", "explicit AS overrides"),
            ("DEFSNG", "32-bit SINGLE"),
        ] {
            assert!(keyword_hover(name).unwrap().contains(expected), "{name}");
        }
    }

    #[test]
    fn indexed_swap_registers_targets_and_preserves_index_references() {
        let state = analyze_source(
            "DIM index\nSWAP first(index, 2), second$(index)\n".to_string(),
            1,
        );
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        for (column, name, definition_line) in
            [(5, "FIRST", 2), (11, "INDEX", 1), (22, "SECOND$", 2)]
        {
            assert_eq!(
                resolve_token_name(&state, Position::new(1, column)).as_deref(),
                Some(name)
            );
            assert_eq!(
                find_symbol(&state.symbols, name).unwrap().line,
                definition_line
            );
        }
        assert_eq!(state.symbols.variables.len(), 3);
        for invalid in ["SWAP a(), b", "SWAP a(1:2), b", "SWAP a.x, b"] {
            let state = analyze_source(format!("{invalid}\n"), 1);
            assert_eq!(state.diagnostics.len(), 1, "{invalid}");
            assert_eq!(state.diagnostics[0].range.start.line, 0);
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
        let state = analyze_source("DATA +2, -3, , \"😀\",\nREAD n, m, s$\n".to_string(), 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        for data in ["DATA +", "DATA 1 + 2", "DATA --1"] {
            let state = analyze_source(format!("{data}\n"), 1);
            assert_eq!(state.diagnostics.len(), 1, "{data}");
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
        let state = analyze_source(format!("{line}\n"), 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        for (name, expected) in [
            ("LEN", "record_expression"),
            ("DET", "two-dimensional MAT shape"),
        ] {
            let column = line[..line.find(name).unwrap()].encode_utf16().count() as u32;
            assert_eq!(
                resolve_token_name(&state, Position::new(0, column)).as_deref(),
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
            let comment_column = line[..line.rfind(name).unwrap()].encode_utf16().count() as u32;
            assert_eq!(
                resolve_token_name(&state, Position::new(0, comment_column)),
                None
            );
        }
    }

    #[test]
    fn byref_element_arguments_preserve_array_and_index_references() {
        let state = analyze_source(
            "DIM values(2), index\nSUB Update(BYREF item)\nitem = 3\nEND SUB\nCALL Update(values(index))\n".to_string(),
            1,
        );
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        for (column, name) in [(12, "VALUES"), (19, "INDEX")] {
            assert_eq!(
                resolve_token_name(&state, Position::new(4, column)).as_deref(),
                Some(name)
            );
            assert_eq!(find_symbol(&state.symbols, name).unwrap().line, 1);
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
            assert!(
                keyword_hover(branch)
                    .unwrap()
                    .contains("shorthand for GOTO")
            );
        }
        assert!(keyword_hover("IF").unwrap().contains("THEN 100 ELSE 200"));
    }

    #[test]
    fn min_max_have_completions_and_hover() {
        let source = "PRINT MIN(3, MAX(1, 2))\n".to_string();
        let state = analyze_source(source, 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        for (expected_name, character) in [("MIN", 6), ("MAX", 13)] {
            let name = resolve_token_name(&state, Position::new(0, character)).unwrap();
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

    #[test]
    fn removed_dialect_directives_have_diagnostics_and_no_completion() {
        for name in ["QB", "ANSI", "QBasic 1.1", "unknown"] {
            let state = analyze_source(format!("OPTION DIALECT \"{name}\"\nPRINT 42\n"), 1);
            assert_eq!(state.diagnostics.len(), 1);
            assert_eq!(
                state.diagnostics[0].severity,
                Some(DiagnosticSeverity::ERROR)
            );
            assert_eq!(state.diagnostics[0].range.start.line, 0);
            assert!(state.diagnostics[0].message.contains("OPTION"));
        }
        assert!(keyword_hover("OPTION DIALECT").is_none());
        assert!(
            !KEYWORD_COMPLETIONS
                .iter()
                .any(|item| item.label == "OPTION DIALECT")
        );
    }

    #[test]
    fn indexed_input_targets_and_subscripts_resolve_to_symbols() {
        let source = "DIM index\nREAD table(index, 2)\nINPUT values(index)\nINPUT #1, filevalues$(index), count\nLINE INPUT lines$(index)\nLINE INPUT #1, filelines$(index)\n".to_string();
        let state = analyze_source(source.clone(), 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        for (name, expected_line) in [
            ("TABLE", 2),
            ("VALUES", 3),
            ("FILEVALUES$", 4),
            ("COUNT", 4),
            ("LINES$", 5),
            ("FILELINES$", 6),
        ] {
            assert_eq!(
                find_symbol(&state.symbols, name).unwrap().line,
                expected_line
            );
        }
        for (line_number, line) in source.lines().enumerate().skip(1) {
            let character = line.find("(index").unwrap() + 1;
            let name =
                resolve_token_name(&state, Position::new(line_number as u32, character as u32))
                    .unwrap();
            assert_eq!(name, "INDEX");
            assert_eq!(find_symbol(&state.symbols, &name).unwrap().line, 1);
        }
        assert_eq!(state.symbols.variables.len(), 7);
        for keyword in ["READ", "INPUT", "LINE INPUT"] {
            assert!(keyword_hover(keyword).unwrap().contains("array element"));
            assert!(KEYWORD_COMPLETIONS.iter().any(|item| item.label == keyword));
        }
    }

    #[test]
    fn eqv_imp_have_hover_and_completions() {
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
        }
    }

    #[test]
    fn doubled_quotes_preserve_unicode_cursor_and_comment_boundaries() {
        let source = r#"PRINT "é ""' REM 😀"""; marker ' 日本語"#;
        let state = analyze_source(format!("{source}\n"), 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        let character =
            |text: &str| source[..source.find(text).unwrap()].encode_utf16().count() as u32;
        assert_eq!(
            resolve_token_name(&state, Position::new(0, character("marker"))).as_deref(),
            Some("MARKER")
        );
        for text in ["REM", "😀", "日本語"] {
            assert_eq!(
                resolve_token_name(&state, Position::new(0, character(text))),
                None
            );
        }
        assert!(state.tokens.iter().any(|token| matches!(
            &token.token,
            rice::token::Token::StringLiteral(value) if value == "é \"' REM 😀\""
        )));
    }

    #[test]
    fn removed_directive_text_is_valid_inside_strings() {
        let state = analyze_source(
            "PRINT \"OPTION DIALECT \"\"ANSI\"\"\"\nPRINT 1 EQV 1\n".into(),
            1,
        );
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
    }

    #[test]
    fn next_lists_preserve_counter_references_and_comma_spans() {
        let source =
            "FOR outer = 1 TO 2\nFOR inner = 1 TO 2\nPRINT inner\nNEXT inner, outer\n".to_string();
        let state = analyze_source(source, 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        for (column, name, definition_line) in [(5, "INNER", 2), (12, "OUTER", 1)] {
            assert_eq!(
                resolve_token_name(&state, Position::new(3, column)).as_deref(),
                Some(name)
            );
            assert_eq!(
                find_symbol(&state.symbols, name).unwrap().line,
                definition_line
            );
        }
        assert_eq!(resolve_token_name(&state, Position::new(3, 10)), None);
        assert!(state.tokens.iter().any(|token| {
            token.span.line == 4 && matches!(token.token, rice::token::Token::Comma)
        }));
        assert!(
            keyword_hover("NEXT")
                .unwrap()
                .contains("innermost to outermost")
        );

        let invalid = analyze_source(
            "FOR outer = 1 TO 2\nFOR inner = 1 TO 2\nNEXT outer, inner\n".to_string(),
            1,
        );
        assert_eq!(invalid.diagnostics.len(), 1);
        assert_eq!(invalid.diagnostics[0].range.start.line, 2);
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
    }

    #[test]
    fn completion_and_hover_cover_language_completeness_additions() {
        let source = "DIM values(2)\nWHILE 1\nSELECT CASE 1\nCASE 1\nGET #1, , values(1)\nPUT #1, , values(2)\nEXIT SELECT\nEND SELECT\nEXIT WHILE\nWEND\nFUNCTION TextValue() AS STRING\nEND FUNCTION\nPRINT VAL(\"1D2tail\"); ENVIRON$(1)\n".to_string();
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
            resolve_token_name(&state, Position::new(2, 7)).as_deref(),
            Some("CASE")
        );
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

    #[test]
    fn string_comparison_and_patterns_have_editor_support() {
        let source = "PRINT \"é\" LIKE \"?\"\nPRINT STRCOMP(\"É\", \"é\", 1)\n".to_string();
        let state = analyze_source(source, 1);
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        assert_eq!(
            resolve_token_name(&state, Position::new(0, 11)).as_deref(),
            Some("LIKE")
        );
        assert_eq!(
            resolve_token_name(&state, Position::new(1, 8)).as_deref(),
            Some("STRCOMP")
        );
        let like = KEYWORD_COMPLETIONS
            .iter()
            .find(|item| item.label == "LIKE")
            .unwrap();
        assert_eq!(like.kind, Some(CompletionItemKind::OPERATOR));
        assert!(keyword_hover("LIKE").unwrap().contains("whole-string"));
        assert!(
            BUILTIN_COMPLETIONS
                .iter()
                .any(|item| item.label == "STRCOMP")
        );
        assert!(
            builtin_hover("STRCOMP")
                .unwrap()
                .contains("exactly numeric 0 or 1")
        );
    }
}
