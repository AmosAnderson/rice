use std::borrow::Cow;
use std::collections::BTreeMap;

use rustyline::Editor;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;

use crate::interpreter::Interpreter;
use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::token::Token;

// 24-bit ANSI color codes (VS Code dark theme inspired)
const COLOR_KEYWORD: &str = "\x1b[38;2;86;156;214m";
const COLOR_STRING: &str = "\x1b[38;2;206;145;120m";
const COLOR_NUMBER: &str = "\x1b[38;2;181;206;168m";
const COLOR_IDENT: &str = "\x1b[38;2;156;220;254m";
const COLOR_OPERATOR: &str = "\x1b[38;2;212;212;212m";
const COLOR_COMMENT: &str = "\x1b[38;2;106;153;85m";
const COLOR_RESET: &str = "\x1b[0m";

struct BasicHelper {
    dialect: crate::Dialect,
}

impl rustyline::Helper for BasicHelper {}
impl rustyline::completion::Completer for BasicHelper {
    type Candidate = String;
}
impl rustyline::hint::Hinter for BasicHelper {
    type Hint = String;
}
impl rustyline::validate::Validator for BasicHelper {}

impl Highlighter for BasicHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        if line.is_empty() {
            return Cow::Borrowed(line);
        }
        Cow::Owned(highlight_line(line, self.dialect))
    }

    fn highlight_char(
        &self,
        _line: &str,
        _pos: usize,
        _forced: rustyline::highlight::CmdKind,
    ) -> bool {
        true
    }
}

