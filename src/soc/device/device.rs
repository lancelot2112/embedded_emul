//! Defines the `Device` trait used by the system bus. Devices expose their
//! memory span and provide typed read/write helpers with a consistent
//! `DeviceResult` error surface so bus code can translate failures into
//! `BusError::DeviceFault`.
use std::{ops::Range, sync::{RwLockReadGuard, RwLockWriteGuard}};

use super::{endianness::Endianness, error::DeviceResult};

pub trait Device: Send + Sync {
    fn name(&self) -> &str;
    fn span(&self) -> Range<usize>;
    fn endianness(&self) -> Endianness {
        Endianness::Little
    }
    
    /// Reserve a byte range on the device for atomic access.
    /// Default implementation is a no-op.
    fn reserve(&self, _byte_offset: usize, _len: usize) -> DeviceResult<()> {
        Ok(())
    }

    /// Commit a previously reserved byte range on the device.
    /// Default implementation is a no-op.
    fn commit(&self, _byte_offset: usize) -> DeviceResult<()> {
        Ok(())
    }

    fn borrow(&self, byte_offset: usize, len: usize) -> DeviceResult<RwLockReadGuard<'_, Vec<u8>>>;
    fn borrow_mut(&self, byte_offset: usize, len: usize) -> DeviceResult<RwLockWriteGuard<'_, Vec<u8>>>;

    /// Read a contiguous slice of bytes from the device at `byte_offset` into `out`.
    fn read(&self, byte_offset: usize, out: &mut [u8]) -> DeviceResult<()>;

    /// Write a contiguous slice of bytes to the device at `byte_offset` from `data`.
    fn write(&self, byte_offset: usize, data: &[u8]) -> DeviceResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{DeviceError, Endianness};

    #[derive(Default)]
    struct FaultyDevice;

    impl Device for FaultyDevice {
        fn name(&self) -> &str {
            "faulty"
        }

        fn span(&self) -> Range<usize> {
            0..4
        }

        fn endianness(&self) -> Endianness {
            Endianness::Little
        }

        fn borrow(&self, _byte_offset: usize, _len: usize) -> DeviceResult<RwLockReadGuard<'_, Vec<u8>>> {
            Err(DeviceError::Unsupported("borrow"))
        }

        fn borrow_mut(&self, _byte_offset: usize, _len: usize) -> DeviceResult<RwLockWriteGuard<'_, Vec<u8>>> {
            Err(DeviceError::Unsupported("borrow_mut"))
        }

        fn read(&self, _byte_offset: usize, _out: &mut [u8]) -> DeviceResult<()> {
            Err(DeviceError::Unsupported("read"))
        }

        fn write(&self, _byte_offset: usize, _data: &[u8]) -> DeviceResult<()> {
            Err(DeviceError::Unsupported("write"))
        }
    }

    #[test]
    fn trait_helpers_propagate_device_errors() {
        let dev = FaultyDevice;
        let mut buf = [0u8; 4];
        assert!(
            dev.read(0, &mut buf).is_err(),
            "read should surface backend errors"
        );
        assert!(
            dev.write(0, &buf).is_err(),
            "write should surface backend errors"
        );
    }
}
