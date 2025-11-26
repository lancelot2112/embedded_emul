use std::{error::Error, fmt};

pub type DeviceResult<T> = Result<T, DeviceError>;

#[derive(Debug)]
pub enum DeviceError {
    OutOfRange {
        offset: usize,
        len: usize,
        capacity: usize,
    },
    Unsupported(&'static str),
    Backend(Box<dyn Error + Send + Sync>),
    LockPoisoned(String),
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceError::OutOfRange {
                offset,
                len,
                capacity,
            } => {
                write!(
                    f,
                    "device access offset 0x{offset:016X} len {len} exceeds capacity 0x{capacity:016X}"
                )
            }
            DeviceError::Unsupported(msg) => write!(f, "device operation unsupported: {msg}"),
            DeviceError::Backend(_) => write!(f, "device backend error"),
            DeviceError::LockPoisoned(msg) => write!(f, "device lock poisoned: {msg}"),
        }
    }
}

impl Error for DeviceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DeviceError::Backend(source) => Some(source.as_ref()),
            _ => None,
        }
    }
}
