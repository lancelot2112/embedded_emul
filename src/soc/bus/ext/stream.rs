//! Stream-style adapters and byte-chunk helpers layered on top of `DataHandle`.
use std::io::{self, Read, Write};
use crate::soc::device::endianness::MAX_ENDIAN_BYTES;
use crate::soc::bus::{BusError, BusResult, DataHandle};


/// Byte convenience helpers so callers can read/write large buffers without
/// replicating the MAX_ENDIAN_BYTES chunking logic every time.
pub trait ByteDataHandleExt {
    fn stream_out(&mut self, out: &mut [u8]) -> BusResult<()>;
    fn stream_in(&mut self, data: &[u8]) -> BusResult<()>;
}

impl ByteDataHandleExt for DataHandle {
    fn stream_out(&mut self, out: &mut [u8]) -> BusResult<()> {
        if out.is_empty() {
            return Ok(());
        }
        self.address_mut().atomic_byte_transact(out.len(), |device, offset, _resolved| {
            device.read(offset, out).into()
        })?;
        Ok(())
    }

    fn stream_in(&mut self, data: &[u8]) -> BusResult<()> {
        if data.is_empty() {
            return Ok(());
        }
        self.address_mut().atomic_byte_transact(data.len(), |device, offset, _resolved| {
            device.write(offset, data).into()
        })?;
        Ok(())
    }
}

/// Lightweight view that exposes buffered style helpers over a mutable [`DataHandle`].
pub struct DataStream<'a> {
    handle: &'a mut DataHandle,
}

impl<'a> DataStream<'a> {
    pub fn new(handle: &'a mut DataHandle) -> Self {
        Self { handle }
    }

    pub fn read_exact(&mut self, len: usize) -> BusResult<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.handle.stream_out(&mut buf)?;
        Ok(buf)
    }

    pub fn skip(&mut self, len: usize) -> BusResult<()> {
        self.handle.address_mut().advance(len)
    }

    pub fn fill_slice(&mut self, out: &mut [u8]) -> BusResult<()> {
        self.handle.stream_out(out)
    }
}

impl Read for DataHandle {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let available = self.address().bytes_to_end();
        if available == 0 {
            return Ok(0);
        }
        let count = available.min(buf.len()) as usize;
        self.stream_out(&mut buf[..count]).map_err(io_error)?;
        Ok(count)
    }
}

impl Write for DataHandle {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let available = self.address().bytes_to_end();
        if available == 0 {
            return Ok(0);
        }
        let count = available.min(buf.len()) as usize;
        self.stream_in(&buf[..count]).map_err(io_error)?;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn io_error(err: BusError) -> io::Error {
    io::Error::other(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{BasicMemory, Device, Endianness};
    use crate::soc::bus::DeviceBus;
    use std::sync::Arc;

    fn make_handle() -> (DataHandle, Arc<BasicMemory>) {
        let bus = Arc::new(DeviceBus::new(8));
        let memory = Arc::new(BasicMemory::new("ram", 0x40, Endianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();
        let mut handle = DataHandle::new(bus);
        handle.address_mut().jump(0).unwrap();
        (handle, memory)
    }

    #[test]
    fn read_exact_returns_requested_bytes() {
        let (mut handle, memory) = make_handle();
        memory.write(0, &[1, 2, 3, 4]).unwrap();
        let mut stream = DataStream::new(&mut handle);
        let bytes = stream.read_exact(4).expect("read snapshot");
        assert_eq!(bytes, &[1, 2, 3, 4], "stream should copy bytes verbatim");
    }

    #[test]
    fn skip_advances_underlying_cursor() {
        let (mut handle, _) = make_handle();
        let mut stream = DataStream::new(&mut handle);
        stream.skip(8).unwrap();
        assert_eq!(
            handle.address().bus_address(),
            Some(8),
            "skip should advance address cursor"
        );
    }
}
