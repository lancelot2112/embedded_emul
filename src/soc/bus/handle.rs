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
use crate::soc::{
    bus::DeviceRef,
    device::{AccessContext, Endianness},
};

use super::error::{BusError, BusResult};

pub struct DeviceHandle{
    device: DeviceRef,
    endian: Endianness,
    pin: usize,
    offset: usize,
    size: usize,
}

impl DeviceHandle {
    pub fn new(device: DeviceRef, offset: usize) -> Self {
        let dev = device.lock().unwrap();
        let size = dev.span().len();
        let endian = dev.endianness();
        drop(dev);
        Self {
            device,
            endian,
            pin: offset,
            offset,
            size,
        }
    }
    // Functional access (advances cursor if needed, though usually fixed width)
    pub fn read(&mut self, out: &mut [u8], ctx: AccessContext) -> BusResult<()> {
        //Reads could mutate the underlying
        let mut dev = self.device.lock()?;
        dev.read(self.offset, out, ctx)?;
        self.offset = self.offset.saturating_add(out.len());
        Ok(())
    }

    pub fn write(&mut self, data: &[u8], ctx: AccessContext) -> BusResult<()> {
        //DeviceRef is Arc<RwLock<dyn Device>>
        self.device.lock()?.write(self.offset, data, ctx)?;
        self.offset = self.offset.saturating_add(data.len());
        Ok(())
    }

    // Pin a cursor position within the mapped range.  Pin is initially set to the offset at new.
    pub fn pin(&mut self, new_offset: usize) -> BusResult<()> {
        if new_offset >= self.size {
            self.pin = self.size;
            self.offset = self.size;
            return Err(BusError::HandleOutOfRange {
                offset: new_offset,
                delta: 0,
            });
        }
        self.pin = new_offset;
        self.offset = new_offset;
        Ok(())
    }

    // Advance or retreate cursor relative to the pinned position.
    pub fn adv_off_pin(&mut self, delta: usize) -> BusResult<()> {
        if self.pin + delta > self.size {
            self.offset = self.size;
            return Err(BusError::HandleOutOfRange {
                offset: self.pin,
                delta: delta as isize,
            });
        }
        self.offset = self.pin.saturating_add(delta);
        Ok(())
    }

    pub fn ret_off_pin(&mut self, delta: usize) -> BusResult<()> {
        if delta > self.pin {
            self.offset = 0;
            return Err(BusError::HandleOutOfRange {
                offset: self.pin,
                delta: -(delta as isize),
            });
        }
        self.offset = self.pin.saturating_sub(delta);
        Ok(())
    }

    pub fn reset(&mut self) {
        self.offset = self.pin;
    }

    // Advance or retreate cursor relative to the current cursor.
    pub fn seek(&mut self, new_offset: usize) -> BusResult<()> {
        if new_offset >= self.size {
            self.offset = self.size;
            return Err(BusError::HandleOutOfRange {
                offset: new_offset,
                delta: 0,
            });
        }
        self.offset = new_offset;
        Ok(())
    }
    pub fn advance(&mut self, delta: usize) -> BusResult<()> {
        self.offset = self.offset.saturating_add(delta);
        if self.offset > self.size {
            let prev_off = self.offset;
            self.offset = self.size;
            return Err(BusError::HandleOutOfRange {
                offset: prev_off,
                delta: delta as isize,
            });
        }
        self.offset += delta;
        Ok(())
    }

    pub fn retreat(&mut self, delta: usize) -> BusResult<()> {
        if delta > self.offset {
            let prev_off = self.offset;
            self.offset = 0;
            return Err(BusError::HandleOutOfRange {
                offset: prev_off,
                delta: -(delta as isize),
            });
        }
        self.offset -= delta;
        Ok(())
    }

    #[inline(always)]
    pub fn get_pin(&self) -> usize {
        self.pin
    }

    #[inline(always)]
    pub fn get_position(&self) -> usize {
        self.offset
    }

    #[inline(always)]
    pub fn get_end(&self) -> usize {
        self.size
    }

    #[inline(always)]
    pub fn get_remaining(&self) -> usize {
        self.size.saturating_sub(self.offset)
    }

    #[inline(always)]
    pub fn get_endianness(&self) -> Endianness {
        self.endian
    }

    #[inline(always)]
    pub fn get_device_name(&self) -> String {
        self.device.lock().unwrap().name().to_string()
    }
}

#[cfg(test)]
mod tests {
    use crate::soc::bus::DeviceBus;
    use crate::soc::device::{Endianness, RamMemory};

    fn make_bus() -> DeviceBus {
        let mut bus = DeviceBus::new();
        let memory = RamMemory::new("ram", 0x2000, Endianness::Little);
        bus.map_device(memory, 0x1000, 0).expect("map device");
        bus
    }

    #[test]
    fn move_relative_cursor() {
        let bus = make_bus();
        let mut handle = bus.resolve(0x1000).expect("ea should resolve");
        assert_eq!(
            handle.get_position(),
            0x0,
            "cursor should align with the jump address"
        );
        handle.advance(0x10).unwrap();
        assert_eq!(
            handle.get_position(),
            0x10,
            "advance should move cursor forward by requested bytes"
        );
        handle.retreat(0x8).unwrap();
        assert_eq!(
            handle.get_position(),
            0x8,
            "retreat pulls cursor back within the range"
        );
        assert!(
            handle.retreat(0x9).is_err(),
            "retreat past mapping start should error"
        );
    }

    #[test]
    fn remaining_updates() {
        let bus = make_bus();
        let mut handle = bus.resolve(0x1FFF).expect("ea should resolve");
        // Confirm we can read up to the range end and that bytes_to_end reflects consumed distance.
        let initial = handle.get_remaining();
        handle.advance(0x10).unwrap();
        assert_eq!(
            handle.get_remaining(),
            initial - 0x10,
            "remaining shrinks by the consumed amount"
        );
    }

    #[test]
    fn move_relative_pin() {
        let bus = make_bus();
        let mut handle = bus.resolve(0x1000).expect("ea should resolve");
        handle.pin(0x20).unwrap();
        // Positive deltas move the cursor forward, but enormous negatives are rejected.
        assert!(
            handle.advance(0x10).is_ok(),
            "relative forward jump within mapping should succeed"
        );
        assert_eq!(
            handle.get_position(),
            0x30,
            "cursor reflects the new relative address"
        );
        assert!(
            handle.adv_off_pin(0x5).is_ok(),
            "relative advance from pin should succeed"
        );
        assert_eq!(
            handle.get_position(),
            0x25,
            "cursor reflects the new relative address"
        );
        assert!(
            handle.ret_off_pin(0x100).is_err(),
            "large negative jump should exceed bounds"
        );
        assert!(
            handle.ret_off_pin(0x10).is_ok(),
            "relative retreat from pin within bounds should succeed"
        );
        assert!(
            handle.get_position() == 0x10,
            "cursor reflects the new relative address"
        );
        assert!(
            handle.ret_off_pin(0x20).is_ok(),
            "relative retreat to zero should succeed"
        );
        assert!(
            handle.get_position() == 0x0,
            "cursor reflects the new relative address"
        );
    }
}
