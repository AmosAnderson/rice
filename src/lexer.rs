use crate::error::LexError;
use crate::token::{Span, SpannedToken, Token, TypeSuffix};

pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
    at_line_start: bool,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
            at_line_start: true,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<SpannedToken>, LexError> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.token == Token::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<SpannedToken, LexError> {
        self.skip_whitespace();

        if self.pos >= self.source.len() {
            return Ok(self.make_token(Token::Eof));
        }

        let ch = self.peek_char().unwrap();

        // Comments with '
        if ch == '\'' {
            self.skip_to_eol();
            return Ok(self.make_token(Token::Newline));
        }

        // Newlines
        if ch == '\n' {
            let tok = self.make_token(Token::Newline);
            self.advance_char();
            self.at_line_start = true;
            return Ok(tok);
        }
        if ch == '\r' {
            let tok = self.make_token(Token::Newline);
            self.advance_char();
            if self.peek_char() == Some('\n') {
                self.advance_char();
            }
            self.at_line_start = true;
            return Ok(tok);
        }

        // Numbers (or line numbers at start of line)
        if ch.is_ascii_digit() || (ch == '.' && self.peek_next().is_some_and(|c| c.is_ascii_digit()))
        {
            return self.read_number();
        }

        // String literals
        if ch == '"' {
            return self.read_string();
        }

        // Identifiers and keywords
        if ch.is_ascii_alphabetic() || ch == '_' {
            return self.read_word();
        }

        // Operators and delimiters
        let span = self.current_span();
        self.advance_char();
        let token = match ch {
            '+' => Token::Plus,
            '-' => Token::Minus,
            '*' => Token::Star,
            '/' => Token::Slash,
            '\\' => Token::Backslash,
            '^' => Token::Caret,
            '=' => Token::Equal,
            '<' => {
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Token::LessEqual
                } else if self.peek_char() == Some('>') {
                    self.advance_char();
                    Token::NotEqual
                } else {
                    Token::Less
                }
            }
            '>' => {
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Token::GreaterEqual
                } else {
                    Token::Greater
                }
            }
            '(' => Token::LeftParen,
            ')' => Token::RightParen,
            ',' => Token::Comma,
            ';' => Token::Semicolon,
            ':' => Token::Colon,
            '#' => Token::Hash,
            '.' => Token::Dot,
            _ => {
                return Err(LexError::UnexpectedChar {
                    line: span.line,
                    col: span.col,
                    ch,
                });
            }
        };

        self.at_line_start = false;
        Ok(SpannedToken { token, span })
    }

    fn read_number(&mut self) -> Result<SpannedToken, LexError> {
        let span = self.current_span();
        let start = self.pos;
        let mut has_dot = false;

        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                self.advance_char();
            } else if ch == '.' && !has_dot {
                has_dot = true;
                self.advance_char();
            } else {
                break;
            }
        }

        // Check for type suffix on number
        let suffix = self.peek_char().and_then(TypeSuffix::from_char);
        if suffix.is_some() {
            self.advance_char();
        }

        // Check for scientific notation (E or D)
        if let Some(ch) = self.peek_char()
            && matches!(ch, 'E' | 'e' | 'D' | 'd')
        {
            has_dot = true; // force float
            self.advance_char();
            if let Some(sign) = self.peek_char()
                && (sign == '+' || sign == '-')
            {
                self.advance_char();
            }
            while let Some(ch) = self.peek_char() {
                if ch.is_ascii_digit() {
                    self.advance_char();
                } else {
                    break;
                }
            }
        }

        let num_str: String = self.source[start..self.pos].iter().collect();
        // Remove suffix char from parse string if present
        let parse_str = if suffix.is_some() {
            &num_str[..num_str.len() - 1]
        } else {
            &num_str
        };
        // Replace D with E for Rust parsing
        let parse_str = parse_str.replace(['D', 'd'], "E");

        if self.at_line_start && !has_dot && suffix.is_none() {
            // Line number
            let n: u32 = parse_str.parse().map_err(|_| LexError::InvalidNumber {
                line: span.line,
                col: span.col,
            })?;
            self.at_line_start = false;
            return Ok(SpannedToken {
                token: Token::LineNumber(n),
                span,
            });
        }

        self.at_line_start = false;

        let token = if has_dot || matches!(suffix, Some(TypeSuffix::Single | TypeSuffix::Double)) {
            let n: f64 = parse_str.parse().map_err(|_| LexError::InvalidNumber {
                line: span.line,
                col: span.col,
            })?;
            Token::DoubleLiteral(n)
        } else {
            let n: i64 = parse_str.parse().map_err(|_| LexError::InvalidNumber {
                line: span.line,
                col: span.col,
            })?;
            Token::IntegerLiteral(n)
        };

        Ok(SpannedToken { token, span })
    }

    fn read_string(&mut self) -> Result<SpannedToken, LexError> {
        let span = self.current_span();
        self.advance_char(); // consume opening "
        let mut s = String::new();

        loop {
            match self.peek_char() {
                None | Some('\n') | Some('\r') => {
                    return Err(LexError::UnterminatedString {
                        line: span.line,
                        col: span.col,
                    });
                }
                Some('"') => {
                    self.advance_char();
                    break;
                }
                Some(ch) => {
                    s.push(ch);
                    self.advance_char();
                }
            }
        }

        self.at_line_start = false;
        Ok(SpannedToken {
            token: Token::StringLiteral(s),
            span,
        })
    }

    fn read_word(&mut self) -> Result<SpannedToken, LexError> {
        let span = self.current_span();
        let start = self.pos;

        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                self.advance_char();
            } else {
                break;
            }
        }

        let word: String = self.source[start..self.pos]
            .iter()
            .flat_map(|c| c.to_uppercase())
            .collect();

        // Check for type suffix
        let suffix = self.peek_char().and_then(TypeSuffix::from_char);
        if suffix.is_some() {
            self.advance_char();
        }

        // Check for compound keywords
        if let Some(token) = self.match_compound_keyword(&word) {
            self.at_line_start = false;
            return Ok(SpannedToken { token, span });
        }

        // Check for REM (rest of line is comment)
        if word == "REM" && suffix.is_none() {
            self.skip_to_eol();
            // Don't set at_line_start to false; newline will handle it
            return Ok(SpannedToken {
                token: Token::Newline,
                span,
            });
        }

        // Match keywords
        let token = if suffix.is_none() {
            match word.as_str() {
                "PRINT" => Token::KwPrint,
                "INPUT" => Token::KwInput,
                "LET" => Token::KwLet,
                "DIM" => Token::KwDim,
                "CONST" => Token::KwConst,
                "AS" => Token::KwAs,
                "IF" => Token::KwIf,
                "THEN" => Token::KwThen,
                "ELSE" => Token::KwElse,
                "ELSEIF" => Token::KwElseIf,
                "FOR" => Token::KwFor,
                "TO" => Token::KwTo,
                "STEP" => Token::KwStep,
                "NEXT" => Token::KwNext,
                "WHILE" => Token::KwWhile,
                "WEND" => Token::KwWend,
                "DO" => Token::KwDo,
                "LOOP" => Token::KwLoop,
                "UNTIL" => Token::KwUntil,
                "GOTO" => Token::KwGoto,
                "GOSUB" => Token::KwGosub,
                "RETURN" => Token::KwReturn,
                "SELECT" => Token::KwSelect,
                "CASE" => Token::KwCase,
                "IS" => Token::KwIs,
                "END" => Token::KwEnd,
                "STOP" => Token::KwStop,
                "SYSTEM" => Token::KwSystem,
                "QUIT" => Token::KwSystem,
                "EXIT" => Token::KwExit,
                "SUB" => Token::KwSub,
                "FUNCTION" => Token::KwFunction,
                "CALL" => Token::KwCall,
                "DECLARE" => Token::KwDeclare,
                "SHARED" => Token::KwShared,
                "STATIC" => Token::KwStatic,
                "BYVAL" => Token::KwByVal,
                "REDIM" => Token::KwRedim,
                "ERASE" => Token::KwErase,
                "PRESERVE" => Token::KwPreserve,
                "OPTION" => Token::KwOption,
                "BASE" => Token::KwBase,
                "SWAP" => Token::KwSwap,
                "TYPE" => Token::KwType,
                "DATA" => Token::KwData,
                "READ" => Token::KwRead,
                "RESTORE" => Token::KwRestore,
                "OPEN" => Token::KwOpen,
                "CLOSE" => Token::KwClose,
                "WRITE" => Token::KwWrite,
                "APPEND" => Token::KwAppend,
                "OUTPUT" => Token::KwOutput,
                "BINARY" => Token::KwBinary,
                "RANDOM" => Token::KwRandom,
                "LEN" => Token::KwLen,
                "GET" => Token::KwGet,
                "PUT" => Token::KwPut,
                "FREEFILE" => Token::KwFreefile,
                "LPRINT" => Token::KwLPrint,
                "USING" => Token::KwUsing,
                "ON" => Token::KwOn,
                "ERROR" => Token::KwError,
                "RESUME" => Token::KwResume,
                "AND" => Token::KwAnd,
                "OR" => Token::KwOr,
                "NOT" => Token::KwNot,
                "XOR" => Token::KwXor,
                "EQV" => Token::KwEqv,
                "IMP" => Token::KwImp,
                "MOD" => Token::KwMod,
                "TAB" => Token::KwTab,
                "SPC" => Token::KwSpc,
                "INTEGER" => Token::KwInteger,
                "LONG" => Token::KwLong,
                "SINGLE" => Token::KwSingle,
                "DOUBLE" => Token::KwDouble,
                "STRING" => Token::KwString,
                "RANDOMIZE" => Token::KwRandomize,
                "TIMER" => Token::KwTimer,
                "SLEEP" => Token::KwSleep,
                "CLEAR" => Token::KwClear,
                "NAME" => Token::KwName,
                "KILL" => Token::KwKill,
                "MKDIR" => Token::KwMkdir,
                "RMDIR" => Token::KwRmdir,
                "CHDIR" => Token::KwChdir,
                "SHELL" => Token::KwShell,
                "LSET" => Token::KwLset,
                "RSET" => Token::KwRset,
                "DEF" => Token::KwDef,
                "DEFINT" => Token::KwDefInt,
                "DEFLNG" => Token::KwDefLng,
                "DEFSNG" => Token::KwDefSng,
                "DEFDBL" => Token::KwDefDbl,
                "DEFSTR" => Token::KwDefStr,
                _ => Token::Identifier { name: word, suffix },
            }
        } else {
            Token::Identifier { name: word, suffix }
        };

        self.at_line_start = false;
        Ok(SpannedToken { token, span })
    }

    fn match_compound_keyword(&mut self, word: &str) -> Option<Token> {
        let save_pos = self.pos;
        let save_col = self.col;

        match word {
            "END" => {
                self.skip_whitespace();
                if let Some(next_word) = self.peek_word() {
                    let next_upper = next_word.to_uppercase();
                    match next_upper.as_str() {
                        "IF" => {
                            self.consume_word();
                            return Some(Token::KwEndIf);
                        }
                        "SUB" => {
                            self.consume_word();
                            return Some(Token::KwEndSub);
                        }
                        "FUNCTION" => {
                            self.consume_word();
                            return Some(Token::KwEndFunction);
                        }
                        "SELECT" => {
                            self.consume_word();
                            return Some(Token::KwEndSelect);
                        }
                        "TYPE" => {
                            self.consume_word();
                            return Some(Token::KwEndType);
                        }
                        "DEF" => {
                            self.consume_word();
                            return Some(Token::KwEndDef);
                        }
                        _ => {}
                    }
                }
            }
            "LINE" => {
                self.skip_whitespace();
                if let Some(next_word) = self.peek_word()
                    && next_word.to_uppercase() == "INPUT"
                {
                    self.consume_word();
                    return Some(Token::KwLineInput);
                }
            }
            "SELECT" => {
                self.skip_whitespace();
                if let Some(next_word) = self.peek_word()
                    && next_word.to_uppercase() == "CASE"
                {
                    self.consume_word();
                    return Some(Token::KwSelect);
                }
            }
            "OPTION" => {
                self.skip_whitespace();
                if let Some(next_word) = self.peek_word()
                    && next_word.to_uppercase() == "BASE"
                {
                    self.consume_word();
                    return Some(Token::KwOption);
                }
            }
            _ => {}
        }

        // Restore position if no match
        self.pos = save_pos;
        self.col = save_col;
        None
    }

    fn peek_word(&self) -> Option<String> {
        let mut i = self.pos;
        while i < self.source.len() && self.source[i].is_ascii_alphabetic() {
            i += 1;
        }
        if i > self.pos {
            Some(self.source[self.pos..i].iter().collect())
        } else {
            None
        }
    }

    fn consume_word(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_alphabetic() {
                self.advance_char();
            } else {
                break;
            }
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch == ' ' || ch == '\t' {
                self.advance_char();
            } else {
                break;
            }
        }
    }

    fn skip_to_eol(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch == '\n' || ch == '\r' {
                break;
            }
            self.advance_char();
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.source.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.source.get(self.pos + 1).copied()
    }

    fn advance_char(&mut self) -> char {
        let ch = self.source[self.pos];
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        ch
    }

    fn current_span(&self) -> Span {
        Span {
            line: self.line,
            col: self.col,
        }
    }

    fn make_token(&self, token: Token) -> SpannedToken {
        SpannedToken {
            token,
            span: self.current_span(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokenize(s: &str) -> Vec<Token> {
        Lexer::new(s)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|st| st.token)
            .filter(|t| !matches!(t, Token::Newline | Token::Eof))
            .collect()
    }

    #[test]
    fn test_arithmetic() {
        // Prefix with a keyword so "2" doesn't get parsed as a line number
        let tokens = tokenize("PRINT 2 + 3 * 4");
        assert_eq!(
            tokens,
            vec![
                Token::KwPrint,
                Token::IntegerLiteral(2),
                Token::Plus,
                Token::IntegerLiteral(3),
                Token::Star,
                Token::IntegerLiteral(4),
            ]
        );
    }

    #[test]
    fn test_case_insensitive_keywords() {
        assert_eq!(tokenize("Print"), vec![Token::KwPrint]);
        assert_eq!(tokenize("PRINT"), vec![Token::KwPrint]);
        assert_eq!(tokenize("print"), vec![Token::KwPrint]);
    }

    #[test]
    fn test_type_suffix() {
        assert_eq!(
            tokenize("x%"),
            vec![Token::Identifier {
                name: "X".into(),
                suffix: Some(TypeSuffix::Integer)
            }]
        );
        assert_eq!(
            tokenize("name$"),
            vec![Token::Identifier {
                name: "NAME".into(),
                suffix: Some(TypeSuffix::String)
            }]
        );
    }

    #[test]
    fn test_line_number() {
        let tokens: Vec<Token> = Lexer::new("100 PRINT 5")
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|st| st.token)
            .filter(|t| !matches!(t, Token::Eof | Token::Newline))
            .collect();
        assert_eq!(
            tokens,
            vec![
                Token::LineNumber(100),
                Token::KwPrint,
                Token::IntegerLiteral(5),
            ]
        );
    }

    #[test]
    fn test_string_literal() {
        assert_eq!(
            tokenize(r#""hello world""#),
            vec![Token::StringLiteral("hello world".into())]
        );
    }

    #[test]
    fn test_compound_keywords() {
        assert_eq!(tokenize("END IF"), vec![Token::KwEndIf]);
        assert_eq!(tokenize("END SUB"), vec![Token::KwEndSub]);
        assert_eq!(tokenize("LINE INPUT"), vec![Token::KwLineInput]);
    }

    #[test]
    fn test_comparison_operators() {
        assert_eq!(
            tokenize("<> <= >="),
            vec![Token::NotEqual, Token::LessEqual, Token::GreaterEqual]
        );
    }

    #[test]
    fn test_comment_rem() {
        assert_eq!(tokenize("REM this is ignored"), vec![]);
    }

    #[test]
    fn test_comment_apostrophe() {
        assert_eq!(tokenize("' this is ignored"), vec![]);
    }

    #[test]
    fn test_float() {
        assert_eq!(tokenize("3.14"), vec![Token::DoubleLiteral(314_f64 / 100.0)]);
    }
}
