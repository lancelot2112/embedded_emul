//! BusCursor wraps a resolved bus range and provides cursor-based navigation
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

use crate::soc::{
    bus::{
        DeviceBus,
        softmmu::{AddressMode, MMUFlags, SoftMMU},
        softtlb::SoftTLB,
    },
    device::AccessContext,
};

use super::error::{BusError, BusResult};

pub struct BusCursor {
    tlb: SoftTLB,
    ref_zero: usize,
    address: usize,
}

impl BusCursor {
    pub fn new(tlb: SoftTLB, start: usize) -> Self {
        Self {
            tlb,
            ref_zero: start,
            address: start,
        }
    }

    pub fn attach_to_bus(bus: Arc<DeviceBus>, start: usize, context: AccessContext) -> Self {
        Self::attach_to_bus_with_mode(bus, start, context, AddressMode::Physical)
    }

    pub fn attach_to_bus_with_mode(
        bus: Arc<DeviceBus>,
        start: usize,
        context: AccessContext,
        mode: AddressMode,
    ) -> Self {
        let mmu = SoftMMU::with_mode(bus, mode);
        let tlb = SoftTLB::new(mmu, context);
        Self::new(tlb, start)
    }

    pub fn set_address_mode(&mut self, mode: AddressMode) {
        self.tlb.set_address_mode(mode);
    }

    pub fn address_mode(&self) -> AddressMode {
        self.tlb.address_mode()
    }

    #[inline(always)]
    fn validate_request(&mut self, requested: usize) -> BusResult<()> {
        let _entry = self.tlb.lookup(requested)?;
        Ok(())
    }

    // General purpose jump to an absolute offset within the mapped range.
    #[inline(always)]
    pub fn goto(&mut self, new_offset: usize) -> BusResult<&mut Self> {
        if new_offset == self.address {
            return Ok(self);
        }

        match self.validate_request(new_offset) {
            Ok(_) => {
                self.address = new_offset;
                Ok(self)
            }
            Err(e) => Err(e),
        }
    }

    // Pin a cursor position within the mapped range.  Pin is initially set to the offset at new.
    #[inline(always)]
    pub fn set_ref(&mut self, new_offset: usize) -> BusResult<&mut Self> {
        self.goto(new_offset)?;
        self.ref_zero = self.address;
        Ok(self)
    }

    // forward or backwarde cursor relative to the pinned position.
    #[inline(always)]
    pub fn forward_from_ref(&mut self, delta: usize) -> BusResult<&mut Self> {
        self.goto(self.ref_zero.saturating_add(delta))
    }

    #[inline(always)]
    pub fn backward_from_ref(&mut self, delta: usize) -> BusResult<&mut Self> {
        let target = self
            .ref_zero
            .checked_sub(delta)
            .ok_or(BusError::InvalidAddress { address: 0 })?;
        self.goto(target)
    }

    #[inline(always)]
    pub fn goto_ref(&mut self) -> BusResult<&mut Self> {
        // Reset cursor to pinned position... it's already been validated
        // so no need to repeat work
        self.address = self.ref_zero;
        Ok(self)
    }

    // forward or backwarde cursor relative to the current cursor.
    #[inline(always)]
    pub fn forward(&mut self, delta: usize) -> BusResult<&mut Self> {
        self.goto(self.address.saturating_add(delta))
    }

    #[inline(always)]
    pub fn backward(&mut self, delta: usize) -> BusResult<&mut Self> {
        let target = self
            .address
            .checked_sub(delta)
            .ok_or(BusError::InvalidAddress { address: 0 })?;
        self.goto(target)
    }

    #[inline(always)]
    pub fn dist_from_ref(&self) -> isize {
        self.address as isize - self.ref_zero as isize
    }

    #[inline(always)]
    pub fn get_ref(&self) -> usize {
        self.ref_zero
    }

    #[inline(always)]
    pub fn get_position(&self) -> usize {
        self.address
    }

