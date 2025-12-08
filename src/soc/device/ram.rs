use std::{ops::Range, sync::Mutex};

use crate::soc::device::{AccessContext, Device, DeviceError, DeviceResult, Endianness};

pub struct RamMemory {
    name: String,
    bytes: Mutex<Vec<u8>>,
    len: usize,
    endian: Endianness,
}

impl RamMemory {
    pub fn new(name: impl Into<String>, len: usize, endian: Endianness) -> Self {
        Self {
            name: name.into(),
            bytes: Mutex::new(vec![0_u8; len + 7]), //Add 7 bytes to allow a u64 read up to the end of the array.
            len,
            endian,
        }
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub fn ptr_at(&self, offset: usize) -> *const u8 {
        debug_assert!(offset < self.len);
        let bytes = self.bytes.lock().unwrap();
        unsafe { bytes.as_ptr().add(offset) }
    }

    #[inline(always)]
    pub fn ptr_at_mut(&self, offset: usize) -> *mut u8 {
        debug_assert!(offset < self.len);
        let mut bytes = self.bytes.lock().unwrap();
        unsafe { bytes.as_mut_ptr().add(offset) }
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
    fn as_ram(&self) -> Option<&RamMemory> {
        Some(self)
    }

    fn read(&self, offset: usize, out: &mut [u8], _ctx: AccessContext) -> DeviceResult<()> {
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
        let bytes = self.bytes.lock().unwrap();
        out.copy_from_slice(&bytes[offset..end]);
        Ok(())
    }

    fn write(&self, offset: usize, data_in: &[u8], _ctx: AccessContext) -> DeviceResult<()> {
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
        let mut bytes = self.bytes.lock().unwrap();
        bytes[offset..end].copy_from_slice(data_in);
        Ok(())
    }
}
