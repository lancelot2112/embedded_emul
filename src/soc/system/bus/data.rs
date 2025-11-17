use std::{
    io::{self, Read, Write},
    sync::Arc,
};

use super::{
    address::AddressHandle,
    bus::DeviceBus,
    error::{BusError, BusResult},
};

pub struct DataHandle {
    address: AddressHandle,
}

impl DataHandle {
    pub fn new(bus: Arc<DeviceBus>) -> Self {
        Self {
            address: AddressHandle::new(bus),
        }
    }

    pub fn address(&self) -> &AddressHandle {
        &self.address
    }

    pub fn address_mut(&mut self) -> &mut AddressHandle {
        &mut self.address
    }

    pub fn available(&self, size: u64) -> bool {
        self.address.available(size)
    }

    pub fn get_u8(&mut self) -> BusResult<u8> {
        self.address
            .transact(1, |device, offset| device.read_u8(offset))
    }

    pub fn set_u8(&mut self, value: u8) -> BusResult<()> {
        self.address
            .transact(1, |device, offset| device.write_u8(offset, value))
    }

    pub fn get_u16(&mut self) -> BusResult<u16> {
        self.address
            .transact(2, |device, offset| device.read_u16(offset))
    }

    pub fn set_u16(&mut self, value: u16) -> BusResult<()> {
        self.address
            .transact(2, |device, offset| device.write_u16(offset, value))
    }

    pub fn get_u32(&mut self) -> BusResult<u32> {
        self.address
            .transact(4, |device, offset| device.read_u32(offset))
    }

    pub fn set_u32(&mut self, value: u32) -> BusResult<()> {
        self.address
            .transact(4, |device, offset| device.write_u32(offset, value))
    }

    pub fn get_u64(&mut self) -> BusResult<u64> {
        self.address
            .transact(8, |device, offset| device.read_u64(offset))
    }

    pub fn set_u64(&mut self, value: u64) -> BusResult<()> {
        self.address
            .transact(8, |device, offset| device.write_u64(offset, value))
    }

    pub fn read_bytes(&mut self, out: &mut [u8]) -> BusResult<()> {
        let len = out.len() as u64;
        if len == 0 {
            return Ok(());
        }
        self.address
            .transact(len, |device, offset| {
                device.read(offset, out)?;
                Ok(())
            })
    }

    pub fn write_bytes(&mut self, data: &[u8]) -> BusResult<()> {
        let len = data.len() as u64;
        if len == 0 {
            return Ok(());
        }
        self.address
            .transact(len, |device, offset| {
                device.write(offset, data)?;
                Ok(())
            })
    }
}

fn io_error(err: BusError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

impl Read for DataHandle {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let available = self.address.bytes_to_end();
        if available == 0 {
            return Ok(0);
        }
        let count = available.min(buf.len() as u64) as usize;
        self.read_bytes(&mut buf[..count]).map_err(io_error)?;
        Ok(count)
    }
}

impl Write for DataHandle {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let available = self.address.bytes_to_end();
        if available == 0 {
            return Ok(0);
        }
        let count = available.min(buf.len() as u64) as usize;
        self.write_bytes(&buf[..count]).map_err(io_error)?;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::system::bus::{BasicMemory, Device, DeviceBus, Endianness};

    #[test]
    fn scalar_read_write_round_trip() {
        let bus = Arc::new(DeviceBus::new(12));
        let memory = Arc::new(BasicMemory::new("ram", 0x1000, Endianness::Little));
        bus.register_device(memory, 0x1000).unwrap();

        let mut handle = DataHandle::new(bus.clone());
        handle.address_mut().jump(0x1000).unwrap();
        handle.set_u32(0xDEADBEEF).unwrap();
        handle.address_mut().jump(0x1000).unwrap();
        assert_eq!(handle.get_u32().unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn redirect_allows_alias_reads() {
        let bus = Arc::new(DeviceBus::new(10));
        let memory = Arc::new(BasicMemory::new("flash", 0x2000, Endianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();

        memory
            .write(0x150, &[0x12, 0x34, 0x56, 0x78])
            .unwrap();
        bus.redirect(0x4000, 4, 0x150).unwrap();

        let mut handle = DataHandle::new(bus);
        handle.address_mut().jump(0x4000).unwrap();
        assert_eq!(handle.get_u32().unwrap(), 0x78563412);
    }
}