    // Direct to TLB interfaces for reads/writes at specific addresses
    #[inline(always)]
    pub fn peek_at<T>(&mut self, vaddr: usize) -> BusResult<T>
    where
        T: crate::soc::bus::EndianWord,
    {
        self.tlb.peek::<T>(vaddr)
    }

    #[inline(always)]
    pub fn read_at<T>(&mut self, vaddr: usize) -> BusResult<T>
    where
        T: crate::soc::bus::EndianWord,
    {
        let out = self.tlb.read::<T>(vaddr)?;
        self.address += std::mem::size_of::<T>();
        Ok(out)
    }

    #[inline(always)]
    pub fn write_at<T>(&mut self, vaddr: usize, value: T) -> BusResult<()>
    where
        T: crate::soc::bus::EndianWord,
    {
        self.tlb.write::<T>(vaddr, value)?;
        self.address += std::mem::size_of::<T>();
        Ok(())
    }

    #[inline(always)]
    pub fn read_ram_at(&mut self, vaddr: usize, size: usize) -> BusResult<&[u8]> {
        let out = self.tlb.read_ram(vaddr, size)?;
        self.address += size;
        Ok(out)
    }

    #[inline(always)]
    pub fn write_ram_at(&mut self, vaddr: usize, data: &[u8]) -> BusResult<()> {
        self.tlb.write_ram(vaddr, data)?;
        self.address += data.len();
        Ok(())
    }

    #[inline(always)]
    pub fn read<T>(&mut self) -> BusResult<T>
    where
        T: crate::soc::bus::EndianWord,
    {
        self.read_at::<T>(self.address)
    }

    #[inline(always)]
    pub fn write<T>(&mut self, value: T) -> BusResult<()>
    where
        T: crate::soc::bus::EndianWord,
    {
        self.write_at::<T>(self.address, value)
    }

    #[inline(always)]
    pub fn peek<T>(&mut self) -> BusResult<T>
    where
        T: crate::soc::bus::EndianWord,
    {
        self.peek_at::<T>(self.address)
    }

    #[inline(always)]
    pub fn read_ram(&mut self, size: usize) -> BusResult<&[u8]> {
        self.read_ram_at(self.address, size)
    }

    #[inline(always)]
    pub fn write_ram(&mut self, data: &[u8]) -> BusResult<()> {
        self.write_ram_at(self.address, data)
    }

    #[inline(always)]
    pub fn peek_ram(&mut self, size: usize) -> BusResult<&[u8]> {
        self.tlb.read_ram(self.address, size)
    }

    // Convenience typed accessors at the current cursor position
    #[inline(always)]
    pub fn peek_u8(&mut self) -> BusResult<u8> {
        self.peek_at::<u8>(self.address)
    }

    #[inline(always)]
    pub fn peek_u16(&mut self) -> BusResult<u16> {
        self.peek_at::<u16>(self.address)
    }

    #[inline(always)]
    pub fn peek_u32(&mut self) -> BusResult<u32> {
        self.peek_at::<u32>(self.address)
    }

    #[inline(always)]
    pub fn peek_u64(&mut self) -> BusResult<u64> {
        self.peek_at::<u64>(self.address)
    }

    #[inline(always)]
    pub fn read_u8(&mut self) -> BusResult<u8> {
        self.read_at::<u8>(self.address)
    }

    #[inline(always)]
    pub fn read_u16(&mut self) -> BusResult<u16> {
        self.read_at::<u16>(self.address)
    }

    #[inline(always)]
    pub fn read_u32(&mut self) -> BusResult<u32> {
        self.read_at::<u32>(self.address)
    }

    #[inline(always)]
    pub fn read_u64(&mut self) -> BusResult<u64> {
        self.read_at::<u64>(self.address)
    }

    #[inline(always)]
    pub fn write_u8(&mut self, value: u8) -> BusResult<()> {
        self.write_at::<u8>(self.address, value)
    }

    #[inline(always)]
    pub fn write_u16(&mut self, value: u16) -> BusResult<()> {
        self.write_at::<u16>(self.address, value)
    }

