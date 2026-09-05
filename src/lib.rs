pub mod ast;
pub mod builtins;
pub mod environment;
pub mod error;
pub mod format_using;
pub mod interpreter;
pub mod lexer;
pub mod mat;
pub mod parser;
pub mod repl;
pub mod token;
pub mod value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Ansi,
    QuickBasic,
}

pub const DEFAULT_DIALECT: Dialect = Dialect::QuickBasic;

/// Detect a source directive using BASIC tokens so strings, comments, and
/// identifiers containing directive-like text do not change the dialect.
pub fn detect_dialect(source: &str) -> Option<Dialect> {
    use token::Token;

    // Inspect lines independently so an unrelated lexical error does not hide
    // a directive elsewhere. QuickBasic accepts both dialects' identifier forms.
    for line in source.lines() {
        let Ok(tokens) = lexer::Lexer::with_dialect(line, Dialect::QuickBasic).tokenize() else {
            continue;
        };
        for window in tokens.windows(3) {
            if let [
                Token::KwOption,
                Token::Identifier(name),
                Token::StringLiteral(value),
            ] = [&window[0].token, &window[1].token, &window[2].token]
                && name == "DIALECT"
            {
                match value.to_ascii_uppercase().as_str() {
                    "ANSI" => return Some(Dialect::Ansi),
                    "QB" | "QBASIC" | "QBASIC 1.1" | "QBASIC1.1" | "QUICKBASIC" => {
                        return Some(Dialect::QuickBasic);
                    }
                    _ => {}
                }
            }
        }
    }
    None
}

/// Non-blocking read of a single keypress using crossterm.
/// Returns empty string if no key available or on error.
pub fn poll_inkey() -> String {
    use crossterm::event::{self, Event};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode, is_raw_mode_enabled};
    use std::time::Duration;

    let Ok(was_raw) = is_raw_mode_enabled() else {
        return String::new();
    };
    if !was_raw && enable_raw_mode().is_err() {
        return String::new();
    }
    let result = if event::poll(Duration::ZERO).unwrap_or(false) {
        match event::read() {
            Ok(Event::Key(key)) => inkey_text(key),
            _ => String::new(),
        }
    } else {
        String::new()
    };
    if !was_raw {
        disable_raw_mode().ok();
    }
    result
}

fn inkey_text(key: crossterm::event::KeyEvent) -> String {
    use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

    if key.kind == KeyEventKind::Release {
        return String::new();
    }
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let upper = c.to_ascii_uppercase();
                match upper {
                    '@'..='_' => String::from((upper as u8 & 0x1f) as char),
                    ' ' => String::from('\0'),
                    '?' => String::from('\x7f'),
                    _ => String::from(c),
                }
            } else {
                String::from(c)
            }
        }
        KeyCode::Enter => String::from('\r'),
        KeyCode::Esc => String::from('\x1b'),
        KeyCode::Backspace => String::from('\x08'),
        KeyCode::Tab => String::from('\t'),
        KeyCode::Up => "\0H".to_string(),
        KeyCode::Down => "\0P".to_string(),
        KeyCode::Left => "\0K".to_string(),
        KeyCode::Right => "\0M".to_string(),
        KeyCode::Home => "\0G".to_string(),
        KeyCode::End => "\0O".to_string(),
        KeyCode::PageUp => "\0I".to_string(),
        KeyCode::PageDown => "\0Q".to_string(),
        KeyCode::Insert => "\0R".to_string(),
        KeyCode::Delete => "\0S".to_string(),
        KeyCode::F(n) if (1..=10).contains(&n) => format!("\0{}", (58 + n) as char),
        _ => String::new(),
    }
}

/// Update a screen buffer with text, tracking row/col position.
pub fn update_screen_buffer(
    buffer: &mut [Vec<u8>],
    print_row: &mut usize,
    print_col: &mut usize,
    text: &str,
) {
    for ch in text.bytes() {
        if ch == b'\n' {
            *print_col = 0;
            *print_row += 1;
        } else if ch == b'\r' {
            *print_col = 0;
        } else {
            let row = print_row.saturating_sub(1);
            let col = *print_col;
            if row < buffer.len() && col < buffer[row].len() {
                buffer[row][col] = ch;
            }
            *print_col += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialect_detection_ignores_strings_comments_and_identifier_fragments() {
        for source in [
            r#"PRINT "OPTION DIALECT ""ANSI""""#,
            r#"REM OPTION DIALECT "ANSI""#,
            r#"PRINT 1 ' OPTION DIALECT "ANSI""#,
            r#"MYOPTION DIALECT "ANSI""#,
            r#"OPTIONDIALECT "ANSI""#,
        ] {
            assert_eq!(detect_dialect(source), None, "{source}");
        }
    }

    #[test]
    fn dialect_detection_accepts_labels_comments_and_whitespace() {
        assert_eq!(
            detect_dialect("10 option\tdialect \"ANSI\" ' comment"),
            Some(Dialect::Ansi)
        );
        assert_eq!(
            detect_dialect("label: OPTION DIALECT \"QBasic 1.1\""),
            Some(Dialect::QuickBasic)
        );
        assert_eq!(
            detect_dialect("PREM = 1: OPTION DIALECT \"ANSI\""),
            Some(Dialect::Ansi)
        );
        assert_eq!(
            detect_dialect("REM$ = \"text\": OPTION DIALECT \"ANSI\""),
            Some(Dialect::Ansi)
        );
    }
    #[test]
    fn inkey_maps_control_characters_without_truncating_unicode() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        for (input, expected) in [
            ('a', "\x01"),
            ('A', "\x01"),
            ('z', "\x1a"),
            ('[', "\x1b"),
            (' ', "\0"),
            ('é', "é"),
        ] {
            assert_eq!(
                inkey_text(KeyEvent::new(KeyCode::Char(input), KeyModifiers::CONTROL)),
                expected
            );
        }
    }

    #[test]
    fn inkey_ignores_release_events_and_preserves_extended_keys() {
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
        let mut key = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(inkey_text(key), "\0H");
        key.kind = KeyEventKind::Release;
        assert_eq!(inkey_text(key), "");
    }
}