/// Find the position of an unquoted comment marker (' or REM) in the line.
/// Returns the byte offset where the comment starts, or None.
fn find_comment_start(line: &str) -> Option<usize> {
    let mut in_string = false;
    let bytes = line.as_bytes();
    for (i, ch) in line.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '\'' if !in_string => return Some(i),
            _ if !in_string => {
                let is_identifier_byte = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
                if (i == 0 || !is_identifier_byte(bytes[i - 1]))
                    && bytes
                        .get(i..i + 3)
                        .is_some_and(|word| word.eq_ignore_ascii_case(b"REM"))
                    && bytes.get(i + 3).is_none_or(|&b| {
                        !is_identifier_byte(b) && !matches!(b, b'$' | b'%' | b'!' | b'#' | b'&')
                    })
                {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn highlight_line(line: &str, dialect: crate::Dialect) -> String {
    if line.contains('\n') {
        return line
            .split('\n')
            .map(|part| highlight_line(part, dialect))
            .collect::<Vec<_>>()
            .join("\n");
    }
    let comment_start = find_comment_start(line);
    let (code_part, comment_part) = match comment_start {
        Some(pos) => (&line[..pos], Some(&line[pos..])),
        None => (line, None),
    };

    let mut result = String::with_capacity(line.len() * 3);

    if !code_part.is_empty() {
        let tokens = match Lexer::with_dialect(code_part, dialect).tokenize() {
            Ok(t) => t,
            Err(_) => {
                // Lex error — return line uncolored
                return line.to_string();
            }
        };

        // Lexer columns count Unicode characters, while Rust string slices use
        // bytes. Infer token extents from adjacent spans to preserve source text
        // for escaped strings, numeric literals, and compound keywords alike.
        let offsets: Vec<usize> = code_part
            .char_indices()
            .map(|(i, _)| i)
            .chain([code_part.len()])
            .collect();
        let mut last_end = 0;
        for (index, st) in tokens.iter().enumerate() {
            if matches!(st.token, Token::Newline | Token::Eof) {
                continue;
            }
            let Some(&start) = offsets.get(st.span.col.saturating_sub(1)) else {
                continue;
            };
            let next_start = tokens
                .get(index + 1)
                .filter(|next| next.span.line == st.span.line)
                .and_then(|next| offsets.get(next.span.col.saturating_sub(1)))
                .copied()
                .unwrap_or(code_part.len());
            let end = start + code_part[start..next_start].trim_end().len();
            result.push_str(&code_part[last_end..start]);
            result.push_str(token_color(&st.token));
            result.push_str(&code_part[start..end]);
            result.push_str(COLOR_RESET);
            last_end = end;
        }
        result.push_str(&code_part[last_end..]);
    }

    if let Some(comment) = comment_part {
        result.push_str(COLOR_COMMENT);
        result.push_str(comment);
        result.push_str(COLOR_RESET);
    }

    result
}

fn token_color(token: &Token) -> &'static str {
    match token {
        Token::StringLiteral(_) => COLOR_STRING,
        Token::NumericLiteral(_) | Token::LineNumber(_) => COLOR_NUMBER,
        Token::Identifier(_) => COLOR_IDENT,
        Token::KwRem => COLOR_COMMENT,
        Token::Plus
        | Token::Minus
        | Token::Star
        | Token::Slash
        | Token::Caret
        | Token::Ampersand
        | Token::Dot
        | Token::Equal
        | Token::NotEqual
        | Token::Less
        | Token::Greater
        | Token::LessEqual
        | Token::GreaterEqual
        | Token::LeftParen
        | Token::RightParen
        | Token::Comma
        | Token::Semicolon
        | Token::Hash
        | Token::Colon => COLOR_OPERATOR,
        Token::Newline | Token::Eof => COLOR_OPERATOR, // should be skipped, but safe default
        _ => COLOR_KEYWORD,                            // All Kw* variants
    }
}

pub struct Repl {
    interpreter: Interpreter,
    program: BTreeMap<u32, String>,
}

/// Classifies what the REPL should do with a line of input.
enum ReplAction {
    /// Store a numbered line in the program buffer.
    StoreLine(u32, String),
    /// RUN the stored program.
    Run,
    /// LIST lines, with optional start and end bounds.
    List(Option<u32>, Option<u32>),
    /// NEW — clear the stored program.
    New,
    /// DELETE a line or range (bare line number or explicit DELETE command).
    Delete(u32, Option<u32>),
    /// A command was recognized but had invalid arguments.
    InvalidCommand(String),
    /// Execute immediately (unnumbered line — existing behavior).
    Execute,
}

/// If `line` starts with an integer, return (line_number, rest_of_line).
fn parse_line_number(line: &str) -> Option<(u32, &str)> {
    let trimmed = line.trim();
    if !trimmed.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let end = trimmed
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(trimmed.len());
    let num: u32 = trimmed[..end].parse().ok()?;
    let rest = trimmed[end..].trim_start();
    Some((num, rest))
}

/// Parse a range argument like "10", "10-50", or "" (empty).
/// Returns (Option<start>, Option<end>).
fn parse_range(arg: &str) -> Option<(Option<u32>, Option<u32>)> {
    let arg = arg.trim();
    if arg.is_empty() {
        return Some((None, None));
    }
    let bounds = if let Some((start, end)) = arg.split_once('-') {
        let parse_bound = |bound: &str| {
            let bound = bound.trim();
            if bound.is_empty() {
                Some(None)
            } else {
                bound.parse::<u32>().ok().map(Some)
            }
        };
        (parse_bound(start)?, parse_bound(end)?)
    } else {
        let n = arg.parse::<u32>().ok()?;
        (Some(n), Some(n))
    };
    if matches!(bounds, (Some(start), Some(end)) if start > end) {
        return None;
    }
    Some(bounds)
}

/// Classify a trimmed input line into a REPL action.
fn classify_input(line: &str) -> ReplAction {
    let trimmed = line.trim();
    let upper = trimmed.to_ascii_uppercase();

    // Check for commands first
    if upper == "RUN" {
        return ReplAction::Run;
    }
    if upper == "NEW" {
        return ReplAction::New;
    }
    if upper == "LIST" {
        return ReplAction::List(None, None);
    }
    if let Some(arg) = upper.strip_prefix("LIST ") {
        let arg = arg.trim();
        if arg.is_empty() {
            return ReplAction::List(None, None);
        }
        return match parse_range(arg) {
            Some((start, end)) => ReplAction::List(start, end),
            None => ReplAction::InvalidCommand(format!("Invalid LIST argument: {arg}")),
        };
    }
    if upper == "DELETE" {
        return ReplAction::InvalidCommand("Usage: DELETE <line> or DELETE <start>-<end>".into());
    }
    if let Some(arg) = upper.strip_prefix("DELETE ") {
        let arg = arg.trim();
        if let Some((Some(start), end)) = parse_range(arg) {
            return ReplAction::Delete(start, Some(end.unwrap_or(u32::MAX)));
        }
        return ReplAction::InvalidCommand(format!("Invalid DELETE argument: {}", arg));
    }

    // Check for numbered line
    if let Some((num, rest)) = parse_line_number(trimmed) {
        if rest.is_empty() {
            return ReplAction::Delete(num, None);
        }
        return ReplAction::StoreLine(num, rest.to_string());
    }

    ReplAction::Execute
}

impl Default for Repl {
    fn default() -> Self {
        Self::new()
    }
}

impl Repl {
    pub fn new() -> Self {
        Self {
            interpreter: Interpreter::new(),
            program: BTreeMap::new(),
        }
    }

    pub fn with_dialect(dialect: crate::Dialect) -> Self {
        let mut interpreter = Interpreter::new();
        interpreter.dialect = dialect;
        Self {
            interpreter,
            program: BTreeMap::new(),
        }
    }

    pub fn run(&mut self) {
        println!("RICE BASIC v{}", env!("CARGO_PKG_VERSION"));
        println!("Type SYSTEM or press Ctrl+D to exit.");
        println!("Commands: RUN, LIST, NEW, DELETE");
        println!();

        let mut editor = Editor::new().expect("failed to create editor");
        editor.set_helper(Some(BasicHelper {
            dialect: self.interpreter.dialect,
        }));
        let history_file = dirs_history_path();
        let _ = editor.load_history(&history_file);

        let mut buffer = String::new();
        let mut depth: i32 = 0;

        loop {
            if let Some(helper) = editor.helper_mut() {
                helper.dialect = self.interpreter.dialect;
            }
            let input = if depth > 0 {
                let indent = "    ".repeat(depth as usize);
                editor.readline_with_initial(". ", (&indent, ""))
            } else {
                editor.readline("Ok\n")
            };
            match input {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        if depth > 0 {
                            // In a block, just append a blank line
                            buffer.push('\n');
                        }
                        continue;
                    }

                    // Classify input before block accumulation.
                    // Numbered lines and commands bypass multi-line block logic.
                    if depth == 0 {
                        let action = classify_input(trimmed);
                        if !matches!(action, ReplAction::Execute) {
                            let _ = editor.add_history_entry(trimmed);
                        }
                        match action {
                            ReplAction::StoreLine(num, text) => {
                                self.program.insert(num, text);
                                continue;
                            }
                            ReplAction::Run => {
                                self.run_stored_program();
                                continue;
                            }
                            ReplAction::List(start, end) => {
                                self.list_program(start, end);
                                continue;
                            }
                            ReplAction::New => {
                                self.program.clear();
                                continue;
                            }
                            ReplAction::Delete(start, end) => {
                                self.delete_lines(start, end);
                                continue;
                            }
                            ReplAction::InvalidCommand(msg) => {
                                eprintln!("{msg}");
                                continue;
                            }
                            ReplAction::Execute => {
                                // Fall through to existing depth/buffer logic
                            }
                        }
                    }

                    let delta = compute_depth_delta(trimmed, self.interpreter.dialect);

                    if depth == 0 {
                        if delta <= 0 {
                            // Single-line statement (or stray closing keyword)
                            let _ = editor.add_history_entry(trimmed);
                            match self.execute_line(trimmed) {
                                Ok(true) => break,
                                Ok(false) => {}
                                Err(e) => eprintln!("{e}"),
                            }
                        } else {
                            // Start accumulating a block
                            buffer = trimmed.to_string();
                            depth = delta;
                        }
                    } else {
                        // Already inside a block
                        buffer.push('\n');
                        buffer.push_str(trimmed);
                        depth += delta;

                        if depth <= 0 {
                            // Block complete — execute the full buffer
                            let _ = editor.add_history_entry(&buffer);
                            match self.execute_line(&buffer) {
                                Ok(true) => break,
                                Ok(false) => {}
                                Err(e) => eprintln!("{e}"),
                            }
                            buffer.clear();
                            depth = 0;
                        }
                    }
                }
                Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                    break;
                }
                Err(e) => {
                    eprintln!("Error: {e}");
                    break;
                }
            }
        }

        let _ = editor.save_history(&history_file);
    }

    fn execute_line(&mut self, line: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let dialect = crate::detect_dialect(line).unwrap_or(self.interpreter.dialect);
        let tokens = Lexer::with_dialect(line, dialect).tokenize()?;
        let program = Parser::with_dialect(tokens, dialect).parse_program()?;
        // Check if any statement is END
        let has_end = program
            .statements
            .iter()
            .any(|s| matches!(s.stmt, crate::ast::Stmt::End | crate::ast::Stmt::System));
        self.interpreter.run_source(line)?;
        Ok(has_end)
    }

    /// Reconstruct the stored program and execute it with a fresh interpreter.
    fn run_stored_program(&mut self) {
        if self.program.is_empty() {
            return;
        }
        let source: String = self
            .program
            .iter()
            .map(|(num, text)| format!("{} {}", num, text))
            .collect::<Vec<_>>()
            .join("\n");

        // Fresh interpreter for each RUN (classic behavior: RUN clears variables)
        let dialect = self.interpreter.dialect;
        self.interpreter = Interpreter::new();
        self.interpreter.dialect = dialect;

        match self.interpreter.run_source(&source) {
            Ok(()) => {}
            Err(e) => eprintln!("{e}"),
        }
    }

    /// Display stored program lines, optionally filtered by range.
    fn list_program(&self, start: Option<u32>, end: Option<u32>) {
        let start = start.unwrap_or(0);
        let end = end.unwrap_or(u32::MAX);
        if start > end {
            return;
        }
        for (num, text) in self.program.range(start..=end) {
            println!("{} {}", num, text);
        }
    }

    /// Delete a line or range of lines from the stored program.
    fn delete_lines(&mut self, start: u32, end: Option<u32>) {
        let end = end.unwrap_or(start);
        if start > end {
            return;
        }
        let to_remove: Vec<u32> = self.program.range(start..=end).map(|(&k, _)| k).collect();
        for k in to_remove {
            self.program.remove(&k);
        }
    }
}

