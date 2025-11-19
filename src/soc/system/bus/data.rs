//! Typed data access wrapper layered on AddressHandle offering scalar helpers
//! and std::io traits for interacting with DeviceBus-backed memory regions.
use std::{
    io::{self, Read, Write},
    mem,
    sync::Arc,
};

use super::{
    DeviceBus,
    address::AddressHandle,
    error::{BusError, BusResult},
    range::ResolvedRange,
};

use crate::soc::device::{
    Device, DeviceError, DeviceResult, Endianness,
    endianness::{MAX_ENDIAN_BYTES, mask_bits},
};
use crate::soc::system::bus::ext::stream::ByteDataHandleExt;

pub struct DataHandle {
    address: AddressHandle,
    cache: BitSliceCache,
}

impl DataHandle {
    pub fn new(bus: Arc<DeviceBus>) -> Self {
        Self {
            address: AddressHandle::new(bus),
            cache: BitSliceCache::default(),
        }
    }

    pub fn address(&self) -> &AddressHandle {
        &self.address
    }

    pub fn address_mut(&mut self) -> &mut AddressHandle {
        &mut self.address
    }

    pub fn available(&self, size: u64) -> bool {
        self.address.available(size)
    }

    // Byte-wise interface -------------------------------------------------

    pub fn read(&mut self, out: &mut [u8]) -> BusResult<()> {
        if out.is_empty() {
            return Ok(());
        }
        let span = out.len() as u64;
        let mut cache = mem::take(&mut self.cache);
        let result = self.address.transact(span, |device, offset, _resolved| {
            let outcome = with_device_transaction(device, || {
                device.read(offset, out).map_err(map_device_err)
            });
            cache.invalidate();
            outcome
        });
        self.cache = cache;
        result
    }

    pub fn write(&mut self, data: &[u8]) -> BusResult<()> {
        if data.is_empty() {
            return Ok(());
        }
        let span = data.len() as u64;
        let mut cache = mem::take(&mut self.cache);
        let result = self.address.transact(span, |device, offset, _resolved| {
            let outcome = with_device_transaction(device, || {
                device.write(offset, data).map_err(map_device_err)
            });
            cache.invalidate();
            outcome
        });
        self.cache = cache;
        result
    }

    pub fn read_bits(&mut self, bit_offset: u8, bit_len: u16) -> BusResult<u128> {
        if bit_len == 0 {
            return Ok(0);
        }
        let byte_span = bits_to_bytes(bit_offset, bit_len) as u64;
        let mut cache = mem::take(&mut self.cache);
        let result = self
            .address
            .transact(byte_span, |device, offset, resolved| {
                with_device_transaction(device, || {
                    let cursor =
                        cache.ensure_slice(device, resolved, offset, bit_offset, bit_len)?;
                    let value = cache.extract_target_bits(
                        cursor.bit_offset,
                        cursor.bit_len,
                        resolved.device.endianness(),
                    );
                    Ok(value)
                })
            });
        self.cache = cache;
        result
    }

    pub fn write_bits(&mut self, bit_offset: u8, bit_len: u16, value: u128) -> BusResult<()> {
        if bit_len == 0 {
            return Ok(());
        }
        let byte_span = bits_to_bytes(bit_offset, bit_len) as u64;
        let mut cache = mem::take(&mut self.cache);
        let result = self
            .address
            .transact(byte_span, |device, offset, resolved| {
                let outcome = with_device_transaction(device, || {
                    let cursor =
                        cache.ensure_slice(device, resolved, offset, bit_offset, bit_len)?;
                    let chunk_bits = cache.chunk_bits();
                    let chunk_bytes = cache.chunk_byte_len();
                    let device_endian = resolved.device.endianness();
                    let current_value = cache.chunk_value(device_endian);

                    let byte_len = bytes_for_len(bit_len);
                    let write_bytes = device_endian.encode_bits(value, bit_len as usize, byte_len);
                    let value_bits =
                        device_endian.decode_bits(&write_bytes[..byte_len], bit_len as usize);

                    let mask = mask_bits(bit_len as usize) << (cursor.bit_offset as u32);
                    let updated = (current_value & !mask)
                        | ((value_bits & mask_bits(bit_len as usize))
                            << (cursor.bit_offset as u32));

                    let encoded_chunk =
                        device_endian.encode_bits(updated, chunk_bits as usize, chunk_bytes);
                    device
                        .write(cache.base_byte(), &encoded_chunk[..chunk_bytes])
                        .map_err(map_device_err)
                });
                cache.invalidate();
                outcome
            });
        self.cache = cache;
        result
    }
}

