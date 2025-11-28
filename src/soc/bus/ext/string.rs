//! UTF-8 helpers layered on top of `DataHandle` mirroring the string utilities from the .NET data bus extensions.

use std::borrow::Cow;

use crate::soc::bus::{
    BusResult, DataHandle,
    ext::{int::IntDataHandleExt, stream::ByteDataHandleExt},
};

pub trait StringDataHandleExt {
    /// Reads a fixed-length UTF-8 string, stopping early at the first nul byte.
    fn read_utf8(&mut self, len: usize) -> BusResult<String>;

    /// Reads until a terminating nul or `max_len` bytes have been consumed.
    fn read_c_string(&mut self, max_len: usize) -> BusResult<String>;
}

impl StringDataHandleExt for DataHandle {
    fn read_utf8(&mut self, len: usize) -> BusResult<String> {
        let mut buf = vec![0u8; len];
        self.stream_out(&mut buf)?;
        Ok(trim_nul(Cow::Borrowed(buf.as_slice())).into_owned())
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
    use super::*;
    use crate::soc::device::{RamMemory, Device, Endianness};
    use crate::soc::bus::DeviceBus;
    use std::sync::Arc;

    fn prepare_handle(bytes: &[u8]) -> DataHandle {
        let bus = Arc::new(DeviceBus::new(8));
        let memory = Arc::new(RamMemory::new("rom", 0x40, Endianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();
        memory.write(0, bytes).unwrap();
        let mut handle = DataHandle::new(bus);
        handle.address_mut().jump(0).unwrap();
        handle
    }

    #[test]
    fn read_utf8_honors_length_and_trims_nul() {
        let mut handle = prepare_handle(b"RPM\0garbage");
        let text = handle.read_utf8(8).expect("utf8");
        assert_eq!(text, "RPM", "helper should trim trailing nul terminator");
    }

    #[test]
    fn c_string_stops_at_terminator() {
        let mut handle = prepare_handle(b"hello\0world");
        let text = handle.read_c_string(32).expect("cstring");
        assert_eq!(text, "hello", "reader should stop at the first nul byte");
    }
}