/// Compute the net nesting depth change for a single line of BASIC code.
/// Returns positive for block openers, negative for block closers.
fn compute_depth_delta(line: &str, dialect: crate::Dialect) -> i32 {
    let tokens = match Lexer::with_dialect(line, dialect).tokenize() {
        Ok(t) => t,
        Err(_) => return 0,
    };

    // Block keywords only open/close blocks at the start of a statement.
    // FOR in OPEN, SUB in DECLARE, and NEXT in RESUME are not delimiters.
    tokens
        .split(|st| matches!(st.token, Token::Colon | Token::Newline | Token::Eof))
        .map(|statement| {
            let statement = if statement
                .first()
                .is_some_and(|st| matches!(st.token, Token::LineNumber(_)))
            {
                &statement[1..]
            } else {
                statement
            };
            let Some(first) = statement.first() else {
                return 0;
            };
            match first.token {
                Token::KwFor
                | Token::KwDo
                | Token::KwWhile
                | Token::KwSub
                | Token::KwFunction
                | Token::KwSelect
                | Token::KwType
                | Token::KwWhen => 1,
                Token::KwIf if statement.last().is_some_and(|st| st.token == Token::KwThen) => 1,
                Token::KwDef if !statement.iter().any(|st| st.token == Token::Equal) => 1,
                Token::KwNext
                | Token::KwWend
                | Token::KwLoop
                | Token::KwEndIf
                | Token::KwEndSub
                | Token::KwEndFunction
                | Token::KwEndSelect
                | Token::KwEndType
                | Token::KwEndWhile
                | Token::KwEndWhen
                | Token::KwEndDef => -1,
                _ => 0,
            }
        })
        .sum()
}

