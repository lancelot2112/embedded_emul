//! Software MMU implementation
//! This is a redirection layer of the underlying DeviceBus to provide virtual memory support.
//! It uses a SoftTLB for caching translations.  Read/Write accesses should go through the SoftTLB.
//! An empty MMU implies only physical addressing on the bus is allowed.
use bitflags::bitflags;
use std::{collections::BTreeMap, sync::Arc};

use crate::soc::{bus::{BusError, BusResult, DeviceBus}, device::Device};

type VirtAddr = usize;
type PhysAddr = usize;

// TLB Entry Flags
bitflags! {
    #[derive(PartialEq, Copy, Clone)]
    pub struct MmuFlags: u32 {
        const VALID     = 0b1;
        const READ      = 0b10;
        const WRITE     = 0b100;
        const EXEC      = 0b1000;
        const RAM       = 0b1_0000;
        const BIGENDIAN = 0b10_0000; // 0 = Little, 1 = Big
    }
}
struct MmuEntry {
    pub vaddr: VirtAddr,
    pub paddr: PhysAddr,
    pub size: usize,
    pub flags: MmuFlags,
}
pub struct SoftMmu {
    regions: BTreeMap<usize, MmuEntry>,
    bus: Arc<DeviceBus>,
}

impl SoftMmu {
    pub fn new(bus: Arc<DeviceBus>) -> Self {
        Self { bus, regions: BTreeMap::new() }
    }

    pub fn map_region(&mut self, vaddr: VirtAddr, paddr: PhysAddr, size: usize, flags: MmuFlags) {
        let region = MmuEntry {
            vaddr,
            paddr,
            size,
            flags,
        };

        self.regions.insert(vaddr, region);
    }

    pub fn unmap_region(&mut self, vaddr: VirtAddr) {
        self.regions.remove(&vaddr);
    }

    pub fn translate(&self, vaddr: VirtAddr) -> BusResult<(usize, MmuFlags, *mut dyn Device)> {
        let (_, region) = self.regions.range(..=vaddr).next_back().ok_or(
            BusError::PageFault {details: format!("No mapping for virtual address {:#X}", vaddr)}
        )?;

        if vaddr >= region.vaddr + region.size {
            return Err(BusError::PageFault {details: format!(
                "Address {:#X} outside of mapped region [{:#X} - {:#X}]",
                vaddr,
                region.vaddr,
                region.vaddr + region.size
            )});
        }

        let addend = region.paddr.wrapping_sub(region.vaddr);
        let paddr = vaddr.wrapping_add(addend);
        let mmio_ptr = self.bus.get_mmio_ptr(paddr)?;

        Ok((addend, region.flags, mmio_ptr))
    }


}

