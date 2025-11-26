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
    data: PinnedRange<'a>,
    cache: Option<u64>,
    start: usize,
    size: usize,
}

impl<'a> ScalarHandle<'a> {
    pub fn create(
        data: PinnedRange<'a>,
        start: usize,
        size: usize,
    ) -> Self {
        Self {
            data,
            cache: None,
            start,
            size,
        }
    }

    pub fn read(&mut self) -> BusResult<u64> {
        if let Some(cached) = self.cache {
            return Ok(cached);
        }
        let cache = if self.size <= 8 {
            let mut buf = [0u8; 8];
            let window = &mut buf[..self.size];
            let data = self.data.cache.as_ref().ok_or(BusError::DeviceFault {
                device: self.data.device.name().to_string(),
                source: Box::new(DeviceError::Unsupported("no cached data")),
            })?;
            window.copy_from_slice(&data[self.start..self.start + self.size]);
            let value = self.data.device.endianness().to_native(u64::from_ne_bytes(buf));
            value
        } else {
            return Err(BusError::Unsupported("scalar read size > 8 bytes"));
        };
        self.cache = Some(cache);
        Ok(cache)
    }
}

//A pinned range allows for reading/writing to a specific range on a device
//and leaving it reserved to promote some atomicity guarantees.  
pub struct PinnedRange<'a> {
    device: &'a dyn Device,
    start: usize,
    len: usize,
}

impl<'a> PinnedRange<'a> {
    pub fn create(
        device: &'a dyn Device,
        start: usize,
        len: usize,
    ) -> BusResult<Self> {
        device.reserve(start, len)?;
        Ok(Self { device, start, len})
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

impl Drop for PinnedRange<'_> {
    fn drop(&mut self) {
        // ignore errors on drop; handle logs if you need them
        let _ = self.device.commit(self.start);
    }
}

pub struct DataHandle {
    address: AddressHandle,
    pub cache: u64,
    pub last_size: usize,
}

impl DataHandle {
    pub fn new(bus: Arc<DeviceBus>) -> Self {
        Self {
            address: AddressHandle::new(bus),
            cache: 0,
            last_size: 0,
        }
    }

    pub fn address(&self) -> &AddressHandle {
        &self.address
    }

    pub fn address_mut(&mut self) -> &mut AddressHandle {
        &mut self.address
    }

    pub fn available(&self, size: usize) -> bool {
        self.address.available(size)
    }

    // Scalar endianness interface -------------------------------------------------
    pub fn fetch(&mut self, size: usize) -> BusResult<u64> {
        assert!((1..=8).contains(&size));
        let mut buf = [0u8; 8];

        self.address.transact(size, |device, offset, _| {
            let window = &mut buf[..size];
            device.read(offset, window).map_err(map_device_err)?;
            device.endianness().to_native_mut(window);
            Ok(())
        })?;

        self.last_size = size;
        self.cache = u64::from_ne_bytes(buf);
        Ok(self.cache)
    }

    pub fn commit(&mut self) -> BusResult<()> {

    }

    pub fn write_data(&mut self, value: u64, size: usize) -> BusResult<()> {
        assert!((1..=8).contains(&size));
        let mut buf = value.to_ne_bytes();

        self.address.transact(size, |device, offset, _| {
            let window = &mut buf[..size];
            device.endianness().from_native_mut(window);
            device.write(offset, window).map_err(map_device_err)
        })
    }
}

fn bytes_for_len(bit_len: u16) -> usize {
    ((bit_len as usize + 7) / 8).max(1)
}

fn map_device_err(err: DeviceError) -> DeviceError {
    err
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{BasicMemory, Endianness};
    use crate::soc::bus::DeviceBus;

    #[test]
    fn read_write_round_trip() {
        let bus = Arc::new(DeviceBus::new(12));
        let memory = Arc::new(BasicMemory::new("ram", 0x1000, Endianness::Little));
        bus.register_device(memory, 0x1000).unwrap();

        let be_memory = Arc::new(BasicMemory::new("be_ram", 0x1000, Endianness::Big));
        bus.register_device(be_memory, 0x2000).unwrap();

        let mut handle = DataHandle::new(bus.clone());
        handle.address_mut().jump(0x1000).unwrap();
        handle.write_data(0xDEADBEEF, 4).unwrap();
        handle.address_mut().jump(0x1000).unwrap();
        let value = handle.fetch(4).unwrap();
        assert_eq!(
            value,
            0xDEADBEEF,
            "scalar helper should round trip the written value"
        );

        handle.address_mut().jump(0x2000).unwrap();
        handle.write_data(0xDEADBEEF, 4).unwrap();
        handle.address_mut().jump(0x2000).unwrap();
        let value = handle.fetch(4).unwrap();
        assert_eq!(
            value,
            0xDEADBEEF,
            "scalar helper should round trip the written value on big-endian device"
        );
    }

    #[test]
    fn redirect_allows_alias_reads() {
        let bus = Arc::new(DeviceBus::new(10));
        let memory = Arc::new(BasicMemory::new("flash", 0x2000, Endianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();

        let mut preload = DataHandle::new(bus.clone());
        preload.address_mut().jump(0x150).unwrap();
        preload.write_data(0x12345678, 4).unwrap();        
        bus.redirect(0x4000, 4, 0x150).unwrap();

        let mut handle = DataHandle::new(bus);
        handle.address_mut().jump(0x4000).unwrap();
        let value = handle.fetch(4).unwrap();
        assert_eq!(
            value,
            0x12345678,
            "handle should read bytes through the redirect alias"
        );
    }
}
