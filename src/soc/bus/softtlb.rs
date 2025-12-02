
use std::sync::Arc;

use crate::soc::bus::softmmu::{SoftMmu, MmuFlags};
use crate::soc::bus::{BusError, BusResult};
use crate::soc::device::{AccessContext, Device, context};



pub trait EndianWord: Copy {
    fn to_host(self, source_bigendian: MmuFlags) -> Self;
    fn from_host(self, target_bigendian: MmuFlags) -> Self;
}

macro_rules! impl_word {
    ($t:ty) => {
        impl EndianWord for $t {
            #[inline(always)]
            fn to_host(self, flags: MmuFlags) -> Self {
                match (flags & MmuFlags::BIGENDIAN) {
                    MmuFlags::BIGENDIAN => Self::from_be(self),
                    _ => Self::from_le(self),
                }
            }

            #[inline(always)]
            fn from_host(self, flags: MmuFlags) -> Self {
                match (flags & MmuFlags::BIGENDIAN) {
                    MmuFlags::BIGENDIAN => Self::to_be(self),
                    _ => Self::to_le(self),
                }
            }
        }
    };
}

impl_word!(u8);
impl_word!(u16);
impl_word!(u32);
impl_word!(u64);
impl_word!(u128);

pub struct TlbEntry {
    pub vpn: usize,           // Virtual Page Number

    // THE MAGIC:
    // For RAM: This is (HostAddress - GuestAddress). 
    //          We add this to vaddr to get the host pointer.
    // For MMIO: This is 0 (or a specific marker).
    pub addend: usize,

    // Helper to tell if this is RAM, MMIO, or Invalid/Empty
    // Also holds permissions (R/W/X)
    pub flags: MmuFlags,

    // If flags says MMIO, we look up the device in this secondary field
    // (Can be an index into a device array, or a raw pointer to the Trait Object)
    pub mmio_ptr: *mut dyn Device,
}

impl TlbEntry {
    pub fn new(vpn: usize, addend: usize, flags: MmuFlags, mmio_ptr: *mut dyn Device) -> Self {
        Self {
            vpn,
            addend,
            flags,
            mmio_ptr,
        }
    }

    
}

pub struct SoftTlb {
    tlb: Vec<TlbEntry>,
    mmu: Arc<SoftMmu>,
    addr_size: usize,
    context: AccessContext,
}

impl SoftTlb {
    pub fn new(mmu: Arc<SoftMmu>, addr_size: usize, context: AccessContext) -> Self {
        Self {
            tlb: vec![],
            mmu,
            addr_size,
            context,
        }
    }

    #[inline(always)]
    fn get_tlb_idx(&self, vaddr: usize) -> usize {
        // Simple hash function: mask the middle bits
        (vaddr >> 12) & 0xFF // Example: 256-entry TLB
    }

    #[inline(always)]
    pub fn translate(&mut self, vaddr: usize) -> BusResult<&TlbEntry> {
        // 1. Indexing: Fast hash (masking)
        let idx = self.get_tlb_idx(vaddr);
        let entry = &self.tlb[idx];
        // 2. Check the tag
        if (entry.flags & MmuFlags::VALID) != MmuFlags::VALID || entry.vpn != (vaddr & !0xFFF) {
            // TLB MISS
            self.fetch_after_miss(vaddr, idx)?;
        }
        
        Ok(&self.tlb[idx])
    }

    #[cold]
    #[inline(always)]
    pub fn fetch_after_miss(&mut self, vaddr: usize, idx: usize) -> BusResult<()> {
        // 3. TLB Miss: Consult the MMU
        let mmu_entry = self.mmu.translate(vaddr);
        match mmu_entry {
            Ok(mmu_entry) => {
                // Update the TLB entry
                self.tlb[idx] = TlbEntry {
                    vpn: vaddr & !0xFFF,
                    addend: mmu_entry.addend,
                    flags: mmu_entry.flags,
                    mmio_ptr: mmu_entry.mmio_ptr,
                };
                Ok(())
            }
            Err(_) => Err(BusError::InvalidAddress{ address: vaddr }),
        }
    }

