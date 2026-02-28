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
    #[error("overflow")]
    Overflow,
    #[error("subscript out of range")]
    SubscriptOutOfRange,
    #[error("RETURN without GOSUB")]
    ReturnWithoutGosub,
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
    #[error("NEXT without FOR")]
    NextWithoutFor,
}
