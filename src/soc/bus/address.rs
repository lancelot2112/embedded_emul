//! AddressHandle wraps a resolved bus range and provides cursor-based navigation
//! so callers can keep a stable cursor across jumps, reads, and writes without
//! mutating the underlying `DeviceBus` mapping.
//!
//! The handle owns an `Arc<DeviceBus>` and validates bounds for every cursor
//! movement, mirroring the responsibilities of `BasicBusAccess` in the .NET
//! reference implementation while remaining borrowing-friendly for Rust.
//! It also provides a `transact` method that simplifies performing
//! read/write operations against the currently mapped device at the current cursor
//! position simulating atomicity.
use std::sync::Arc;
use smallvec::SmallVec;

use crate::soc::device::{Device, DeviceResult};

use super::{
    DeviceBus,
    error::{BusError, BusResult},
    range::ResolvedRange,
};

#[derive(Clone)]
pub struct AddressHandle {
    bus: Arc<DeviceBus>,
    active: Option<ActiveRange>,
    jump_address: Option<usize>,
    jump_device_offset: Option<usize>,
}

#[derive(Clone)]
struct ActiveRange {
    resolved: ResolvedRange,
    cursor: usize,
}

impl ActiveRange {
    fn bus_address(&self) -> usize {
        self.resolved.bus_start + self.cursor
    }

    fn device_offset(&self) -> usize {
        self.resolved.device_offset + self.cursor
    }

    fn bytes_remaining(&self) -> usize {
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

    pub fn jump(&mut self, address: usize) -> BusResult<()> {
        let resolved = self.bus.resolve(address)?;
        let cursor = address - resolved.bus_start;
        let device_offset = resolved.device_offset + cursor;
        self.jump_address = Some(address);
        self.jump_device_offset = Some(device_offset);
        self.active = Some(ActiveRange { resolved, cursor });
        Ok(())
    }

    pub fn jump_relative(&mut self, delta: isize) -> BusResult<()> {
        let base = self
            .jump_device_offset
            .ok_or(BusError::HandleNotPositioned)? as i128;
        let active = self.active.as_mut().ok_or(BusError::HandleNotPositioned)?;
        let range_start = active.resolved.device_offset as i128;
        let range_end = range_start + active.resolved.len() as i128;
        let target = base + delta as i128;
        if target < range_start || target >= range_end {
            return Err(BusError::OutOfRange {
                address: self.jump_address.unwrap_or(active.bus_address()),
                end: active.resolved.bus_end,
            });
        }
        active.cursor = (target - range_start) as usize;
        Ok(())
    }

    //-------- Advance / Retreat / Transact --------------------------------
    //Advance and retreat adjust the cursor within the currently resolved device
    pub fn advance(&mut self, bytes: usize) -> BusResult<()> {
        let active = self.active.as_mut().ok_or(BusError::HandleNotPositioned)?;
        if bytes > active.bytes_remaining() {
            return Err(BusError::OutOfRange {
                address: active.bus_address() + bytes,
                end: active.resolved.bus_end,
            });
        }
        active.cursor += bytes;
        Ok(())
    }

    pub fn retreat(&mut self, bytes: usize) -> BusResult<()> {
        let active = self.active.as_mut().ok_or(BusError::HandleNotPositioned)?;
        if bytes > active.cursor {
            return Err(BusError::OutOfRange {
                address: active.resolved.bus_start,
                end: active.resolved.bus_end,
            });
        }
        active.cursor -= bytes;
        Ok(())
    }

    pub fn bytes_to_end(&self) -> usize {
        self.active
            .as_ref()
            .map(|range| range.bytes_remaining())
            .unwrap_or(0)
    }

    pub fn bus_address(&self) -> Option<usize> {
        self.active.as_ref().map(|range| range.bus_address())
    }

    pub fn device_offset(&self) -> Option<usize> {
        self.active.as_ref().map(|range| range.device_offset())
    }

    pub fn available(&self, size: usize) -> bool {
        self.active
            .as_ref()
            .map(|range| range.bytes_remaining() >= size)
            .unwrap_or(false)
    }

    pub fn lock(&mut self, out: &mut [u8]) -> BusResult<())> {
        //Do we have a device at our current location
        let active = self.active.as_mut().ok_or(BusError::HandleNotPositioned)?;
        //Start a transaction on the device
        active.resolved.device.start_transact()?;

        //Check our requested size
        if size > active.bytes_remaining() {
            let err = Err(BusError::OutOfRange {
                address: active.bus_address() + size,
                end: active.resolved.bus_end,
            });
            active.resolved.device.end_transact()?;
            return err;
        }
        let mut buf = SmallVec::<[u8; 256]>::with_capacity(size);
        active.resolved.device.read(active.device_offset(), &mut buf).map_err(|err| {
            BusError::DeviceFault {
                device: active.resolved.device.name().to_string(),
                source: Box::new(err),
            }
        })?;
        Ok(&buf)
    }