    pub fn get_ram_slice(&mut self, vaddr: usize, size: usize) -> BusResult<&[u8]> {
        let entry = self.translate(vaddr)?;

        // 3. TLB Hit
        if (entry.flags & MmuFlags::RAM) == MmuFlags::RAM{
            //RAM Access (FAST PATH)
            unsafe {
                // Calculate exact host address: 
                let host_ptr = vaddr.wrapping_add(entry.addend) as *const u8;
                let slice = std::slice::from_raw_parts(host_ptr, size);
                return Ok(slice);
            }
        }

        // MMIO Access (SLOW PATH)
        Err(BusError::InvalidAddress{ address: vaddr }) // Slices not supported for MMIO
    }

    pub fn write_ram_slice(&mut self, vaddr: usize, data: &[u8]) -> BusResult<()> {
        let entry = self.translate(vaddr)?;

        // 3. TLB Hit
        if (entry.flags & MmuFlags::RAM) == MmuFlags::RAM{
            //RAM Access (FAST PATH)
            unsafe {
                // Calculate exact host address: 
                let host_ptr = vaddr.wrapping_add(entry.addend) as *mut u8;
                let slice = std::slice::from_raw_parts_mut(host_ptr, data.len());
                slice.copy_from_slice(data);
                return Ok(());
            }
        }

        // MMIO Access (SLOW PATH)
        Err(BusError::InvalidAddress{ address: vaddr }) // Slices not supported for MMIO
    }

    pub fn read<T>(&mut self, vaddr: usize) -> BusResult<T> where T: EndianWord {
        let entry = self.translate(vaddr)?;

        // 3. TLB Hit
        if (entry.flags & MmuFlags::RAM) == MmuFlags::RAM{
            //RAM Access (FAST PATH)
            unsafe {
                // Calculate exact host address: 
                let host_ptr = vaddr.wrapping_add(entry.addend) as *const T;
                let raw = std::ptr::read_unaligned(host_ptr);
                return Ok(raw.to_host(entry.flags));
            }
        }

        // MMIO Access (SLOW PATH)
        self.read_dev::<T>(vaddr)
    }

    #[cold]
    fn read_dev<T>(&self, vaddr: usize) -> BusResult<T> where T: EndianWord
    {
        let entry = &self.tlb[self.get_tlb_idx(vaddr)];
        debug_assert!((entry.flags & MmuFlags::VALID) == MmuFlags::VALID, "Expect valid entries when this is called");

        // Use a temporary buffer sized to T
        let mut buf = [0u8; 8];

        unsafe {
            // SAFETY: the MMU filled mmio_ptr with a live device, and SoftTlb
            // holds the only mutable reference while servicing this access.
            let device = &mut *entry.mmio_ptr;
            let offset = vaddr.wrapping_add(entry.addend);
            device.read(offset, &mut buf[0..std::mem::size_of::<T>()], self.context)?;

            // Convert bytes to T
            let buf_ptr = buf.as_ptr() as *const T;
            let raw = std::ptr::read_unaligned(buf_ptr);
            Ok(raw.to_host(entry.flags))
        }
    }

    pub fn write<T>(&mut self, vaddr: usize, value: T) -> BusResult<()> where T: EndianWord {
        // 1. Indexing: Fast hash (masking)
        let entry = self.translate(vaddr)?;

        // Tlb Hit
        if (entry.flags & MmuFlags::RAM) == MmuFlags::RAM {
            // 3. TLB Hit
            unsafe {
                // Calculate exact host address: 
                let host_ptr = vaddr.wrapping_add(entry.addend) as *mut T;
                std::ptr::write_unaligned(host_ptr, value.from_host(entry.flags));
                return Ok(());
            }
        } 

        // 4. Slow Path (TLB Miss OR MMIO)
        self.write_dev::<T>(vaddr, value)
    }

    #[cold]
    pub fn write_dev<T>(&mut self, vaddr: usize, value: T) -> BusResult<()> where T: EndianWord {
        let entry = &self.tlb[self.get_tlb_idx(vaddr)];
        debug_assert!((entry.flags & MmuFlags::VALID) == MmuFlags::VALID, "Expect valid entries when this is called");

        // Use a temporary buffer sized to T
        let mut buf = [0u8; 8];
        let buf_ptr = buf.as_mut_ptr() as *mut T;
        unsafe {
            std::ptr::write_unaligned(buf_ptr, value.from_host(entry.flags));

            // SAFETY: the MMU filled mmio_ptr with a live device, and SoftTlb
            // holds the only mutable reference while servicing this access.
            let device = &mut *entry.mmio_ptr;
            let offset = vaddr.wrapping_add(entry.addend);
            device.write(offset, &buf[0..std::mem::size_of::<T>()], self.context)?;
            Ok(())
        }
    }
}