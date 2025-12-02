use std::{
    error::Error,
    fmt,
    sync::{MutexGuard, PoisonError},
};

use crate::soc::device::{Device, DeviceError};

pub type BusResult<T> = Result<T, BusError>;

#[derive(Debug)]
pub enum BusError {
    NotMapped {
        address: usize,
    },
    Overlap {
        address: usize,
        details: String,
    },
    RedirectInvalid {
        source: usize,
        size: usize,
        target: usize,
        reason: &'static str,
    },
    PageFault {
        details: String,
    },
    DeviceFault {
        device: String,
        source: Box<dyn Error + Send + Sync>,
    },
    HandleOutOfRange {
        offset: usize,
        delta: isize,
    },
    OutOfRange {
        address: usize,
        end: usize,
    },
    InvalidAddress {
        address: usize,
    },
    InvalidDeviceSpan {
        device: String,
    },
    HandleNotPositioned,
    LockPoisoned,
}

impl fmt::Display for BusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BusError::NotMapped { address } => write!(f, "address 0x{address:016X} is not mapped"),
            BusError::Overlap { address, details } => write!(
                f,
                "address 0x{address:016X} overlaps existing mapping ({details})"
            ),
            BusError::RedirectInvalid {
                source,
                size,
                target,
                reason,
            } => {
                let end = source.saturating_add(*size);
                write!(
                    f,
                    "redirect 0x{source:016X}..0x{end:016X} -> 0x{target:016X} invalid: {reason}"
                )
            }
            BusError::DeviceFault { device, .. } => write!(f, "device '{device}' reported a fault"),
            BusError::PageFault { details } => write!(f, "page fault: {details}"),
            BusError::HandleOutOfRange { offset, delta } => write!(
                f,
                "access at offset 0x{offset:016X} + {delta} out of device range"
            ),
            BusError::OutOfRange { address, end } => write!(
                f,
                "address 0x{address:016X} exceeds mapping end 0x{end:016X}"
            ),
            BusError::InvalidDeviceSpan { device } => {
                write!(f, "device '{device}' reported an invalid span")
            }
            BusError::InvalidAddress { address } => {
                write!(
                    f,
                    "address 0x{address:016X} is invalid for the target device"
                )
            }
            BusError::HandleNotPositioned => {
                write!(f, "address handle has not been positioned with jump()")
            }
            BusError::LockPoisoned => {
                write!(f, "bus lock has been poisoned due to a prior error")
            }
        }
    }
}

impl From<PoisonError<MutexGuard<'_, dyn Device + 'static>>> for BusError {
    fn from(_value: PoisonError<MutexGuard<'_, dyn Device + 'static>>) -> Self {
        BusError::LockPoisoned
    }
}

impl From<DeviceError> for BusError {
    fn from(value: DeviceError) -> Self {
        BusError::DeviceFault {
            device: "unknown".into(),
            source: Box::new(value),
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
