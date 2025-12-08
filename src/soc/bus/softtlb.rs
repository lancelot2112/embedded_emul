use crate::soc::bus::{
    BusError, BusResult, DeviceRef,
    softmmu::{AddressMode, MMUFlags, SoftMMU},
};
use crate::soc::device::AccessContext;

const TLB_SETS: usize = 256;
const MAX_WORD_BYTES: usize = 16;

pub trait EndianWord: Copy {
    fn to_host(self, source_bigendian: MMUFlags) -> Self;
    fn from_host(self, target_bigendian: MMUFlags) -> Self;
}

macro_rules! impl_word {
    ($t:ty) => {
        impl EndianWord for $t {
            #[inline(always)]
            fn to_host(self, flags: MMUFlags) -> Self {
                match (flags.contains(MMUFlags::BIGENDIAN)) {
                    true => Self::from_be(self),
                    false => Self::from_le(self),
                }
            }

            #[inline(always)]
            fn from_host(self, flags: MMUFlags) -> Self {
                match (flags.contains(MMUFlags::BIGENDIAN)) {
                    true => Self::to_be(self),
                    false => Self::to_le(self),
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

#[derive(Clone, Default)]
pub struct TLBEntry {
    pub vpn: usize, // Virtual Page Number

    // THE MAGIC:
    // For RAM: This is (HostAddress - GuestAddress).
    //          We add this to vaddr to get the host pointer.
    // For MMIO: This is 0 (or a specific marker).
    pub addend: usize,

    // Helper to tell if this is RAM, MMIO, or Invalid/Empty
    // Also holds permissions (R/W/X)
    pub flags: MMUFlags,

    // If flags says MMIO, we look up the device in this secondary field
    // (Can be an index into a device array, or a raw pointer to the Trait Object)
    pub device: Option<DeviceRef>,
}

impl TLBEntry {
    pub fn new(vpn: usize, addend: usize, flags: MMUFlags, device: Option<DeviceRef>) -> Self {
        Self {
            vpn,
            addend,
            flags,
            device,
        }
    }
}

pub struct SoftTLB {
    tlb: Vec<TLBEntry>,
    mmu: SoftMMU,
    context: AccessContext,
}

impl SoftTLB {
    pub fn new(mmu: SoftMMU, context: AccessContext) -> Self {
        Self {
            tlb: vec![TLBEntry::default(); TLB_SETS],
            mmu,
            context,
        }
    }

    pub fn set_address_mode(&mut self, mode: AddressMode) {
        if self.mmu.mode() != mode {
            self.mmu.set_mode(mode);
            self.invalidate_all();
        }
    }

    pub fn address_mode(&self) -> AddressMode {
        self.mmu.mode()
    }

    #[inline(always)]
    fn get_tlb_idx(&self, vaddr: usize) -> usize {
        // Simple hash function: mask the middle bits
        (vaddr >> 12) & 0xFF // Example: 256-entry TLB
    }

    #[inline(always)]
    pub fn lookup(&mut self, vaddr: usize) -> BusResult<&TLBEntry> {
        // 1. Indexing: Fast hash (masking)
        let idx = self.get_tlb_idx(vaddr);
        let entry = &self.tlb[idx];
        // 2. Check the tag
        if !entry.flags.contains(MMUFlags::VALID) || entry.vpn != (vaddr & !0xFFF) {
            // TLB MISS
            self.translate_after_miss(vaddr, idx)?;
        }

        Ok(&self.tlb[idx])
    }

    #[cold]
    #[inline(always)]
    pub fn translate_after_miss(&mut self, vaddr: usize, idx: usize) -> BusResult<()> {
        // 3. TLB Miss: Consult the MMU
        let (addend, flags, device) = self.mmu.translate(vaddr)?;
        // Update the TLB entry
        self.tlb[idx] = TLBEntry {
            vpn: vaddr & !0xFFF,
            addend,
            flags,
            device: Some(device),
        };
        Ok(())
    }

    pub fn read_ram(&mut self, vaddr: usize, size: usize) -> BusResult<&[u8]> {
        let entry = self.lookup(vaddr)?;

        // 3. TLB Hit
        if entry.flags.contains(MMUFlags::RAM) {
            //RAM Access (FAST PATH)
            unsafe {
                // Calculate exact host address:
                let host_ptr = vaddr.wrapping_add(entry.addend) as *const u8;
                let slice = std::slice::from_raw_parts(host_ptr, size);
                return Ok(slice);
            }
        }

        // MMIO Access (SLOW PATH)
        Err(BusError::InvalidAddress { address: vaddr }) // Slices not supported for MMIO
    }

    pub fn write_ram(&mut self, vaddr: usize, data: &[u8]) -> BusResult<()> {
        let entry = self.lookup(vaddr)?;

        // 3. TLB Hit
        if entry.flags.contains(MMUFlags::RAM) {
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
        Err(BusError::InvalidAddress { address: vaddr }) // Slices not supported for MMIO
    }

    pub fn peek<T>(&mut self, vaddr: usize) -> BusResult<T>
    where
        T: EndianWord,
    {
        self.read_internal::<T>(vaddr, AccessContext::DEBUG)
    }

    pub fn read<T>(&mut self, vaddr: usize) -> BusResult<T>
    where
        T: EndianWord,
    {
        self.read_internal::<T>(vaddr, self.context)
    }

    fn read_internal<T>(&mut self, vaddr: usize, context: AccessContext) -> BusResult<T>
    where
        T: EndianWord,
    {
        let entry = self.lookup(vaddr)?;

        // 3. TLB Hit
        if entry.flags.contains(MMUFlags::RAM) {
            //RAM Access (FAST PATH)
            unsafe {
                // Calculate exact host address:
                let host_ptr = vaddr.wrapping_add(entry.addend) as *const T;
                let raw = std::ptr::read_unaligned(host_ptr);
                return Ok(raw.to_host(entry.flags));
            }
        }

        // MMIO Access (SLOW PATH)
        self.read_dev::<T>(vaddr, context)
    }

    #[cold]
    fn read_dev<T>(&mut self, vaddr: usize, context: AccessContext) -> BusResult<T>
    where
        T: EndianWord,
    {
        let entry = self.lookup(vaddr)?;
        debug_assert!(
            (entry.flags & MMUFlags::VALID) == MMUFlags::VALID,
            "Expect valid entries when this is called"
        );

        let mut buf = [0u8; MAX_WORD_BYTES];
        let word_len = std::mem::size_of::<T>();
        let slice = &mut buf[..word_len];
        let device_ref = entry
            .device
            .as_ref()
            .ok_or(BusError::InvalidAddress { address: vaddr })?;
        let offset = vaddr.wrapping_add(entry.addend);
        device_ref.read(offset, slice, context)?;
        let raw = unsafe { std::ptr::read_unaligned(slice.as_ptr() as *const T) };
        Ok(raw.to_host(entry.flags))
    }

    pub fn write<T>(&mut self, vaddr: usize, value: T) -> BusResult<()>
    where
        T: EndianWord,
    {
        // 1. Indexing: Fast hash (masking)
        let entry = self.lookup(vaddr)?;

        // Tlb Hit
        if (entry.flags & MMUFlags::RAM) == MMUFlags::RAM {
            // 3. TLB Hit
            unsafe {
                // Calculate exact host address:
                let host_ptr = vaddr.wrapping_add(entry.addend) as *mut T;
                std::ptr::write_unaligned(host_ptr, value.from_host(entry.flags));
                return Ok(());
            }
        }

        // 4. Slow Path (TLB Miss OR MMIO)
        self.write_dev::<T>(vaddr, value, self.context)
    }

    #[cold]
    pub fn write_dev<T>(&mut self, vaddr: usize, value: T, context: AccessContext) -> BusResult<()>
    where
        T: EndianWord,
    {
        let entry = self.lookup(vaddr)?;
        debug_assert!(
            (entry.flags & MMUFlags::VALID) == MMUFlags::VALID,
            "Expect valid entries when this is called"
        );

        let mut buf = [0u8; MAX_WORD_BYTES];
        let word_len = std::mem::size_of::<T>();
        let slice = &mut buf[..word_len];
        unsafe {
            std::ptr::write_unaligned(slice.as_mut_ptr() as *mut T, value.from_host(entry.flags));
        }
        let device_ref = entry
            .device
            .as_ref()
            .ok_or(BusError::InvalidAddress { address: vaddr })?;
        let offset = vaddr.wrapping_add(entry.addend);
        device_ref.write(offset, slice, context)?;
        Ok(())
    }

    fn invalidate_all(&mut self) {
        for entry in &mut self.tlb {
            *entry = TLBEntry::default();
        }
    }
}
