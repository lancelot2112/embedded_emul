//! Defines the `Device` trait used by the system bus. Devices expose their
//! memory span and provide typed read/write helpers with a consistent
//! `DeviceResult` error surface so bus code can translate failures into
//! `BusError::DeviceFault`.
use std::ops::Range;

use super::{endianness::Endianness, error::DeviceResult};

pub trait Device: Send + Sync {
    fn name(&self) -> &str;
    fn span(&self) -> Range<u64>;
    fn endianness(&self) -> Endianness {
        Endianness::Little
    }

    fn read(&self, offset: u64, buf: &mut [u8]) -> DeviceResult<()>;
    fn write(&self, offset: u64, data: &[u8]) -> DeviceResult<()>;

    fn read_u8(&self, offset: u64) -> DeviceResult<u8> {
        let mut buf = [0_u8; 1];
        self.read(offset, &mut buf)?;
        Ok(buf[0])
    }

    fn write_u8(&self, offset: u64, value: u8) -> DeviceResult<()> {
        let buf = [value];
        self.write(offset, &buf)
    }

    fn read_u16(&self, offset: u64) -> DeviceResult<u16> {
        let mut buf = [0_u8; 2];
        self.read(offset, &mut buf)?;
        Ok(self.endianness().read_u16(buf))
    }

    fn write_u16(&self, offset: u64, value: u16) -> DeviceResult<()> {
        let buf = self.endianness().write_u16(value);
        self.write(offset, &buf)
    }

    fn read_u32(&self, offset: u64) -> DeviceResult<u32> {
        let mut buf = [0_u8; 4];
        self.read(offset, &mut buf)?;
        Ok(self.endianness().read_u32(buf))
    }

    fn write_u32(&self, offset: u64, value: u32) -> DeviceResult<()> {
        let buf = self.endianness().write_u32(value);
        self.write(offset, &buf)
    }

    fn read_u64(&self, offset: u64) -> DeviceResult<u64> {
        let mut buf = [0_u8; 8];
        self.read(offset, &mut buf)?;
        Ok(self.endianness().read_u64(buf))
    }

    fn write_u64(&self, offset: u64, value: u64) -> DeviceResult<()> {
        let buf = self.endianness().write_u64(value);
        self.write(offset, &buf)
    }

    fn read_endianed_bytes(&self, offset: u64, out: &mut [u8]) -> DeviceResult<()> {
        self.read(offset, out)?;
        let ordered = self.endianness().read_bytes(out);
        out.copy_from_slice(&ordered);
        Ok(())
    }

    fn write_endianed_bytes(&self, offset: u64, data: &[u8]) -> DeviceResult<()> {
        let ordered = self.endianness().write_bytes(data);
        self.write(offset, &ordered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{BasicMemory, DeviceError, DeviceResult};

    #[test]
    fn scalar_helpers_follow_endianness() {
        let mem = BasicMemory::new("mem", 16, Endianness::Little);
        mem.write_u32(0, 0xAABB_CCDD).expect("write_u32 little");
        assert_eq!(
            mem.read_u32(0).unwrap(),
            0xAABB_CCDD,
            "little-endian helper should round-trip value"
        );

        let mem_be = BasicMemory::new("mem_be", 16, Endianness::Big);
        mem_be.write_u16(2, 0xCAFE).expect("write big-endian");
        assert_eq!(
            mem_be.read_u16(2).unwrap(),
            0xCAFE,
            "big-endian helper should round-trip value"
        );
    }

    #[test]
    fn read_write_guard_against_out_of_range() {
        let mem = BasicMemory::new("mem", 8, Endianness::Little);
        let err = mem
            .read_u64(4)
            .expect_err("read beyond capacity should fail");
        match err {
            DeviceError::OutOfRange {
                offset, capacity, ..
            } => {
                assert_eq!(offset, 4, "offset should report access start");
                assert_eq!(capacity, 8, "capacity reflects device span");
            }
            other => panic!("Unexpected error variant: {other:?}"),
        }
    }

    #[derive(Default)]
    struct FaultyDevice;

    impl Device for FaultyDevice {
        fn name(&self) -> &str {
            "faulty"
        }

        fn span(&self) -> Range<u64> {
            0..4
        }

        fn read(&self, _offset: u64, _buf: &mut [u8]) -> DeviceResult<()> {
            Err(DeviceError::Unsupported("read"))
        }

        fn write(&self, _offset: u64, _data: &[u8]) -> DeviceResult<()> {
            Err(DeviceError::Unsupported("write"))
        }
    }

    #[test]
    fn trait_helpers_propagate_device_errors() {
        let dev = FaultyDevice;
        assert!(
            dev.read_u8(0).is_err(),
            "read_u8 should surface backend errors"
        );
        assert!(
            dev.write_u8(0, 0xAA).is_err(),
            "write_u8 should surface backend errors"
        );
    }
}