    pub fn unlock(&mut self) -> BusResult<()> {
        if let Some(active) = self.active.as_mut() {
            active
                .resolved
                .device
                .end_transact()?;
            Ok(())
        } else {
            Err(BusError::HandleNotPositioned)
        }
    }
    pub(crate) fn transact<F, T>(&mut self, size: usize, op: F) -> BusResult<T>
    where
        F: FnOnce(&dyn Device, usize, &ResolvedRange) -> DeviceResult<T>,
    {
        let active = self.active.as_mut().ok_or(BusError::HandleNotPositioned)?;
        if size > active.bytes_remaining() {
            return Err(BusError::OutOfRange {
                address: active.bus_address() + size,
                end: active.resolved.bus_end,
            });
        }
        let device_offset = active.device_offset();
        let device_name = active.resolved.device.name().to_string();
        active.resolved.device.start_transact()?;
        let result =
            op(&*active.resolved.device, device_offset, &active.resolved).map_err(|err| {
                BusError::DeviceFault {
                    device: device_name,
                    source: Box::new(err),
                }
            })?;
        active.resolved.device.end_transact()?;
        active.cursor += size;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{BasicMemory, Endianness};
    use crate::soc::bus::DeviceBus;
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
        assert!(
            handle.jump(0x1000).is_ok(),
            "jump to base mapping should succeed"
        );
        assert_eq!(
            handle.bus_address(),
            Some(0x1000),
            "cursor should align with the jump address"
        );
        handle.advance(0x10).unwrap();
        assert_eq!(
            handle.bus_address(),
            Some(0x1010),
            "advance should move cursor forward by requested bytes"
        );
        handle.retreat(0x8).unwrap();
        assert_eq!(
            handle.bus_address(),
            Some(0x1008),
            "retreat pulls cursor back within the range"
        );
        assert!(
            handle.retreat(0x9).is_err(),
            "retreat past mapping start should error"
        );
    }

    #[test]
    fn available_and_bytes_to_end_update() {
        let bus = make_bus();
        let mut handle = AddressHandle::new(bus);
        handle.jump(0x1FFF).unwrap();
        // Confirm we can read up to the range end and that bytes_to_end reflects consumed distance.
        let initial = handle.bytes_to_end();
        assert!(
            handle.available(0x10),
            "range reports availability before consuming bytes"
        );
        handle.advance(0x10).unwrap();
        assert_eq!(
            handle.bytes_to_end(),
            initial - 0x10,
            "bytes_to_end shrinks by the consumed amount"
        );
        assert!(
            !handle.available(initial + 1),
            "request larger than remaining bytes should fail"
        );
    }

    #[test]
    fn jump_relative_bounds_check() {
        let bus = make_bus();
        let mut handle = AddressHandle::new(bus);
        handle.jump(0x1000).unwrap();
        handle.advance(0x20).unwrap();
        // Positive deltas move the cursor forward, but enormous negatives are rejected.
        assert!(
            handle.jump_relative(0x10).is_ok(),
            "relative forward jump within mapping should succeed"
        );
        assert_eq!(
            handle.bus_address(),
            Some(0x1010),
            "cursor reflects the new relative address"
        );
        assert!(
            handle.jump_relative(-0x100).is_err(),
            "large negative jump should exceed bounds"
        );
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
            .transact(4, |device, offset, _resolved| {
                let write_bytes = 0xAABB_CCDD_u32.to_le_bytes();
                device.write(offset, &write_bytes)?;
                let mut out = [0u8; 4];
                device.read(offset, &mut out)?;
                Ok(u32::from_le_bytes(out) as u64)
            })
            .unwrap();
        assert_eq!(
            value, 0xAABB_CCDD_u64,
            "closure should observe the written 32-bit value"
        );
        assert_eq!(
            handle.bus_address(),
            Some(0x2004),
            "cursor should advance by the transact size"
        );

        // Underlying memory sees the write at the expected device offset.
        let mut check = [0u8; 4];
        memory.read(0, &mut check).expect("read back value");
        assert_eq!(
            u32::from_le_bytes(check),
            0xAABB_CCDD,
            "device offset zero stores the same pattern"
        );
    }
}
