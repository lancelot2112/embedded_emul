//! Direct memory access wrapper layered on AddressHandle offering scalar helpers
//! and std::io traits for interacting with DeviceBus-backed memory regions.
//! Handles device specifics and exposes a consistent BusResult error surface.
use std::{
    sync::Arc,
};

use super::{
    DeviceBus,
    address::AddressHandle,
    error::{BusResult},
};

use crate::soc::{bus::BusError, device::{
    Device, DeviceError, Endianness
}};


pub struct ScalarHandle<'a> {
    data: DataHandle<'a>,
    cache: Option<u64>,
    edits: bool,
}

impl<'a> ScalarHandle<'a> {
    pub fn create(
        data: DataHandle<'a>,
        
    ) -> Self {
        Self {
            data,
            cache: None,
            edits: false,
        }
    }

    pub fn len(&self) -> usize {
        self.data.len
    }

    pub fn fetch(&mut self) -> BusResult<u64> {
        //TODO: read from the underlying pinned range, handle endianness
        let mut buf = [0u8; 8];
        let endian = self.data.device.endianness();
        self.data.read(endian.fill(&mut buf, self.data.len))?;
        let value = endian.to_native_scalar(&buf);
        self.cache = Some(value);
        Ok(value)
    }

    pub fn read(&mut self) -> BusResult<u64> {
        if let Some(cached) = self.cache {
            return Ok(cached);
        }
        Ok(self.fetch()?)
    }

    pub fn write(&mut self, value: u64) -> BusResult<()> {
        //TODO: mark as edited and write to the cached value. Should we add masking? so we can edit subranges of bits? 
        self.edits = true;
        let mask = (1u64.unbounded_shl(self.data.len as u32)).wrapping_sub(1);
        self.cache = Some(value & mask);
        Ok(())
    }

    pub fn commit(&mut self) -> BusResult<()> {
        if !self.edits {
            return Ok(());
        }
        match self.cache {
            None => Err(BusError::HandleNotPositioned),
            Some(value) => {
                //TODO: Add endianness handling to flip to device order then
                //write to the underlying pinned range.
                let endian = self.data.device.endianness();
                let mut bytes = endian.from_native_scalar(value);
                self.data.write(endian.fill(&mut bytes, self.data.len))?;
                self.edits = false;
                Ok(())
            }
        }
    }
}

impl Drop for ScalarHandle<'_> {
    fn drop(&mut self) {
        self.commit().ok();
    }
}

//A pinned range allows for reading/writing to a specific range on a device
//and leaving it reserved to promote some atomicity guarantees.  
pub struct DataHandle<'a> {
    device: &'a dyn Device,
    start: usize,
    len: usize,
}

impl<'a> DataHandle<'a> {
    pub fn create(
        device: &'a dyn Device,
        start: usize,
        len: usize,
    ) -> BusResult<Self> {
        device.lock(start, len)?;
        Ok(Self { device, start, len})
    }

    pub fn len(self) -> usize {
        self.len
    }
    pub fn as_scalar(self) -> ScalarHandle<'a> {
        ScalarHandle::create(self)
    }

    pub fn read(&self, dest: &mut [u8]) -> BusResult<()> {
        if dest.len() > self.len {
            return Err(BusError::OutOfRange {
                address: self.start + dest.len(),
                end: self.start + self.len,
            });
        }
        self.device.read(self.start, dest).map_err(|err| BusError::DeviceFault {
            device: self.device.name().to_string(),
            source: Box::new(err),
        })
    }
   
    pub fn write(&self, src: &[u8]) -> BusResult<()> {
        if src.len() > self.len {
            return Err(BusError::OutOfRange {
                address: self.start + src.len(),
                end: self.start + self.len,
            });
        }
        self.device.write(self.start, src).map_err(|err| BusError::DeviceFault {
            device: self.device.name().to_string(),
            source: Box::new(err),
        })
    }
}

impl Drop for DataHandle<'_> {
    fn drop(&mut self) {
        // ignore errors on drop; handle logs if you need them
        let _ = self.device.unlock(self.start);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{RamMemory, Endianness};
    use crate::soc::bus::DeviceBus;

    #[test]
    fn read_write_round_trip() {
        let bus = Arc::new(DeviceBus::new(12));
        let memory = Arc::new(RamMemory::new("ram", 0x1000, Endianness::Little));
        bus.register_device(memory, 0x1000).unwrap();

        let be_memory = Arc::new(RamMemory::new("be_ram", 0x1000, Endianness::Big));
        bus.register_device(be_memory, 0x2000).unwrap();

        let mut cursor = AddressHandle::new(bus);
        cursor.jump(0x1000).expect("valid address");
        {
            let mut scalar = cursor.scalar_handle(4).expect("pin is valid");
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
        let value = cursor.scalar_handle(4).expect("pin is valid").read().expect("read succeeds");
        assert_eq!(
            value,
            0xDEADBEEF,
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
        let value = cursor.scalar_handle(4).expect("pin is valid").read().expect("read succeeds");
        assert_eq!(
            value,
            0xDEADBEEF,
            "scalar helper should read the written value on big-endian device"
        );
    }

    #[test]
    fn redirect_allows_alias_reads() {
        let bus = Arc::new(DeviceBus::new(10));
        let memory = Arc::new(RamMemory::new("flash", 0x2000, Endianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();

        let mut addr = AddressHandle::new(bus.clone());
        addr.jump(0x150).unwrap();
        addr.scalar_handle(4).unwrap().write(0x12345678).unwrap();

        bus.redirect(0x4000, 4, 0x150).unwrap();
        addr.jump(0x4000).unwrap();
        let value = addr.scalar_handle(4).expect("pin is valid").read().expect("read succeeds");
        assert_eq!(
            value,
            0x12345678,
            "handle should read bytes through the redirect alias"
        );
    }
}
