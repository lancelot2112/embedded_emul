//! LEB128 read/write helpers reused by symbol and loader tooling.

use crate::soc::bus::{BusResult, DataHandle, ext::int::IntDataHandleExt};

pub trait Leb128DataHandleExt {
    fn read_uleb128(&mut self) -> BusResult<u64>;
    fn read_sleb128(&mut self) -> BusResult<i64>;
}

impl Leb128DataHandleExt for DataHandle {
    fn read_uleb128(&mut self) -> BusResult<u64> {
        let mut result = 0u64;
        let mut shift = 0;
        loop {
            let byte = self.read_u8()?;
            result |= ((byte & 0x7F) as u64) << shift;
            if (byte & 0x80) == 0 {
                break;
            }
            shift += 7;
        }
        Ok(result)
    }

    fn read_sleb128(&mut self) -> BusResult<i64> {
        let mut result = 0i64;
        let mut shift = 0;
        let mut byte;
        loop {
            byte = self.read_u8()? as i64;
            result |= (byte & 0x7F) << shift;
            shift += 7;
            if (byte & 0x80) == 0 {
                break;
            }
        }
        if (shift < 64) && ((byte & 0x40) != 0) {
            result |= !0 << shift;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{BasicMemory, Device, Endianness};
    use crate::soc::bus::DeviceBus;
    use std::sync::Arc;

    fn make_handle(bytes: &[u8]) -> DataHandle {
        let bus = Arc::new(DeviceBus::new(8));
        let memory = Arc::new(BasicMemory::new("rom", 0x20, Endianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();
        memory.write(0, bytes).unwrap();
        let mut handle = DataHandle::new(bus);
        handle.address_mut().jump(0).unwrap();
        handle
    }

    #[test]
    fn read_uleb128_decodes_example() {
        let mut handle = make_handle(&[0xE5, 0x8E, 0x26]);
        let value = handle.read_uleb128().expect("uleb");
        assert_eq!(
            value, 624485,
            "ULEB128 example from DWARF spec should parse"
        );
    }

    #[test]
    fn read_sleb128_decodes_negative_example() {
        let mut handle = make_handle(&[0x9B, 0xF1, 0x59]);
        let value = handle.read_sleb128().expect("sleb");
        assert_eq!(
            value, -624485,
            "SLEB128 example from DWARF spec should parse"
        );
    }
}