    #[inline(always)]
    pub fn write_u32(&mut self, value: u32) -> BusResult<()> {
        self.write_at::<u32>(self.address, value)
    }

    #[inline(always)]
    pub fn write_u64(&mut self, value: u64) -> BusResult<()> {
        self.write_at::<u64>(self.address, value)
    }

    pub(crate) fn flags_at(&mut self, address: usize) -> BusResult<MMUFlags> {
        Ok(self.tlb.lookup(address)?.flags)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::soc::bus::{BusCursor, DeviceBus};
    use crate::soc::device::{AccessContext, Endianness, RamMemory};

    fn make_bus_at(map_start: usize) -> DeviceBus {
        let mut bus = DeviceBus::new(32);
        let memory = RamMemory::new("ram", 0x2000, Endianness::Little);
        bus.map_device(memory, map_start, 0)
            .expect("map device");
        bus
    }

    fn make_cursor_from(map_start: usize, start: usize) -> BusCursor {
        let bus = Arc::new(make_bus_at(map_start));
        BusCursor::attach_to_bus(bus, start, AccessContext::CPU)
    }

    fn make_cursor() -> BusCursor {
        make_cursor_from(0x1000, 0x1000)
    }

    #[test]
    fn move_relative_cursor() {
        let mut cursor = make_cursor();
        assert_eq!(
            cursor.get_position(),
            0x1000,
            "cursor should align with the jump address"
        );
        assert_eq!(
            cursor
                .forward(0x10)
                .expect("forward within range")
                .get_position(),
            0x1010,
            "forward should move cursor forward by requested bytes"
        );

        assert_eq!(
            cursor
                .backward(0x8)
                .expect("backward within range")
                .get_position(),
            0x1008,
            "backward pulls cursor back within the range"
        );
        assert!(
            cursor.backward(0x9).is_err(),
            "backward past mapping start should error"
        );

        assert_eq!(
            cursor.get_position(),
            0x1008,
            "error'd cursor was not moved"
        );

        assert!(
            cursor.forward(0x1FF8).is_err(),
            "forward past mapping end should error"
        );

        assert_eq!(
            cursor.get_position(),
            0x1008,
            "error'd cursor was not moved"
        );

        assert!(
            cursor.goto(0x3000).is_err(),
            "absolute jump past mapping end should error"
        );

        assert_eq!(
            cursor.get_position(),
            0x1008,
            "error'd cursor was not moved"
        );

        assert_eq!(
            cursor
                .goto(0x2000)
                .expect("absolute jump to mapping end")
                .get_position(),
            0x2000,
            "absolute jump to mapping end should succeed"
        );

        assert_eq!(
            cursor
                .goto(0x1000)
                .expect("absolute jump to mapping start")
                .get_position(),
            0x1000,
            "absolute jump to mapping start should succeed"
        );
    }

    #[test]
    fn move_relative_ref() {
        let mut cursor = make_cursor_from(0, 0);
        cursor.set_ref(0x20).unwrap();
        // Positive deltas move the cursor forward, but enormous negatives are rejected.
        assert!(
            cursor.forward(0x10).is_ok(),
            "relative forward jump within mapping should succeed"
        );
        assert_eq!(
            cursor.get_position(),
            0x30,
            "cursor reflects the new relative address"
        );
        assert!(
            cursor.forward_from_ref(0x5).is_ok(),
            "relative forward from pin should succeed"
        );
        assert_eq!(
            cursor.get_position(),
            0x25,
            "cursor reflects the new relative address"
        );
        assert!(
            cursor.backward_from_ref(0x100).is_err(),
            "large negative jump should exceed bounds"
        );
        assert!(
            cursor.backward_from_ref(0x10).is_ok(),
            "relative backward from pin within bounds should succeed"
        );
        assert!(
            cursor.get_position() == 0x10,
            "cursor reflects the new relative address"
        );
        assert!(
            cursor.backward_from_ref(0x20).is_ok(),
            "relative backward to zero should succeed"
        );
        assert!(
            cursor.get_position() == 0x0,
            "cursor reflects the new relative address"
        );
    }
}
