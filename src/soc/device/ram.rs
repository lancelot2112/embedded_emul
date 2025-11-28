use std::ops::Range;

use crate::soc::device::{Device, DeviceError, DeviceResult, Endianness};

pub struct RamMemory {
    name: String,
    bytes: Vec<u8>,
    len: usize,
    endian: Endianness,
}

impl RamMemory {
    pub fn new(name: impl Into<String>, len: usize, endian: Endianness) -> Self {
        Self {
            name: name.into(),
            bytes: vec![0_u8; len+7], //Add 7 bytes to allow a u64 read up to the end of the array.
            len,
            endian,
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn ptr_at(&self, offset: usize) -> *const u8 {
        debug_assert!(offset < self.len);
        unsafe { self.bytes.as_ptr().add(offset) }
    }

    #[inline(always)]
    pub fn ptr_at_mut(&mut self, offset: usize) -> *mut u8 {
        debug_assert!(offset < self.len);
        unsafe { self.bytes.as_mut_ptr().add(offset) }
    }
}

impl Device for RamMemory {
    fn name(&self) -> &str {
        &self.name
    }

    
    #[inline(always)]
    fn span(&self) -> Range<usize> {
        0..self.len()
    }

    
    #[inline(always)]
    fn endianness(&self) -> Endianness {
        self.endian
    }

    #[inline(always)]
    fn peek(&self, offset: usize, len: usize) -> &[u8] {
        let end = offset + len;
        debug_assert!(end <= self.len, "Out of range slice requested from RamMemory");
        &self.bytes[offset .. end]
    }

    #[inline(always)]
    fn as_ram(&self) -> Option<&RamMemory> {
        Some(self)
    }

    #[inline(always)]
    fn as_ram_mut(&mut self) -> Option<&mut RamMemory> {
        Some(self)
    }

    fn read(&mut self, offset: usize, out: &mut [u8]) -> DeviceResult<()> {
        if out.is_empty() {
            return Ok(());
        }
        let end = offset + out.len();
        if end > self.len {
            return Err(DeviceError::OutOfRange {
                offset,
                len: out.len(),
                capacity: self.len,
            });
        }
        out.copy_from_slice(&self.bytes[offset..end]);
        Ok(())
    }

    fn write(&mut self, offset: usize, data_in: &[u8]) -> DeviceResult<()> {
        if data_in.is_empty() {
            return Ok(());
        }
        let end = offset + data_in.len();
        if end > self.len {
            return Err(DeviceError::OutOfRange {
                offset,
                len: data_in.len(),
                capacity: self.len,
            });
        }
        self.bytes[offset..end].copy_from_slice(data_in);
        Ok(())
    }
}
