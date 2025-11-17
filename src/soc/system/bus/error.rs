use std::{error::Error, fmt};

pub type BusResult<T> = Result<T, BusError>;

#[derive(Debug)]
pub enum BusError {
    NotMapped { address: u64 },
    Overlap { address: u64, details: String },
    RedirectInvalid { source: u64, size: u64, target: u64, reason: &'static str },
    DeviceFault { device: String, source: Box<dyn Error + Send + Sync> },
    OutOfRange { address: u64, end: u64 },
    InvalidDeviceSpan { device: String },
    HandleNotPositioned,
}

impl fmt::Display for BusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BusError::NotMapped { address } => write!(f, "address 0x{address:016X} is not mapped"),
            BusError::Overlap { address, details } => write!(f, "address 0x{address:016X} overlaps existing mapping ({details})"),
            BusError::RedirectInvalid { source, size, target, reason } => {
                let end = source.saturating_add(*size);
                write!(f, "redirect 0x{source:016X}..0x{end:016X} -> 0x{target:016X} invalid: {reason}")
            }
            BusError::DeviceFault { device, .. } => write!(f, "device '{device}' reported a fault"),
            BusError::OutOfRange { address, end } => write!(f, "address 0x{address:016X} exceeds mapping end 0x{end:016X}"),
            BusError::InvalidDeviceSpan { device } => write!(f, "device '{device}' reported an invalid span"),
            BusError::HandleNotPositioned => write!(f, "address handle has not been positioned with jump()"),
        }
    }
}

impl Error for BusError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            BusError::DeviceFault { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
