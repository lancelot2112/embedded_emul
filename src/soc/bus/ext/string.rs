//! UTF-8 helpers layered on top of `DataHandle` mirroring the string utilities from the .NET data bus extensions.
use std::borrow::Cow;

use crate::soc::bus::{BusCursor, BusResult};

pub trait StringCursorExt {
    /// Reads a fixed-length UTF-8 string, stopping early at the first nul byte.
    fn read_utf8(&mut self, len: usize) -> BusResult<String>;

    /// Reads until a terminating nul or `max_len` bytes have been consumed.
    fn read_c_string(&mut self, max_len: usize) -> BusResult<String>;
}

impl StringCursorExt for BusCursor {
    fn read_utf8(&mut self, len: usize) -> BusResult<String> {
        let buf = self.read_ram(len)?;
        Ok(trim_nul(Cow::Borrowed(buf)).into_owned())
    }

    fn read_c_string(&mut self, max_len: usize) -> BusResult<String> {
        let mut bytes = Vec::new();
        for _ in 0..max_len {
            let byte = self.read_u8()?;
            if byte == 0 {
                break;
            }
            bytes.push(byte);
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

fn trim_nul(data: Cow<'_, [u8]>) -> Cow<'_, str> {
    match data {
        Cow::Borrowed(bytes) => {
            let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
            Cow::Owned(String::from_utf8_lossy(&bytes[..end]).into_owned())
        }
        Cow::Owned(vec) => {
            let end = vec.iter().position(|b| *b == 0).unwrap_or(vec.len());
            Cow::Owned(String::from_utf8_lossy(&vec[..end]).into_owned())
        }
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
    fn read_utf8_honors_length_and_trims_nul() {
        let mut cursor = make_cursor(b"RPM\0garbage");
        let text = cursor.read_utf8(8).expect("utf8");
        assert_eq!(text, "RPM", "helper should trim trailing nul terminator");
    }

    #[test]
    fn c_string_stops_at_terminator() {
        let mut cursor = make_cursor(b"hello\0world");
        let text = cursor.read_c_string(32).expect("cstring");
        assert_eq!(text, "hello", "reader should stop at the first nul byte");
    }
}
