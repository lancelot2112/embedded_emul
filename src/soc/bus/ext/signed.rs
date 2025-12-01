//! Integer helpers that perform width-aware reads and sign/zero extension on top of `DataHandle`.

use crate::soc::bus::{BusResult, CursorBehavior, DataView};

/// Trait adding width-aware integer reads directly on top of `DataHandle`.
pub trait SignedDataViewExt {
    fn read_i8(&mut self) -> BusResult<i8>;
    fn read_i16(&mut self) -> BusResult<i16>;
    fn read_i32(&mut self) -> BusResult<i32>;
    fn read_i64(&mut self) -> BusResult<i64>;
}

impl<C: CursorBehavior> SignedDataViewExt for DataView<C> {
    fn read_i8(&mut self) -> BusResult<i8> {
        let raw = self.read_u8()?;
        Ok(raw as i8)
    }
    fn read_i16(&mut self) -> BusResult<i16> {
        let raw = self.read_u16()?;
        Ok(raw as i16)
    }
    fn read_i32(&mut self) -> BusResult<i32> {
        let raw = self.read_u32()?;
        Ok(raw as i32)
    }
    fn read_i64(&mut self) -> BusResult<i64> {
        let raw = self.read_u64()?;
        Ok(raw as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::bus::{DeviceBus, ext::stream::ByteDataHandleExt};
    use crate::soc::device::{Endianness as DeviceEndianness, RamMemory};

    fn make_handle(bytes: &[u8]) -> DataView {
        let bus = DeviceBus::new();
        let memory = RamMemory::new("ram", 0x20, DeviceEndianness::Little);
        bus.register_device(memory.clone(), 0).unwrap();
        let mut handle = bus.resolve(0).unwrap();
        handle.write::<StaticCursor>(bytes, AccessContext::DEBUG).expect("write preload");
        DataView::<StaticCursor>::new(handle, AccessContext::CPU)
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
