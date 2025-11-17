//! AddressHandle wraps a resolved bus range and provides cursor-based navigation
//! so callers can keep a stable cursor across jumps, reads, and writes without
//! mutating the underlying `DeviceBus` mapping.
//!
//! The handle owns an `Arc<DeviceBus>` and validates bounds for every cursor
//! movement, mirroring the responsibilities of `BasicBusAccess` in the .NET
//! reference implementation while remaining borrowing-friendly for Rust.
use std::sync::Arc;

use super::{
    bus::DeviceBus,
    device::Device,
    error::{BusError, BusResult},
    range::ResolvedRange,
};

#[derive(Clone)]
pub struct AddressHandle {
    bus: Arc<DeviceBus>,
    active: Option<ActiveRange>,
    jump_address: Option<u64>,
    jump_device_offset: Option<u64>,
}

#[derive(Clone)]
struct ActiveRange {
    resolved: ResolvedRange,
    cursor: u64,
}

impl ActiveRange {
    fn bus_address(&self) -> u64 {
        self.resolved.bus_start + self.cursor
    }

    fn device_offset(&self) -> u64 {
        self.resolved.device_offset + self.cursor
    }

    fn bytes_remaining(&self) -> u64 {
        self.resolved.bus_end - self.bus_address()
    }
}

impl AddressHandle {
    pub fn new(bus: Arc<DeviceBus>) -> Self {
        Self {
            bus,
            active: None,
            jump_address: None,
            jump_device_offset: None,
        }
    }

    pub fn bus(&self) -> &Arc<DeviceBus> {
        &self.bus
    }

    pub fn jump(&mut self, address: u64) -> BusResult<()> {
        let resolved = self.bus.resolve(address)?;
        let cursor = address - resolved.bus_start;
        let device_offset = resolved.device_offset + cursor;
        self.jump_address = Some(address);
        self.jump_device_offset = Some(device_offset);
        self.active = Some(ActiveRange { resolved, cursor });
        Ok(())
    }

    pub fn jump_relative(&mut self, delta: i64) -> BusResult<()> {
        let base = self
            .jump_device_offset
            .ok_or(BusError::HandleNotPositioned)? as i128;
        let active = self
            .active
            .as_mut()
            .ok_or(BusError::HandleNotPositioned)?;
        let range_start = active.resolved.device_offset as i128;
        let range_end = range_start + active.resolved.len() as i128;
        let target = base + delta as i128;
        if target < range_start || target >= range_end {
            return Err(BusError::OutOfRange {
                address: self.jump_address.unwrap_or(active.bus_address()),
                end: active.resolved.bus_end,
            });
        }
        active.cursor = (target - range_start) as u64;
        Ok(())
    }

    pub fn advance(&mut self, bytes: u64) -> BusResult<()> {
        let active = self
            .active
            .as_mut()
            .ok_or(BusError::HandleNotPositioned)?;
        if bytes > active.bytes_remaining() {
            return Err(BusError::OutOfRange {
                address: active.bus_address() + bytes,
                end: active.resolved.bus_end,
            });
        }
        active.cursor += bytes;
        Ok(())
    }

    pub fn retreat(&mut self, bytes: u64) -> BusResult<()> {
        let active = self
            .active
            .as_mut()
            .ok_or(BusError::HandleNotPositioned)?;
        if bytes > active.cursor {
            return Err(BusError::OutOfRange {
                address: active.resolved.bus_start,
                end: active.resolved.bus_end,
            });
        }
        active.cursor -= bytes;
        Ok(())
    }

    pub fn bytes_to_end(&self) -> u64 {
        self.active
            .as_ref()
            .map(|range| range.bytes_remaining())
            .unwrap_or(0)
    }

    pub fn bus_address(&self) -> Option<u64> {
        self.active.as_ref().map(|range| range.bus_address())
    }

    pub fn device_offset(&self) -> Option<u64> {
        self.active.as_ref().map(|range| range.device_offset())
    }

    pub fn available(&self, size: u64) -> bool {
        self.active
            .as_ref()
            .map(|range| range.bytes_remaining() >= size)
            .unwrap_or(false)
    }

    pub(crate) fn transact<F, T>(&mut self, size: u64, op: F) -> BusResult<T>
    where
        F: FnOnce(&dyn Device, u64) -> BusResult<T>,
    {
        let active = self
            .active
            .as_mut()
            .ok_or(BusError::HandleNotPositioned)?;
        if size > active.bytes_remaining() {
            return Err(BusError::OutOfRange {
                address: active.bus_address() + size,
                end: active.resolved.bus_end,
            });
        }
        let device_offset = active.device_offset();
        let result = op(&*active.resolved.device, device_offset)?;
        active.cursor += size;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::system::bus::{BasicMemory, Device, DeviceBus, Endianness};
    use std::sync::Arc;

    fn make_bus() -> Arc<DeviceBus> {
        let bus = Arc::new(DeviceBus::new(12));
        let memory = Arc::new(BasicMemory::new("ram", 0x2000, Endianness::Little));
        bus.register_device(memory, 0x1000).unwrap();
        bus
    }

    #[test]
    fn jump_retreat_and_advance_track_cursor() {
        let bus = make_bus();
        let mut handle = AddressHandle::new(bus);
        assert!(handle.jump(0x1000).is_ok());
        assert_eq!(handle.bus_address(), Some(0x1000));
        handle.advance(0x10).unwrap();
        assert_eq!(handle.bus_address(), Some(0x1010));
        handle.retreat(0x8).unwrap();
        assert_eq!(handle.bus_address(), Some(0x1008));
        assert!(handle.retreat(0x9).is_err());
    }

    #[test]
    fn available_and_bytes_to_end_update() {
        let bus = make_bus();
        let mut handle = AddressHandle::new(bus);
        handle.jump(0x1FFF).unwrap();
        // Confirm we can read up to the range end and that bytes_to_end reflects consumed distance.
        let initial = handle.bytes_to_end();
        assert!(handle.available(0x10));
        handle.advance(0x10).unwrap();
        assert_eq!(handle.bytes_to_end(), initial - 0x10);
        assert!(!handle.available(initial + 1));
    }

    #[test]
    fn jump_relative_bounds_check() {
        let bus = make_bus();
        let mut handle = AddressHandle::new(bus);
        handle.jump(0x1000).unwrap();
        handle.advance(0x20).unwrap();
        // Positive deltas move the cursor forward, but enormous negatives are rejected.
        assert!(handle.jump_relative(0x10).is_ok());
        assert_eq!(handle.bus_address(), Some(0x1010));
        assert!(handle.jump_relative(-0x100).is_err());
    }

    #[test]
    fn transact_performs_operation_and_advances() {
        let bus = Arc::new(DeviceBus::new(12));
        let memory = Arc::new(BasicMemory::new("ram", 0x2000, Endianness::Little));
        bus.register_device(memory.clone(), 0x2000).unwrap();
        let mut handle = AddressHandle::new(bus);
        handle.jump(0x2000).unwrap();

        // transact should execute the closure against the resolved device and advance the cursor.
        let value = handle
            .transact(4, |device, offset| {
                device.write_u32(offset, 0xAABB_CCDD)?;
                device.read_u32(offset)
            })
            .unwrap();
        assert_eq!(value, 0xAABB_CCDD);
        assert_eq!(handle.bus_address(), Some(0x2004));

        // Underlying memory sees the write at the expected device offset.
        assert_eq!(memory.read_u32(0).unwrap(), 0xAABB_CCDD);
    }
}