const MAX_SLICE_BYTES: usize = MAX_ENDIAN_BYTES;
const MAX_SLICE_BITS: u16 = (MAX_SLICE_BYTES * 8) as u16;

struct SliceCursor {
    bit_offset: u16,
    bit_len: u16,
}

#[derive(Default)]
struct BitSliceCache {
    device_id: Option<usize>,
    base_byte: u64,
    word_count: usize,
    device_bytes: [u8; MAX_SLICE_BYTES],
    target_value: Option<(Endianness, u128)>,
}

impl BitSliceCache {
    fn invalidate(&mut self) {
        self.device_id = None;
        self.word_count = 0;
        self.target_value = None;
    }

    fn ensure_slice(
        &mut self,
        device: &dyn Device,
        resolved: &ResolvedRange,
        device_offset: u64,
        bit_offset: u8,
        bit_len: u16,
    ) -> DeviceResult<SliceCursor> {
        let base = device_offset & !7;
        let intra = ((device_offset - base) * 8) as u16 + bit_offset as u16;
        if intra + bit_len > MAX_SLICE_BITS {
            return Err(DeviceError::Unsupported("bit slice exceeds cache window"));
        }
        let word_count = if intra + bit_len > 64 { 2 } else { 1 };
        self.ensure_loaded(device, resolved.device_id, base, word_count)?;
        Ok(SliceCursor {
            bit_offset: intra,
            bit_len,
        })
    }

    fn ensure_loaded(
        &mut self,
        device: &dyn Device,
        device_id: usize,
        base_byte: u64,
        word_count: usize,
    ) -> DeviceResult<()> {
        if self.matches(device_id, base_byte, word_count) {
            return Ok(());
        }
        let byte_len = word_count * 8;
        device.read(base_byte, &mut self.device_bytes[..byte_len])?;
        for idx in byte_len..MAX_SLICE_BYTES {
            self.device_bytes[idx] = 0;
        }
        self.device_id = Some(device_id);
        self.base_byte = base_byte;
        self.word_count = word_count;
        self.target_value = None;
        Ok(())
    }

    fn matches(&self, device_id: usize, base_byte: u64, word_count: usize) -> bool {
        self.device_id == Some(device_id)
            && self.base_byte == base_byte
            && self.word_count == word_count
    }

    fn extract_target_bits(
        &mut self,
        bit_offset: u16,
        bit_len: u16,
        device_endian: Endianness,
    ) -> u128 {
        let value = self.ensure_chunk_value(device_endian);
        let chunk_bits = self.chunk_bits() as usize;
        match device_endian {
            Endianness::Little => (value >> bit_offset) & mask_bits(bit_len as usize),
            Endianness::Big => {
                let shift = chunk_bits - (bit_offset as usize + bit_len as usize);
                (value >> shift) & mask_bits(bit_len as usize)
            }
        }
    }

    fn ensure_chunk_value(&mut self, device_endian: Endianness) -> u128 {
        let needs_update = match self.target_value {
            Some((cached, _)) if cached == device_endian => false,
            _ => true,
        };
        if needs_update {
            let byte_len = self.word_count * 8;
            let value = device_endian.decode_bytes(&self.device_bytes[..byte_len]);
            self.target_value = Some((device_endian, value));
        }
        self.target_value
            .as_ref()
            .map(|(_, value)| *value)
            .unwrap_or(0)
    }

    fn chunk_bits(&self) -> u16 {
        (self.word_count * 64) as u16
    }

    fn chunk_byte_len(&self) -> usize {
        self.word_count * 8
    }

    fn chunk_value(&mut self, device_endian: Endianness) -> u128 {
        self.ensure_chunk_value(device_endian)
    }

    fn base_byte(&self) -> u64 {
        self.base_byte
    }
}

fn bits_to_bytes(bit_offset: u8, bit_len: u16) -> usize {
    let total_bits = bit_offset as usize + bit_len as usize;
    ((total_bits + 7) / 8).max(1)
}

