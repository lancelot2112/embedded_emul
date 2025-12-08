//! Software MMU implementation layered on top of the physical `DeviceBus`.
//! Redirect (virtual) address mappings live here so individual cores can
//! curate their own view of the bus without mutating the underlying map.
use bitflags::bitflags;
use std::{collections::BTreeMap, sync::Arc};

use crate::soc::bus::{BusError, BusResult, DeviceBus, DeviceRef, range::BusRange};
use crate::soc::device::Endianness;

type VirtAddr = usize;
type PhysAddr = usize;

bitflags! {
    #[derive(Default, PartialEq, Copy, Clone)]
    pub struct MMUFlags: u32 {
        const VALID     = 0b1;
        const READ      = 0b10;
        const WRITE     = 0b100;
        const EXEC      = 0b1000;
        const RAM       = 0b1_0000;
        const BIGENDIAN = 0b10_0000; // 0 = Little, 1 = Big
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressMode {
    Physical,
    Effective,
}

#[derive(Clone)]
pub struct MMUEntry {
    pub vaddr: VirtAddr,
    pub paddr: PhysAddr,
    pub size: usize,
    pub flags: MMUFlags,
    device_offset: usize,
    device: DeviceRef,
}

pub struct SoftMMU {
    regions: BTreeMap<VirtAddr, MMUEntry>,
    bus: Arc<DeviceBus>,
    mode: AddressMode,
}

impl SoftMMU {
    pub fn new(bus: Arc<DeviceBus>) -> Self {
        Self::with_mode(bus, AddressMode::Effective)
    }

    pub fn with_mode(bus: Arc<DeviceBus>, mode: AddressMode) -> Self {
        Self {
            regions: BTreeMap::new(),
            bus,
            mode,
        }
    }

    /// Maps a virtual region to a physical device span.
    /// The entire range must live within a single physical mapping so the
    /// generated TLB entry can service the request without crossing devices.
    pub fn map_region(
        &mut self,
        vaddr: VirtAddr,
        paddr: PhysAddr,
        size: usize,
        flags: MMUFlags,
    ) -> BusResult<()> {
        if size == 0 {
            return Err(BusError::RedirectInvalid {
                source: vaddr,
                size,
                target: paddr,
                reason: "zero-length region",
            });
        }

        let vend = vaddr.checked_add(size).ok_or(BusError::RedirectInvalid {
            source: vaddr,
            size,
            target: paddr,
            reason: "virtual range overflow",
        })?;

        if self.overlaps(vaddr, vend) {
            return Err(BusError::Overlap {
                address: vaddr,
                details: "virtual range overlaps existing mapping".into(),
            });
        }

        let (device, phys_range) = self.bus.resolve_device_at(paddr)?;
        validate_physical_span(paddr, size, &phys_range)?;
        let device_offset = phys_range.device_offset + (paddr - phys_range.bus_start);

        let flags = Self::flags_for_device_flags(&device, flags | MMUFlags::VALID);

        self.regions.insert(
            vaddr,
            MMUEntry {
                vaddr,
                paddr,
                size,
                flags,
                device_offset,
                device,
            },
        );
        Ok(())
    }

    pub fn unmap_region(&mut self, vaddr: VirtAddr) -> BusResult<()> {
        self.regions
            .remove(&vaddr)
            .map(|_| ())
            .ok_or(BusError::NotMapped { address: vaddr })
    }

    // Translate a virtual address into an addend, flags, and a device ref
    pub fn translate(&self, vaddr: VirtAddr) -> BusResult<(usize, MMUFlags, DeviceRef)> {
        match self.mode {
            AddressMode::Physical => self.translate_physical(vaddr),
            AddressMode::Effective => self.translate_effective(vaddr),
        }
    }

    pub fn set_mode(&mut self, mode: AddressMode) {
        self.mode = mode;
    }

    pub fn mode(&self) -> AddressMode {
        self.mode
    }

    fn translate_physical(&self, addr: VirtAddr) -> BusResult<(usize, MMUFlags, DeviceRef)> {
        let (device, phys_range) = self.bus.resolve_device_at(addr)?;
        let device_offset = phys_range.device_offset + (addr - phys_range.bus_start);
        let mut flags = Self::flags_for_device_flags(
            &device,
            MMUFlags::VALID | MMUFlags::READ | MMUFlags::WRITE | MMUFlags::EXEC,
        );
        if let Some(ram) = device.as_ram() {
            let host_ptr = ram.ptr_at(device_offset) as usize;
            let addend = host_ptr.wrapping_sub(addr);
            return Ok((addend, flags, device));
        }
        let addend = device_offset.wrapping_sub(addr);
        Ok((addend, flags, device))
    }

    fn translate_effective(&self, vaddr: VirtAddr) -> BusResult<(usize, MMUFlags, DeviceRef)> {
        let entry = self
            .regions
            .range(..=vaddr)
            .next_back()
            .map(|(_, entry)| entry)
            .ok_or_else(|| BusError::PageFault {
                details: format!("No mapping for virtual address {:#X}", vaddr),
            })?;

        if vaddr >= entry.vaddr + entry.size {
            return Err(BusError::PageFault {
                details: format!(
                    "Address {:#X} outside of mapped region [{:#X} - {:#X}]",
                    vaddr,
                    entry.vaddr,
                    entry.vaddr + entry.size
                ),
            });
        }

        let offset = vaddr - entry.vaddr;
        let device_offset = entry.device_offset + offset;
        let device = entry.device.clone();
        let mut flags = entry.flags | MMUFlags::VALID;
        flags = Self::flags_for_device_flags(&device, flags);

        if let Some(ram) = device.as_ram() {
            let host_ptr = ram.ptr_at(device_offset) as usize;
            let addend = host_ptr.wrapping_sub(vaddr);
            return Ok((addend, flags, device));
        }
        let addend = device_offset.wrapping_sub(vaddr);
        Ok((addend, flags, device))
    }

    fn overlaps(&self, start: VirtAddr, end: VirtAddr) -> bool {
        if let Some((_, region)) = self.regions.range(..=start).next_back() {
            if region.vaddr + region.size > start {
                return true;
            }
        }
        self.regions.range(start..end).next().is_some()
    }

    fn flags_for_device_flags(device: &DeviceRef, mut flags: MMUFlags) -> MMUFlags {
        match device.endianness() {
            Endianness::Big => flags |= MMUFlags::BIGENDIAN,
            _ => flags.remove(MMUFlags::BIGENDIAN),
        }
        if device.as_ram().is_some() {
            flags |= MMUFlags::RAM;
        } else {
            flags.remove(MMUFlags::RAM);
        }
        flags
    }
}

fn validate_physical_span(start: PhysAddr, size: usize, range: &BusRange) -> BusResult<()> {
    let phys_end = start.checked_add(size).ok_or(BusError::RedirectInvalid {
        source: start,
        size,
        target: range.bus_start,
        reason: "physical range overflow",
    })?;

    if phys_end > range.bus_end {
        return Err(BusError::RedirectInvalid {
            source: start,
            size,
            target: range.bus_start,
            reason: "mapping spans multiple physical devices",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{Endianness, RamMemory};

    #[test]
    fn virtual_mapping_resolves_into_ram_entry() {
        let mut bus = DeviceBus::new(32);
        let ram = RamMemory::new("ram", 0x2000, Endianness::Little);
        let expected_ptr = ram.ptr_at(0x1880 - 0x1000) as usize;
        bus.map_device(ram, 0x1000, 0).unwrap();
        let bus = Arc::new(bus);
        let mut mmu = SoftMMU::new(bus);
        mmu.map_region(0x8000, 0x1800, 0x100, MMUFlags::READ)
            .expect("map virtual region");

        let (addend, flags, _device) = mmu.translate(0x8080).expect("translate within range");
        assert!(
            flags.contains(MMUFlags::RAM),
            "ram-backed mappings should set the RAM flag"
        );
        assert_eq!(
            0x8080usize.wrapping_add(addend),
            expected_ptr,
            "translated address should map to backing RAM"
        )
    }
}