fn dirs_history_path() -> String {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    format!("{home}/.rice_history")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compute_depth_delta(line: &str) -> i32 {
        super::compute_depth_delta(line, crate::Dialect::Ansi)
    }

    fn highlight_line(line: &str) -> String {
        super::highlight_line(line, crate::Dialect::Ansi)
    }

    #[test]
    fn test_depth_single_line_if() {
        // Single-line IF — THEN is NOT the last token
        assert_eq!(compute_depth_delta("IF x > 0 THEN PRINT x"), 0);
    }

    #[test]
    fn test_depth_block_if() {
        // Block IF — THEN IS the last token
        assert_eq!(compute_depth_delta("IF x > 0 THEN"), 1);
    }

    #[test]
    fn test_depth_end_if() {
        assert_eq!(compute_depth_delta("END IF"), -1);
    }

    #[test]
    fn test_depth_for() {
        assert_eq!(compute_depth_delta("FOR i = 1 TO 10"), 1);
    }

    #[test]
    fn test_depth_next() {
        assert_eq!(compute_depth_delta("NEXT i"), -1);
    }

    #[test]
    fn test_depth_for_next_same_line() {
        // FOR and NEXT on same line via colon — net 0
        assert_eq!(compute_depth_delta("FOR i = 1 TO 3: PRINT i: NEXT i"), 0);
    }

    #[test]
    fn test_depth_while() {
        assert_eq!(compute_depth_delta("WHILE x > 0"), 1);
    }

    #[test]
    fn test_depth_wend() {
        assert_eq!(compute_depth_delta("WEND"), -1);
    }

    #[test]
    fn test_depth_end_while() {
        assert_eq!(compute_depth_delta("END WHILE"), -1);
    }

    #[test]
    fn test_depth_do_loop() {
        assert_eq!(compute_depth_delta("DO"), 1);
        assert_eq!(compute_depth_delta("DO WHILE x > 0"), 1);
        assert_eq!(compute_depth_delta("LOOP"), -1);
        assert_eq!(compute_depth_delta("LOOP UNTIL x = 0"), -1);
    }

    #[test]
    fn test_depth_sub() {
        assert_eq!(compute_depth_delta("SUB MySub"), 1);
        assert_eq!(compute_depth_delta("END SUB"), -1);
    }

    #[test]
    fn test_depth_function() {
        assert_eq!(compute_depth_delta("FUNCTION MyFunc"), 1);
        assert_eq!(compute_depth_delta("END FUNCTION"), -1);
    }

    #[test]
    fn test_depth_select_case() {
        assert_eq!(compute_depth_delta("SELECT CASE x"), 1);
        assert_eq!(compute_depth_delta("END SELECT"), -1);
    }

    #[test]
    fn test_depth_plain_statement() {
        assert_eq!(compute_depth_delta("PRINT \"hello\""), 0);
        assert_eq!(compute_depth_delta("LET x = 5"), 0);
    }

    #[test]
    fn test_highlight_contains_colors() {
        let result = highlight_line("PRINT \"hello\"");
        assert!(
            result.contains(COLOR_KEYWORD),
            "should contain keyword color"
        );
        assert!(result.contains(COLOR_STRING), "should contain string color");
        assert!(result.contains(COLOR_RESET), "should contain reset");
    }

    #[test]
    fn test_highlight_comment() {
        let result = highlight_line("' this is a comment");
        assert!(
            result.contains(COLOR_COMMENT),
            "should contain comment color"
        );
        // The whole line should be a comment
        assert!(
            !result.contains(COLOR_KEYWORD),
            "should not contain keyword color"
        );
    }

    #[test]
    fn test_highlight_inline_comment() {
        let result = highlight_line("x = 42 ' inline");
        assert!(result.contains(COLOR_IDENT), "should have identifier color");
        assert!(result.contains(COLOR_NUMBER), "should have number color");
        assert!(result.contains(COLOR_COMMENT), "should have comment color");
    }

    #[test]
    fn test_highlight_empty_line() {
        assert_eq!(highlight_line(""), "");
    }

    #[test]
    fn test_highlight_lex_error_fallback() {
        // Unterminated string should fall back to uncolored
        let result = highlight_line("PRINT \"hello");
        assert_eq!(result, "PRINT \"hello");
    }

    #[test]
    fn test_find_comment_not_in_string() {
        // Apostrophe inside string should NOT be detected as comment
        assert_eq!(find_comment_start("PRINT \"it's fine\""), None);
        // Apostrophe outside string should be detected
        assert_eq!(find_comment_start("x = 1 ' comment"), Some(6));
    }

    #[test]
    fn test_find_comment_rem() {
        assert_eq!(find_comment_start("REM this is a comment"), Some(0));
        assert_eq!(find_comment_start("x = 1: REM comment"), Some(7));
        // REMEMBER should NOT trigger REM detection
        assert_eq!(find_comment_start("REMEMBER = 1"), None);
    }

    // --- Line number REPL tests ---

    #[test]
    fn test_parse_line_number_with_statement() {
        let (num, rest) = parse_line_number("10 PRINT \"HELLO\"").unwrap();
        assert_eq!(num, 10);
        assert_eq!(rest, "PRINT \"HELLO\"");
    }

    #[test]
    fn test_parse_line_number_bare() {
        let (num, rest) = parse_line_number("10").unwrap();
        assert_eq!(num, 10);
        assert_eq!(rest, "");
    }

    #[test]
    fn test_parse_line_number_none() {
        assert!(parse_line_number("PRINT 42").is_none());
        assert!(parse_line_number("").is_none());
        assert!(parse_line_number("  HELLO").is_none());
    }

    #[test]
    fn test_parse_line_number_large() {
        let (num, rest) = parse_line_number("65000 REM end").unwrap();
        assert_eq!(num, 65000);
        assert_eq!(rest, "REM end");
    }

    #[test]
    fn test_parse_range_empty() {
        assert_eq!(parse_range(""), Some((None, None)));
    }

    #[test]
    fn test_parse_range_single() {
        assert_eq!(parse_range("10"), Some((Some(10), Some(10))));
    }

    #[test]
    fn test_parse_range_full() {
        assert_eq!(parse_range("10-50"), Some((Some(10), Some(50))));
    }

    #[test]
    fn test_parse_range_with_spaces() {
        assert_eq!(parse_range(" 10 - 50 "), Some((Some(10), Some(50))));
    }

    #[test]
    fn test_classify_run() {
        assert!(matches!(classify_input("RUN"), ReplAction::Run));
        assert!(matches!(classify_input("run"), ReplAction::Run));
        assert!(matches!(classify_input("Run"), ReplAction::Run));
    }

    #[test]
    fn test_classify_new() {
        assert!(matches!(classify_input("NEW"), ReplAction::New));
        assert!(matches!(classify_input("new"), ReplAction::New));
    }

    #[test]
    fn test_classify_list() {
        assert!(matches!(
            classify_input("LIST"),
            ReplAction::List(None, None)
        ));
        assert!(matches!(
            classify_input("LIST 10"),
            ReplAction::List(Some(10), Some(10))
        ));
        assert!(matches!(
            classify_input("LIST 10-50"),
            ReplAction::List(Some(10), Some(50))
        ));
        assert!(matches!(
            classify_input("list"),
            ReplAction::List(None, None)
        ));
    }

    #[test]
    fn test_classify_delete() {
        assert!(matches!(
            classify_input("DELETE 10"),
            ReplAction::Delete(10, Some(10))
        ));
        assert!(matches!(
            classify_input("DELETE 10-50"),
            ReplAction::Delete(10, Some(50))
        ));
    }

    #[test]
    fn test_classify_delete_invalid() {
        assert!(matches!(
            classify_input("DELETE"),
            ReplAction::InvalidCommand(_)
        ));
        assert!(matches!(
            classify_input("DELETE abc"),
            ReplAction::InvalidCommand(_)
        ));
    }

    #[test]
    fn test_classify_store_line() {
        match classify_input("10 PRINT \"HELLO\"") {
            ReplAction::StoreLine(num, text) => {
                assert_eq!(num, 10);
                assert_eq!(text, "PRINT \"HELLO\"");
            }
            _ => panic!("expected StoreLine"),
        }
    }

    #[test]
    fn test_classify_delete_line() {
        assert!(matches!(classify_input("10"), ReplAction::Delete(10, None)));
    }

    #[test]
    fn test_classify_execute() {
        assert!(matches!(classify_input("PRINT 42"), ReplAction::Execute));
        assert!(matches!(classify_input("LET X = 5"), ReplAction::Execute));
    }
    #[test]
    fn test_depth_keywords_in_other_statements() {
        for line in [
            "EXIT FOR",
            "EXIT DO",
            "EXIT SUB",
            "EXIT FUNCTION",
            "DECLARE SUB Foo()",
            "DECLARE FUNCTION Foo()",
            "RESUME NEXT",
            r#"OPEN "test.txt" FOR INPUT AS #1"#,
        ] {
            assert_eq!(
                super::compute_depth_delta(line, crate::Dialect::QuickBasic),
                0,
                "{line}"
            );
        }
        assert_eq!(compute_depth_delta("LOOP WHILE x < 3"), -1);
        assert_eq!(compute_depth_delta("DEF FNdouble(x)"), 1);
        assert_eq!(compute_depth_delta("DEF FNdouble(x) = x * 2"), 0);
        assert_eq!(compute_depth_delta("END DEF"), -1);
    }

    #[test]
    fn test_highlight_preserves_unicode_and_complete_token_text() {
        for line in [
            r#"PRINT "é😀"; "ok""yes"; 3"#,
            r#"PRINT "😀": REM comment"#,
            "END\tWHILE",
            "PRINT &HFF&; &O10",
            "PRINT \"你好\"\nPRINT 42",
            "PRINT é",
        ] {
            let highlighted = super::highlight_line(line, crate::Dialect::QuickBasic);
            let plain = [
                COLOR_KEYWORD,
                COLOR_STRING,
                COLOR_NUMBER,
                COLOR_IDENT,
                COLOR_OPERATOR,
                COLOR_COMMENT,
                COLOR_RESET,
            ]
            .iter()
            .fold(highlighted, |text, code| text.replace(code, ""));
            assert_eq!(plain, line);
        }
    }

    #[test]
    fn test_comment_boundaries_are_unicode_safe() {
        assert_eq!(find_comment_start("PRINT 😀"), None);
        assert_eq!(find_comment_start("\tREM comment"), Some(1));
        assert_eq!(find_comment_start("REM$ = \"value\""), None);
    }

    #[test]
    fn test_repl_rejects_malformed_and_reversed_ranges() {
        for line in [
            "LIST 50-10",
            "DELETE 50-10",
            "LIST abc",
            "DELETE 10-abc",
            "DELETE 10-20-30",
        ] {
            assert!(
                matches!(classify_input(line), ReplAction::InvalidCommand(_)),
                "{line}"
            );
        }
        assert!(matches!(
            classify_input("DELETE 10-"),
            ReplAction::Delete(10, Some(u32::MAX))
        ));
    }

    #[test]
    fn test_immediate_dialect_directive_applies_before_parsing() {
        let mut repl = Repl::with_dialect(crate::Dialect::Ansi);
        assert!(!repl.execute_line("OPTION DIALECT \"QB\": X% = 7").unwrap());
        assert_eq!(repl.interpreter.dialect, crate::Dialect::QuickBasic);
    }
}
