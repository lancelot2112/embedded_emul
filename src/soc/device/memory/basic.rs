use std::{ops::Range, sync::{RwLock, RwLockReadGuard, RwLockWriteGuard}};

use crate::soc::device::{Device, DeviceError, DeviceResult, Endianness};

pub struct BasicMemory {
    name: String,
    bytes: RwLock<Vec<u8>>,
    endian: Endianness,
}

impl BasicMemory {
    pub fn new(name: impl Into<String>, size: usize, endian: Endianness) -> Self {
        Self {
            name: name.into(),
            bytes: RwLock::new(vec![0_u8; size]),
            endian,
        }
    }

    pub fn size(&self) -> usize {
        self.bytes.read().unwrap().len()
    }
}

impl Device for BasicMemory {
    fn name(&self) -> &str {
        &self.name
    }

    fn span(&self) -> Range<usize> {
        0..self.size()
    }

    fn endianness(&self) -> Endianness {
        self.endian
    }

    fn borrow(&self, byte_offset: usize, len: usize) -> DeviceResult<RwLockReadGuard<'_, Vec<u8>>> {
        let start = byte_offset as usize;
        let end = start + len;
        let data = self.bytes.read().map_err(|_| DeviceError::LockPoisoned(format!("read from {}", self.name)))?;
        if end > data.len() {
            return Err(DeviceError::OutOfRange {
                offset: byte_offset,
                len,
                capacity: data.len(),
            });
        }
        Ok(data)
    }

    fn borrow_mut(&self, byte_offset: usize, len: usize) -> DeviceResult<RwLockWriteGuard<'_, Vec<u8>>> {
        let start = byte_offset as usize;
        let end = start + len;
        let data = self.bytes.write().map_err(|_| DeviceError::LockPoisoned(format!("write to {}", self.name)))?;
        if end > data.len() {
            return Err(DeviceError::OutOfRange {
                offset: byte_offset,
                len,
                capacity: data.len(),
            });
        }
        Ok(data)
    }

    fn read(&self, byte_offset: usize, out: &mut [u8]) -> DeviceResult<()> {
        if out.is_empty() {
            return Ok(());
        }
        let start = byte_offset as usize;
        let end = start + out.len();
        let data = self.bytes.read().unwrap();
        if end > data.len() {
            return Err(DeviceError::OutOfRange {
                offset: byte_offset,
                len: out.len(),
                capacity: data.len(),
            });
        }
        out.copy_from_slice(&data[start..end]);
        Ok(())
    }

    fn write(&self, byte_offset: usize, data_in: &[u8]) -> DeviceResult<()> {
        if data_in.is_empty() {
            return Ok(());
        }
        let start = byte_offset as usize;
        let end = start + data_in.len();
        let mut data = self.bytes.write().unwrap();
        if end > data.len() {
            return Err(DeviceError::OutOfRange {
                offset: byte_offset,
                len: data_in.len(),
                capacity: data.len(),
            });
        }
        data[start..end].copy_from_slice(data_in);
        Ok(())
    }
}
