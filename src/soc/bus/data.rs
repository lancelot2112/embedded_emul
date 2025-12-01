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
    pub fn read(&mut self, out: &mut [u8]) -> BusResult<()> {
        self.handle.read(out, self.context)
    }

    #[inline(always)]
    pub fn write(&mut self, data: &[u8]) -> BusResult<()> {
        self.handle.write(data, self.context)
    }

    pub fn read_u8(&mut self) -> BusResult<u8> {
        let mut buf = [0u8; 1];
        let _ = self.read(&mut buf)?;
        Ok(buf[0])
    }

    pub fn read_u16(&mut self) -> BusResult<u16> {
        let mut buf = [0u8; 2];
        self.read(&mut buf)?;
        match self.handle.get_endianness() {
            Endianness::Big => Ok(u16::from_be_bytes(buf)),
            Endianness::Little => Ok(u16::from_le_bytes(buf)),
        }
    }

    pub fn read_u32(&mut self) -> BusResult<u32> {
        let mut buf = [0u8; 4];
        self.read(&mut buf)?;
        match self.handle.get_endianness() {
            Endianness::Big => Ok(u32::from_be_bytes(buf)),
            Endianness::Little => Ok(u32::from_le_bytes(buf)),
        }
    }

    pub fn read_u64(&mut self) -> BusResult<u64> {
        let mut buf = [0u8; 8];
        self.read(&mut buf)?;
        match self.handle.get_endianness() {
            Endianness::Big => Ok(u64::from_be_bytes(buf)),
            Endianness::Little => Ok(u64::from_le_bytes(buf)),
        }
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
        let bus = DeviceBus::new();
        let memory = RamMemory::new("ram", 0x1000, Endianness::Little);
        bus.map_device(memory, 0x1000, 0).unwrap();

        let be_memory = RamMemory::new("be_ram", 0x1000, Endianness::Big);
        bus.map_device(be_memory, 0x2000, 0).unwrap();

        let mut handle = bus.resolve(0x1000).expect("valid address");
        let mut cursor = DataView::new(handle, AccessContext::CPU);
        cursor.write_u32(0xDEADBEEF).expect("write succeeds");
        assert_eq!(cursor.get_handle().get_position(), 4, "cursor should advance");

        let mut handle = bus.resolve(0x1000).expect("valid address");

        {
            let mut scalar = cursor.scalar_handle(4).expect("pin is valid");
            scalar.write(0xDEADBEEF).expect("write succeeds");
            let cached = scalar.read().expect("read cached value");
            assert_eq!(cached, 0xDEADBEEF, "cached value matches written");
        }

        {
            let mut 
            scalar.write(0xDEADBEEF).expect("write succeeds");
            let cached = scalar.read().expect("read cached value");
            assert_eq!(cached, 0xDEADBEEF, "cached value matches written");
        }
        assert_eq!(
            cursor.bus_address(),
            Some(0x1004),
            "cursor should advance by the scalar size"
        );
        cursor.jump(0x1000).unwrap();
        let value = cursor
            .scalar_handle(4)
            .expect("pin is valid")
            .read()
            .expect("read succeeds");
        assert_eq!(
            value, 0xDEADBEEF,
            "scalar helper should read the written value on big-endian device"
        );

        cursor.jump(0x2000).expect("valid address");
        {
            let mut scalar = cursor.scalar_handle(4).expect("pin is valid");
            scalar.write(0xDEADBEEF).expect("write succeeds");
            let cached = scalar.read().expect("read cached value");
            assert_eq!(cached, 0xDEADBEEF, "cached value matches written");
        }
        assert_eq!(
            cursor.bus_address(),
            Some(0x2004),
            "cursor should advance by the scalar size"
        );
        cursor.jump(0x2000).unwrap();
        let value = cursor
            .scalar_handle(4)
            .expect("pin is valid")
            .read()
            .expect("read succeeds");
        assert_eq!(
            value, 0xDEADBEEF,
            "scalar helper should read the written value on big-endian device"
        );
    }
}
