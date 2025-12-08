//! Helpers for building printable representations from bus data.

use crate::soc::bus::{BusCursor, BusResult};

pub trait StringReprCursorExt {
    fn read_hex(&mut self, length: usize) -> BusResult<String>;
    fn read_ascii(&mut self, length: usize) -> BusResult<String>;
}

impl StringReprCursorExt for BusCursor {
    fn read_hex(&mut self, length: usize) -> BusResult<String> {
        let buf = self.read_ram(length)?;
        Ok(buf.iter().map(|b| format!("{b:02X}")).collect())
    }

    fn read_ascii(&mut self, length: usize) -> BusResult<String> {
        let buf = self.read_ram(length)?;
        Ok(buf
            .iter()
            .map(|&b| if b.is_ascii_graphic() { b as char } else { '.' })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::soc::bus::DeviceBus;
    use crate::soc::device::{AccessContext, Device, Endianness, RamMemory};

    fn make_cursor(bytes: &[u8]) -> BusCursor {
        let mut bus = DeviceBus::new(32);
        let memory = RamMemory::new("rom", 0x40, Endianness::Little);
        memory.write(0, bytes, AccessContext::DEBUG).unwrap();
        bus.map_device(memory, 0, 0).unwrap();
        BusCursor::attach_to_bus(Arc::new(bus), 0, AccessContext::CPU)
    }

    #[test]
    fn read_hex_produces_uppercase_pairs() {
        let data = [0xDE, 0xAD, 0xBE, 0xEF];
        let mut cursor = make_cursor(&data);
        let as_hex = cursor.read_hex(data.len()).expect("hex");
        assert_eq!(as_hex, "DEADBEEF");
    }

    #[test]
    fn read_ascii_masks_non_printable() {
        let data = [b'A', 0x00, b'Z'];
        let mut cursor = make_cursor(&data);
        let text = cursor.read_ascii(data.len()).expect("ascii");
        assert_eq!(text, "A.Z");
    }
}
