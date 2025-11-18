//! SymbolValue enum definitions plus shared error types exposed to bus consumers.

use crate::soc::system::bus::BusError;

#[derive(Clone, Debug, PartialEq)]
pub enum SymbolValue {
    Unsigned(u64),
    Signed(i64),
    Float(f64),
    Utf8(String),
    Enum { label: Option<String>, value: i64 },
    Bytes(Vec<u8>),
}

#[derive(Debug)]
pub enum SymbolAccessError {
    MissingAddress { label: String },
    MissingSize { label: String },
    Bus(BusError),
    UnsupportedTraversal { label: String },
}

impl From<BusError> for SymbolAccessError {
    fn from(value: BusError) -> Self {
        SymbolAccessError::Bus(value)
    }
}

impl std::fmt::Display for SymbolAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolAccessError::MissingAddress { label } => {
                write!(f, "symbol '{label}' has no runtime or file address")
            }
            SymbolAccessError::MissingSize { label } => {
                write!(f, "symbol '{label}' has no byte size or sized type metadata")
            }
            SymbolAccessError::Bus(err) => err.fmt(f),
            SymbolAccessError::UnsupportedTraversal { label } => {
                write!(f, "symbol '{label}' has no type metadata to drive traversal")
            }
        }
    }
}

impl std::error::Error for SymbolAccessError {}
