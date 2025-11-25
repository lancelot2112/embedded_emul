use std::fmt;

use crate::soc::isa::diagnostic::{DiagnosticPhase, IsaDiagnostic};
use crate::soc::bus::error::BusError;

/// Represents any failure that can occur while loading, parsing, validating, or executing ISA
/// artifacts.
pub enum IsaError {
    Io(std::io::Error),
    Parser(String),
    Validation(String),
    IncludeLoop {
        chain: Vec<String>,
    },
    Machine(String),
    Diagnostics {
        phase: DiagnosticPhase,
        diagnostics: Vec<IsaDiagnostic>,
    },
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
            IsaError::Parser(msg) => write!(f, "parser error: {msg}"),
            IsaError::Validation(msg) => write!(f, "validation error: {msg}"),
            IsaError::IncludeLoop { chain } => write!(f, "cyclic include detected: {chain:?}"),
            IsaError::Machine(msg) => write!(f, "machine construction error: {msg}"),
            IsaError::Diagnostics { phase, diagnostics } => {
                writeln!(f, "{phase:?} produced {} issue(s):", diagnostics.len())?;
                for diag in diagnostics {
                    writeln!(f, "  - {}", diag.format_human())?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for IsaError {}

impl fmt::Debug for IsaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}