fn bytes_for_len(bit_len: u16) -> usize {
    ((bit_len as usize + 7) / 8).max(1)
}

fn map_device_err(err: DeviceError) -> DeviceError {
    err
}

fn with_device_transaction<F, T>(device: &dyn Device, mut body: F) -> DeviceResult<T>
where
    F: FnMut() -> DeviceResult<T>,
{
    device.start_transact().map_err(map_device_err)?;
    let body_result = body();
    let end_result = device.end_transact().map_err(map_device_err);
    match (body_result, end_result) {
        (Err(err), _) => Err(err),
        (Ok(_), Err(err)) => Err(err),
        (Ok(value), Ok(())) => Ok(value),
    }
}

fn io_error(err: BusError) -> io::Error {
    io::Error::other(err)
}

impl Read for DataHandle {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let available = self.address.bytes_to_end();
        if available == 0 {
            return Ok(0);
        }
        let count = available.min(buf.len() as u64) as usize;
        self.read_bytes(&mut buf[..count]).map_err(io_error)?;
        Ok(count)
    }
}

impl Write for DataHandle {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let available = self.address.bytes_to_end();
        if available == 0 {
            return Ok(0);
        }
        let count = available.min(buf.len() as u64) as usize;
        self.write_bytes(&buf[..count]).map_err(io_error)?;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{BasicMemory, Device, Endianness};
    use crate::soc::system::bus::DeviceBus;

    #[test]
    fn read_write_round_trip() {
        let bus = Arc::new(DeviceBus::new(12));
        let memory = Arc::new(BasicMemory::new("ram", 0x1000, Endianness::Little));
        bus.register_device(memory, 0x1000).unwrap();

        let mut handle = DataHandle::new(bus.clone());
        handle.address_mut().jump(0x1000).unwrap();
        handle.write(&[0xEF, 0xBE, 0xAD, 0xDE]).unwrap();
        handle.address_mut().jump(0x1000).unwrap();
        let mut buf = [0u8; 4];
        handle.read(&mut buf).unwrap();
        assert_eq!(
            u32::from_le_bytes(buf),
            0xDEADBEEF,
            "scalar helper should round trip the written value"
        );
    }

    #[test]
    fn redirect_allows_alias_reads() {
        let bus = Arc::new(DeviceBus::new(10));
        let memory = Arc::new(BasicMemory::new("flash", 0x2000, Endianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();

        let mut preload = DataHandle::new(bus.clone());
        preload.address_mut().jump(0x150).unwrap();
        preload.write(&[0x12, 0x34, 0x56, 0x78]).unwrap();
        bus.redirect(0x4000, 4, 0x150).unwrap();

        let mut handle = DataHandle::new(bus);
        handle.address_mut().jump(0x4000).unwrap();
        let mut buf = [0u8; 4];
        handle.read(&mut buf).unwrap();
        assert_eq!(
            u32::from_le_bytes(buf),
            0x78563412,
            "handle should read bytes through the redirect alias"
        );
    }

    #[test]
    fn bit_reads_handle_offsets() {
        let bus = Arc::new(DeviceBus::new(8));
        let memory = Arc::new(BasicMemory::new("ram", 0x20, Endianness::Big));
        bus.register_device(memory.clone(), 0).unwrap();
        memory.write(0, &[0x12, 0x34]).expect("seed memory");

        let mut handle = DataHandle::new(bus.clone());
        handle.address_mut().jump(0).unwrap();
        let value = handle.read_bits(0, 12).expect("read bits");
        assert_eq!(value as u16, 0x123, "bit slice honors device endianness");
    }

    #[test]
    fn bit_writes_update_partial_ranges() {
        let bus = Arc::new(DeviceBus::new(8));
        let memory = Arc::new(BasicMemory::new("ram", 0x20, Endianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();
        memory.write(0, &[0x00, 0xFF]).expect("seed memory");

        let mut handle = DataHandle::new(bus.clone());
        handle.address_mut().jump(0).unwrap();
        handle.write_bits(4, 8, 0x5Au128).expect("write bits");
        handle.address_mut().jump(0).unwrap();
        let value = handle.read_bits(4, 8).expect("read back bits");
        assert_eq!(value as u8, 0x5A, "bit range should retain written value");
    }
}
