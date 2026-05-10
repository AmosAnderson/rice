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

/// Non-blocking read of a single keypress using crossterm.
/// Returns empty string if no key available or on error.
pub fn poll_inkey() -> String {
    use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
    use std::time::Duration;
    if crossterm::terminal::enable_raw_mode().is_err() {
        return String::new();
    }
    let result = if event::poll(Duration::ZERO).unwrap_or(false) {
        match event::read() {
            Ok(Event::Key(KeyEvent { code, modifiers, .. })) => match code {
                KeyCode::Char(c) => {
                    if modifiers.contains(KeyModifiers::CONTROL) {
                        let ctrl = (c as u8).wrapping_sub(b'a').wrapping_add(1);
                        String::from(ctrl as char)
                    } else {
                        String::from(c)
                    }
                }
                KeyCode::Enter => String::from('\r'),
                KeyCode::Esc => String::from(27 as char),
                KeyCode::Backspace => String::from(8 as char),
                KeyCode::Tab => String::from(9 as char),
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
            },
            _ => String::new(),
        }
    } else {
        String::new()
    };
    crossterm::terminal::disable_raw_mode().ok();
    result
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
