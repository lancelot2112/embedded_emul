use std::{ops::Range, sync::RwLock};

use super::error::{BusError, BusResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Endianness {
    Little,
    Big,
}

impl Endianness {
    pub const fn native() -> Self {
        if cfg!(target_endian = "little") {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }

    fn read_u16(self, bytes: [u8; 2]) -> u16 {
        match self {
            Endianness::Little => u16::from_le_bytes(bytes),
            Endianness::Big => u16::from_be_bytes(bytes),
        }
    }

    fn read_u32(self, bytes: [u8; 4]) -> u32 {
        match self {
            Endianness::Little => u32::from_le_bytes(bytes),
            Endianness::Big => u32::from_be_bytes(bytes),
        }
    }

    fn read_u64(self, bytes: [u8; 8]) -> u64 {
        match self {
            Endianness::Little => u64::from_le_bytes(bytes),
            Endianness::Big => u64::from_be_bytes(bytes),
        }
    }

    fn write_u16(self, value: u16) -> [u8; 2] {
        match self {
            Endianness::Little => value.to_le_bytes(),
            Endianness::Big => value.to_be_bytes(),
        }
    }

    fn write_u32(self, value: u32) -> [u8; 4] {
        match self {
            Endianness::Little => value.to_le_bytes(),
            Endianness::Big => value.to_be_bytes(),
        }
    }

    fn write_u64(self, value: u64) -> [u8; 8] {
        match self {
            Endianness::Little => value.to_le_bytes(),
            Endianness::Big => value.to_be_bytes(),
        }
    }
}

pub trait Device: Send + Sync {
    fn name(&self) -> &str;
    fn span(&self) -> Range<u64>;
    fn endianness(&self) -> Endianness {
        Endianness::Little
    }

    fn read(&self, offset: u64, buf: &mut [u8]) -> BusResult<()>;
    fn write(&self, offset: u64, data: &[u8]) -> BusResult<()>;

    fn read_u8(&self, offset: u64) -> BusResult<u8> {
        let mut buf = [0_u8; 1];
        self.read(offset, &mut buf)?;
        Ok(buf[0])
    }

    fn write_u8(&self, offset: u64, value: u8) -> BusResult<()> {
        let buf = [value];
        self.write(offset, &buf)
    }

    fn read_u16(&self, offset: u64) -> BusResult<u16> {
        let mut buf = [0_u8; 2];
        self.read(offset, &mut buf)?;
        Ok(self.endianness().read_u16(buf))
    }

    fn write_u16(&self, offset: u64, value: u16) -> BusResult<()> {
        let buf = self.endianness().write_u16(value);
        self.write(offset, &buf)
    }

    fn read_u32(&self, offset: u64) -> BusResult<u32> {
        let mut buf = [0_u8; 4];
        self.read(offset, &mut buf)?;
        Ok(self.endianness().read_u32(buf))
    }

    fn write_u32(&self, offset: u64, value: u32) -> BusResult<()> {
        let buf = self.endianness().write_u32(value);
        self.write(offset, &buf)
    }

    fn read_u64(&self, offset: u64) -> BusResult<u64> {
        let mut buf = [0_u8; 8];
        self.read(offset, &mut buf)?;
        Ok(self.endianness().read_u64(buf))
    }

    fn write_u64(&self, offset: u64, value: u64) -> BusResult<()> {
        let buf = self.endianness().write_u64(value);
        self.write(offset, &buf)
    }
}

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

    pub fn size(&self) -> u64 {
        self.bytes.read().unwrap().len() as u64
    }
}

impl Device for BasicMemory {
    fn name(&self) -> &str {
        &self.name
    }

    fn span(&self) -> Range<u64> {
        0..self.size()
    }

    fn endianness(&self) -> Endianness {
        self.endian
    }

    fn read(&self, offset: u64, buf: &mut [u8]) -> BusResult<()> {
        let len = buf.len() as u64;
        let data = self.bytes.read().unwrap();
        if offset + len > data.len() as u64 {
            return Err(BusError::OutOfRange { address: offset + len, end: data.len() as u64 });
        }
        let start = offset as usize;
        let end = start + buf.len();
        buf.copy_from_slice(&data[start..end]);
        Ok(())
    }

    fn write(&self, offset: u64, data_in: &[u8]) -> BusResult<()> {
        let len = data_in.len() as u64;
        let mut data = self.bytes.write().unwrap();
        if offset + len > data.len() as u64 {
            return Err(BusError::OutOfRange { address: offset + len, end: data.len() as u64 });
        }
        let start = offset as usize;
        let end = start + data_in.len();
        data[start..end].copy_from_slice(data_in);
        Ok(())
    }
}
