use crate::soc::{bus::{BusResult, DataHandle}, device::{endianness::MAX_ENDIAN_BYTES}};

const MAX_SLICE_BYTES: usize = MAX_ENDIAN_BYTES;
const MAX_SLICE_BITS: u16 = (MAX_SLICE_BYTES * 8) as u16;

pub trait BitDataHandleExt {
    fn read_bits(&mut self, bit_offset: u8, bit_len: u16) -> BusResult<u64>;
    fn write_bits(&mut self, bit_offset: u8, bit_len: u16, value: u64) -> BusResult<()>;
}
struct SliceCursor {
    bit_offset: u16,
    bit_len: u16,
}

impl BitDataHandleExt for DataHandle {
    fn read_bits(&mut self, bit_offset: u8, bit_len: u16) -> BusResult<u64> {
        if bit_len == 0 {
            return Ok(0);
        }
        let total_bytes = (bit_offset as usize + bit_len as usize).div_ceil(8);
        self.read_data(total_bytes).map(|value| {
            let shifted = value >> (bit_offset as u32);
            let mask = mask_bits(bit_len as usize);
            shifted & mask
        })
    }

    fn write_bits(&mut self, bit_offset: u8, bit_len: u16, value: u64) -> BusResult<()> {
        if bit_len == 0 {
            return Ok(());
        }
        let result = self.read_bits(bit_offset, bit_len).and_then(|current| {
            let mask = mask_bits(bit_len as usize);
            let cleared = current & !(mask << (bit_offset as u32));
            let new_value = cleared | ((value & mask) << (bit_offset as u32));
            let total_bytes = (bit_offset as usize + bit_len as usize).div_ceil(8);
            self.write_data(new_value, total_bytes)
        });
        result
    }
}

#[inline]
fn bits_to_bytes(bit_offset: u8, bit_len: u16) -> usize {
    let total_bits = bit_offset as usize + bit_len as usize;
    total_bits.div_ceil(8).max(1)
}

#[inline]
fn mask_bits(len: usize) -> u64 {
    if len >= 64 {
        u64::MAX
    } else {
        (1u64 << len) - 1
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::soc::{bus::DeviceBus, device::{BasicMemory, Device, Endianness}};

    use super::*;
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
        handle.write_bits(4, 8, 0x5Au64).expect("write bits");
        handle.address_mut().jump(0).unwrap();
        let value = handle.read_bits(4, 8).expect("read back bits");
        assert_eq!(value as u8, 0x5A, "bit range should retain written value");
    }
}