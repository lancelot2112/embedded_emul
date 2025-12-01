//! Defines the `Device` trait used by the system bus. Devices expose their
//! memory span and provide typed read/write helpers with a consistent
//! `DeviceResult` error surface so bus code can translate failures into
//! `BusError::DeviceFault`.
use std::ops::Range;

use crate::soc::device::{AccessContext, RamMemory};

use super::{endianness::Endianness, error::DeviceResult};

pub trait Device: Send + Sync {
    fn name(&self) -> &str;
    fn span(&self) -> Range<usize>;

    #[inline(always)]
    fn endianness(&self) -> Endianness {
        Endianness::Little
    }

    // Fast path pointer access
    fn as_ram(&self) -> Option<&RamMemory> {
        None
    }
    fn as_ram_mut(&mut self) -> Option<&mut RamMemory> {
        None
    }

    /// Read a contiguous slice of bytes from the device at `byte_offset` into `out`.
    /// Reads may mutate if the device has side effects on read (clear bit on read)
    fn read(&mut self, offset: usize, out: &mut [u8], ctx: AccessContext) -> DeviceResult<()>;

    /// Write a contiguous slice of bytes to the device at `byte_offset` from `data`.
    fn write(&mut self, offset: usize, data: &[u8], ctx: AccessContext) -> DeviceResult<()>;
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

        fn read(
            &mut self,
            _byte_offset: usize,
            _out: &mut [u8],
            _ctx: AccessContext,
        ) -> DeviceResult<()> {
            Err(DeviceError::Unsupported("read"))
        }

        fn write(
            &mut self,
            _byte_offset: usize,
            _data: &[u8],
            _ctx: AccessContext,
        ) -> DeviceResult<()> {
            Err(DeviceError::Unsupported("write"))
        }
    }

    #[test]
    fn trait_helpers_propagate_device_errors() {
        let mut dev = FaultyDevice;
        let mut buf = [0u8; 4];
        assert!(
            dev.read(0, &mut buf, AccessContext::CPU).is_err(),
            "read should surface backend errors"
        );
        assert!(
            dev.write(0, &buf, AccessContext::CPU).is_err(),
            "write should surface backend errors"
        );
    }
}
