//! Integer helpers that perform width-aware reads and sign/zero extension on top of `DataHandle`.

use crate::soc::bus::{BusError, BusResult, DataHandle};
use super::bits::BitDataHandleExt;

/// Trait adding width-aware integer reads directly on top of `DataHandle`.
pub trait IntDataHandleExt {
    fn read_unsigned(&mut self, bit_off: u8, width_bits: usize) -> BusResult<u64>;
    fn read_signed(&mut self, bit_off: u8, width_bits: usize) -> BusResult<i64>;
    fn write_unsigned(&mut self, bit_off: u8, width_bits: usize, value: u64) -> BusResult<()>;

    fn read_u8(&mut self) -> BusResult<u8>;
    fn read_u16(&mut self) -> BusResult<u16>;
    fn read_u32(&mut self) -> BusResult<u32>;
    fn read_u64(&mut self) -> BusResult<u64>;

    fn write_u8(&mut self, value: u8) -> BusResult<()>;
    fn write_u16(&mut self, value: u16) -> BusResult<()>;
    fn write_u32(&mut self, value: u32) -> BusResult<()>;
    fn write_u64(&mut self, value: u64) -> BusResult<()>;
}

impl IntDataHandleExt for DataHandle {
    fn read_unsigned(&mut self, bit_off: u8, width_bits: usize) -> BusResult<u64> {
        ensure_width(width_bits)?;
        self.read_bits(bit_off, width_bits as u16)
            .map(|value| value as u64)
    }

    fn read_signed(&mut self, bit_off: u8, width_bits: usize) -> BusResult<i64> {
        let value = self.read_unsigned(bit_off, width_bits)?;
        Ok(sign_extend(value, width_bits as u32))
    }

    fn write_unsigned(&mut self, bit_off: u8, width_bits: usize, value: u64) -> BusResult<()> {
        ensure_width(width_bits)?;
        self.write_bits(bit_off, width_bits as u16, value)
    }

    fn read_u8(&mut self) -> BusResult<u8> {
        self.fetch(1).map(|val| val as u8)
    }

    fn read_u16(&mut self) -> BusResult<u16> {
        self.fetch(2).map(|val| val as u16)
    }

    fn read_u32(&mut self) -> BusResult<u32> {
        self.fetch(4).map(|val| val as u32)
    }

    fn read_u64(&mut self) -> BusResult<u64> {
        self.fetch(8)
    }

    fn write_u8(&mut self, value: u8) -> BusResult<()> {
        self.write_data(value as u64, 1)
    }

    fn write_u16(&mut self, value: u16) -> BusResult<()> {
        self.write_data(value as u64, 2)
    }

    fn write_u32(&mut self, value: u32) -> BusResult<()> {
        self.write_data(value as u64, 4)
    }

    fn write_u64(&mut self, value: u64) -> BusResult<()> {
        self.write_data(value, 8)
    }
}

fn sign_extend(value: u64, bits: u32) -> i64 {
    if bits == 0 {
        return 0;
    }
    let shift = 64u32.saturating_sub(bits);
    ((value << shift) as i64) >> shift
}

fn ensure_width(width_bits: usize) -> BusResult<()> {
    if width_bits == 0 || width_bits > 64 {
        return Err(BusError::DeviceFault {
            device: "bus-ext-int".into(),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "integer width must be between 1 and 64 bits",
            )),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{BasicMemory, Endianness as DeviceEndianness};
    use crate::soc::bus::{DeviceBus, ext::stream::ByteDataHandleExt};
    use std::sync::Arc;

    fn make_handle(bytes: &[u8]) -> DataHandle {
        let bus = Arc::new(DeviceBus::new(8));
        let memory = Arc::new(BasicMemory::new("ram", 0x20, DeviceEndianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();
        let mut preload = DataHandle::new(bus.clone());
        preload.address_mut().jump(0).unwrap();
        preload.stream_in(bytes).unwrap();
        let mut handle = DataHandle::new(bus);
        handle.address_mut().jump(0).unwrap();
        handle
    }

    #[test]
    fn read_unsigned_matches_expected_value() {
        let mut handle = make_handle(&[0x34, 0x12, 0, 0]);
        let value = handle.read_unsigned(0, 16).expect("read u16");
        assert_eq!(value, 0x1234, "big-endian decode should match reference");
    }

    #[test]
    fn read_signed_sign_extends_properly() {
        let mut handle = make_handle(&[0x80]);
        let value = handle.read_signed(0, 8).expect("read i8");
        assert_eq!(value, -128, "sign extension should honor the MSB");
    }

    #[test]
    fn write_unsigned_respects_bit_offset() {
        let mut handle = make_handle(&[0; 2]);
        handle.write_unsigned(4, 8, 0xAB).expect("write field");
        handle.address_mut().jump(0).unwrap();
        let raw = handle.read_unsigned(0, 16).expect("read word");
        assert_eq!(raw, 0x0AB0, "write should update the correct bit window");

        handle.address_mut().jump(0).unwrap();
        let field = handle.read_unsigned(4, 8).expect("read field");
        assert_eq!(field, 0xAB, "field should retain the written value");
    }
}
