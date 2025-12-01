//! Helpers for building printable representations from bus data.

use crate::soc::bus::{BusResult, CursorBehavior, DataView};

pub trait StringReprDataViewExt {
    fn read_hex(&mut self, length: usize) -> BusResult<String>;
    fn read_ascii(&mut self, length: usize) -> BusResult<String>;
}

impl StringReprDataViewExt for DataView {
    fn read_hex(&mut self, length: usize) -> BusResult<String> {
        let mut buf = vec![0u8; length];
        self.read(&mut buf)?;
        Ok(buf.iter().map(|b| format!("{b:02X}")).collect())
    }

    fn read_ascii(&mut self, length: usize) -> BusResult<String> {
        let mut buf = vec![0u8; length];
        self.read(&mut buf)?;
        Ok(buf
            .into_iter()
            .map(|b| if b.is_ascii_graphic() { b as char } else { '.' })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::bus::{DeviceBus, StaticCursor};
    use crate::soc::device::{AccessContext, Device, Endianness, RamMemory};

    fn make_handle(bytes: &[u8]) -> DataView{
        let mut bus = DeviceBus::new();
        let mut memory = RamMemory::new("rom", 0x40, Endianness::Little);
        memory.write(0, bytes, AccessContext::DEBUG).unwrap();
        bus.map_device(memory, 0, 0).unwrap();
        let handle = bus.resolve(0).unwrap();
        DataView::new(handle, AccessContext::CPU)
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
