use std::fmt;

use crate::soc::system::bus::error::BusError;

/// Represents any failure that can occur while loading, parsing, validating, or executing ISA
/// artifacts.
#[derive(Debug)]
pub enum IsaError {
    Io(std::io::Error),
    Lexer(String),
    Parser(String),
    Validation(String),
    IncludeLoop { chain: Vec<String> },
    Machine(String),
}

impl From<std::io::Error> for IsaError {
    fn from(err: std::io::Error) -> Self {
        IsaError::Io(err)
    }
}

impl From<BusError> for IsaError {
    fn from(err: BusError) -> Self {
        IsaError::Machine(format!("bus error: {err}"))
    }
}

impl fmt::Display for IsaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IsaError::Io(err) => write!(f, "I/O error: {err}"),
            IsaError::Lexer(msg) => write!(f, "lexer error: {msg}"),
            IsaError::Parser(msg) => write!(f, "parser error: {msg}"),
            IsaError::Validation(msg) => write!(f, "validation error: {msg}"),
            IsaError::IncludeLoop { chain } => write!(f, "cyclic include detected: {chain:?}"),
            IsaError::Machine(msg) => write!(f, "machine construction error: {msg}"),
        }
    }
}

impl std::error::Error for IsaError {}
