use thiserror::Error;

#[derive(Error, Debug)]
pub enum LexError {
    #[error("line {line}, col {col}: unterminated string literal")]
    UnterminatedString { line: usize, col: usize },
    #[error("line {line}, col {col}: unexpected character '{ch}'")]
    UnexpectedChar {
        line: usize,
        col: usize,
        ch: char,
    },
    #[error("line {line}, col {col}: invalid number")]
    InvalidNumber { line: usize, col: usize },
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("line {line}: expected {expected}, found {found}")]
    Expected {
        line: usize,
        expected: String,
        found: String,
    },
    #[error("line {line}: unexpected token: {token}")]
    Unexpected { line: usize, token: String },
    #[error("line {line}: {msg}")]
    General { line: usize, msg: String },
}

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("type mismatch: {msg}")]
    TypeMismatch { msg: String },
    #[error("division by zero")]
    DivisionByZero,
    #[error("undefined variable: {name}")]
    UndefinedVariable { name: String },
    #[error("overflow: {msg}")]
    Overflow { msg: String },
    #[error("subscript out of range")]
    SubscriptOutOfRange,
    #[error("undefined label: {label}")]
    UndefinedLabel { label: String },
    #[error("wrong number of arguments: expected {expected}, got {got}")]
    ArityMismatch { expected: usize, got: usize },
    #[error("{msg}")]
    General { msg: String },
    #[error("illegal function call: {msg}")]
    IllegalFunctionCall { msg: String },
    #[error("duplicate definition: {name}")]
    DuplicateDefinition { name: String },
    #[error("{msg}")]
    IoError { msg: String, code: i32 },
}

impl RuntimeError {
    /// Map a RuntimeError to an ANSI BASIC exception type number.
    pub fn ansi_extype(&self) -> i32 {
        match self {
            RuntimeError::DivisionByZero => 3001,
            RuntimeError::TypeMismatch { .. } => 4001,
            RuntimeError::SubscriptOutOfRange => 3000,
            RuntimeError::Overflow { .. } => 1000,
            RuntimeError::IllegalFunctionCall { .. } => 5000,
            RuntimeError::IoError { code, .. } => {
                match *code {
                    53 => 8001, // FileNotFound
                    _ => 8000 + *code,
                }
            }
            _ => 9999,
        }
    }

    /// Create an IoError from a std::io::Error with appropriate error code.
    pub fn from_io(context: &str, e: std::io::Error) -> Self {
        RuntimeError::IoError {
            msg: format!("{} error: {}", context, e),
            code: io_error_to_basic_code(&e),
        }
    }
}

/// Map a std::io::Error to an ANSI BASIC-compatible error code number.
pub fn io_error_to_basic_code(e: &std::io::Error) -> i32 {
    match e.kind() {
        std::io::ErrorKind::NotFound => 53,
        std::io::ErrorKind::PermissionDenied => 70,
        std::io::ErrorKind::AlreadyExists => 58,
        _ => 76,
    }
}
