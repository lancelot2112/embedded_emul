use crate::soc::{bus::{BusResult, DataHandle}};

pub trait BitDataHandleExt {
    fn read_bits(&mut self, msb0: u8, bitlen: u8) -> BusResult<u64>;
    fn write_bits(&mut self, msb0: u8, bitlen: u8, value: u64) -> BusResult<()>;
}

impl BitDataHandleExt for DataHandle {
    // Reads a bitfield starting at the given msb0 offset with the specified length.
    // Returns the value right-aligned.
    // For example, reading 5 bits at msb0=3 from the short 0b111|0_1011|_0010_1010 would return 0b10110

    fn read_bits(&mut self, msb0: u8, bitlen: u8) -> BusResult<u64> {
        if bitlen == 0 || self.last_size == 0{
            return Ok(0);
        }
        //total bits from leftmost to right most
        let total_bits = msb0 as usize + bitlen as usize;
        let total_bytes = (total_bits).div_ceil(8);
        let msbit = total_bytes * 8;
        self.fetch(total_bytes).map(|value| {
            let shifted = value >> (msbit - total_bits);
            let mask = (1u64 << bitlen) - 1;
            shifted & mask
        })
    }

    fn write_bits(&mut self, msb0: u8, bitlen: u8, value: u64) -> BusResult<()> {
        if bitlen == 0 {
            return Ok(());
        }
        //total bits from leftmost to right most
        let total_bits = msb0 as usize + bitlen as usize;
        let total_bytes = (total_bits).div_ceil(8);
        let msbit = total_bytes * 8;
        let result = self.fetch(total_bytes).and_then(|current| {
            let mask = (1u64 << bitlen) - 1;
            let shifted_mask = 
            let mask = mask_bits(bit_len as usize) << (msbit - total_bits);
            let cleared = current & !mask;
            let new_value = cleared | ((value << (msbit - total_bits)) & mask);
            self.write_data(new_value, total_bytes)
        });
        result
    }
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
        let raw = handle.fetch(2).expect("read raw");
        assert_eq!(raw, 0x1234, "raw read matches expected {raw:04X}");
        
        let value = handle.read_bits(0, 12).expect("read bits");
        handle.address_mut().jump(0).unwrap();
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
        let raw = handle.fetch(2).expect("read raw");
        assert_eq!(raw, 0x05AF, "raw data reflects bit write {raw:04X}");

        handle.address_mut().jump(0).unwrap();
        let value = handle.read_bits(4, 8).expect("read back bits");
        assert_eq!(value as u8, 0x5A, "bit range should retain written value");
    }
}