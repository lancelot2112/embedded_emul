//! Helpers for building printable representations from bus data.

use crate::soc::system::bus::{BusResult, DataHandle, ext::stream::ByteDataHandleExt};

pub trait StringReprDataHandleExt {
    fn read_hex(&mut self, length: usize) -> BusResult<String>;
    fn read_ascii(&mut self, length: usize) -> BusResult<String>;
}

impl StringReprDataHandleExt for DataHandle {
    fn read_hex(&mut self, length: usize) -> BusResult<String> {
        let mut buf = vec![0u8; length];
        self.read_bytes(&mut buf)?;
        Ok(buf.iter().map(|b| format!("{b:02X}")).collect())
    }

    fn read_ascii(&mut self, length: usize) -> BusResult<String> {
        let mut buf = vec![0u8; length];
        self.read_bytes(&mut buf)?;
        Ok(buf
            .into_iter()
            .map(|b| if b.is_ascii_graphic() { b as char } else { '.' })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{BasicMemory, Device, Endianness};
    use crate::soc::system::bus::DeviceBus;
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
    fn read_hex_produces_uppercase_pairs() {
        let data = [0xDE, 0xAD, 0xBE, 0xEF];
        let mut handle = make_handle(&data);
        let as_hex = handle.read_hex(data.len()).expect("hex");
        assert_eq!(as_hex, "DEADBEEF");
    }

    #[test]
    fn read_ascii_masks_non_printable() {
        let data = [b'A', 0x00, b'Z'];
        let mut handle = make_handle(&data);
        let text = handle.read_ascii(data.len()).expect("ascii");
        assert_eq!(text, "A.Z");
    }
}
