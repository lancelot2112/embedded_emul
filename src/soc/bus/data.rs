//! Direct memory access wrapper layered on DeviceHandle offering scalar helpers
//! and std::io traits for interacting with DeviceBus-backed memory regions.
//! Handles device specifics and exposes a consistent BusResult error surface.
use super::{error::BusResult, handle::DeviceHandle};

use crate::soc::{
    device::{AccessContext,Endianness},
};

pub struct DataView {
    handle: DeviceHandle,
    context: AccessContext,
}

impl DataView {
    pub fn new(handle: DeviceHandle, context: AccessContext) -> Self {
        Self {
            handle,
            context,
        }
    }

    #[inline(always)]
    pub fn peek(&mut self, out: &mut [u8]) -> BusResult<()> {
        self.handle.peek(out)
    }

    #[inline(always)]
    pub fn read(&mut self, out: &mut [u8]) -> BusResult<()> {
        self.handle.read(out, self.context)
    }

    #[inline(always)]
    pub fn write(&mut self, data: &[u8]) -> BusResult<()> {
        self.handle.write(data, self.context)
    }

    pub fn peek_u8(&mut self) -> BusResult<u8> {
        let mut buf = [0u8; 1];
        self.peek(&mut buf)?;
        Ok(buf[0])
    }

    pub fn read_u8(&mut self) -> BusResult<u8> {
        let mut buf = [0u8; 1];
        self.read(&mut buf)?;
        Ok(buf[0])
    }

    fn u16_from_bytes(&self, buf: [u8; 2]) -> u16 {
        match self.handle.get_endianness() {
            Endianness::Big => u16::from_be_bytes(buf),
            Endianness::Little => u16::from_le_bytes(buf),
        }
    }

    fn u32_from_bytes(&self, buf: [u8; 4]) -> u32 {
        match self.handle.get_endianness() {
            Endianness::Big => u32::from_be_bytes(buf),
            Endianness::Little => u32::from_le_bytes(buf),
        }
    }

    fn u64_from_bytes(&self, buf: [u8; 8]) -> u64 {
        match self.handle.get_endianness() {
            Endianness::Big => u64::from_be_bytes(buf),
            Endianness::Little => u64::from_le_bytes(buf),
        }
    }

    pub fn peek_u16(&mut self) -> BusResult<u16> {
        let mut buf = [0u8; 2];
        self.peek(&mut buf)?;
        Ok(self.u16_from_bytes(buf))
    }

    pub fn peek_u32(&mut self) -> BusResult<u32> {
        let mut buf = [0u8; 4];
        self.peek(&mut buf)?;
        Ok(self.u32_from_bytes(buf))
    }

    pub fn peek_u64(&mut self) -> BusResult<u64> {
        let mut buf = [0u8; 8];
        self.peek(&mut buf)?;
        Ok(self.u64_from_bytes(buf))
    }

    pub fn read_u16(&mut self) -> BusResult<u16> {
        let mut buf = [0u8; 2];
        self.read(&mut buf)?;
        Ok(self.u16_from_bytes(buf))
    }

    pub fn read_u32(&mut self) -> BusResult<u32> {
        let mut buf = [0u8; 4];
        self.read(&mut buf)?;
        Ok(self.u32_from_bytes(buf))
    }

    pub fn read_u64(&mut self) -> BusResult<u64> {
        let mut buf = [0u8; 8];
        self.read(&mut buf)?;
        Ok(self.u64_from_bytes(buf))
    }

    pub fn write_u8(&mut self, value: u8) -> BusResult<()> {
        let buf = [value];
        self.write(&buf)
    }

    pub fn write_u16(&mut self, value: u16) -> BusResult<()> {
        let buf = match self.handle.get_endianness() {
            Endianness::Big => value.to_be_bytes(),
            Endianness::Little => value.to_le_bytes(),
        };
        self.write(&buf)
    }

    pub fn write_u32(&mut self, value: u32) -> BusResult<()> {
        let buf = match self.handle.get_endianness() {
            Endianness::Big => value.to_be_bytes(),
            Endianness::Little => value.to_le_bytes(),
        };
        self.write(&buf)
    }

    pub fn write_u64(&mut self, value: u64) -> BusResult<()> {
        let buf = match self.handle.get_endianness() {
            Endianness::Big => value.to_be_bytes(),
            Endianness::Little => value.to_le_bytes(),
        };
        self.write(&buf)
    }

    pub fn get_handle_mut(&mut self) -> &mut DeviceHandle {
        &mut self.handle
    }

    pub fn get_handle(&self) -> &DeviceHandle {
        &self.handle
    }
}

#[cfg(test)]
mod tests {
    use crate::soc::bus::{DataView, DeviceBus};
    use crate::soc::device::{AccessContext, Endianness, RamMemory};

    #[test]
    fn read_write_round_trip() {
        let mut bus = DeviceBus::new();
        let memory = RamMemory::new("ram", 0x1000, Endianness::Little);
        bus.map_device(memory, 0x1000, 0).unwrap();

        let be_memory = RamMemory::new("be_ram", 0x1000, Endianness::Big);
        bus.map_device(be_memory, 0x2000, 0).unwrap();

        let handle = bus.resolve(0x1000).expect("valid address");
        let mut little = DataView::new(handle, AccessContext::CPU);
        little
            .write_u32(0xDEADBEEF)
            .expect("write succeeds on little-endian device");
        assert_eq!(
            little.get_handle().get_position(),
            4,
            "writes should advance the underlying cursor"
        );
        little.get_handle_mut().seek(0).expect("reset cursor to start");
        let value = little.read_u32().expect("read back little-endian value");
        assert_eq!(value, 0xDEADBEEF, "round trip should match written data");

        let handle = bus.resolve(0x2000).expect("valid address");
        let mut big = DataView::new(handle, AccessContext::CPU);
        big.write_u32(0x01020304)
            .expect("write succeeds on big-endian device");
        big.get_handle_mut().seek(0).expect("reset cursor to start");
        let value = big.read_u32().expect("read back big-endian value");
        assert_eq!(value, 0x01020304, "round trip should respect endianness");
    }
}
