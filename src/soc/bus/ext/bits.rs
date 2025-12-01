use crate::soc::bus::{BusError, BusResult, data::DataView, handle::StaticCursor};

pub trait BitDataViewExt {
    fn read_bits(&mut self, msb0: u8, bitlen: u8) -> BusResult<u64>;
    fn write_bits(&mut self, msb0: u8, bitlen: u8, value: u64) -> BusResult<()>;
}

impl BitDataViewExt for DataView<StaticCursor> {
    // Reads a bitfield starting at the given msb0 offset with the specified length.
    // Returns the value right-aligned.
    // For example, reading 5 bits at msb0=3 from the short 0b111|0_1011|_0010_1010 would return 0b10110

    fn read_bits(&mut self, msb0: u8, bitlen: u8) -> BusResult<u64> {
        if bitlen == 0 {
            return Ok(0);
        }

        //total bits from leftmost to right most
        let total_bits = msb0 as usize + bitlen as usize;
        let total_bytes = (total_bits).div_ceil(8);
        if total_bytes > 8 {
            return Err(BusError::OutOfRange {
                address: 0,
                end: total_bytes,
            });
        }

        let msbit = total_bytes * 8;
        let raw = self.read_u64()? >> (msbit - total_bits);
        let mask = mask_bits(bitlen as usize);
        Ok(raw & mask)
    }

    fn write_bits(&mut self, msb0: u8, bitlen: u8, value: u64) -> BusResult<()> {
        if bitlen == 0 {
            return Ok(());
        }
        //total bits from leftmost to right most
        let total_bits = msb0 as usize + bitlen as usize;
        let total_bytes = (total_bits).div_ceil(8);
        if total_bytes > 8 {
            return Err(BusError::OutOfRange {
                address: 0,
                end: total_bytes,
            });
        }
        let msbit = total_bytes * 8;
        let raw = self.read_u64()?;
        let mask = mask_bits(bitlen as usize);
        let shifted_mask = mask << (msbit - total_bits);
        let cleared = raw & !shifted_mask;
        let new_value = cleared | ((value << (msbit - total_bits)) & shifted_mask);
        self.write_u64(new_value)
    }
}

#[inline]
fn mask_bits(bitlen: usize) -> u64 {
    1u64.unbounded_shl(bitlen as u32).wrapping_sub(1)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::soc::{
        bus::{AddressHandle, DeviceBus},
        device::{Device, Endianness, RamMemory},
    };

    use super::*;
    #[test]
    fn bit_reads_handle_offsets() {
        let bus = Arc::new(DeviceBus::new(8));
        let memory = Arc::new(RamMemory::new("ram", 0x20, Endianness::Big));
        bus.register_device(memory.clone(), 0).unwrap();
        memory.write(0, &[0x12, 0x34]).expect("seed memory");

        let mut addr = AddressHandle::new(bus.clone());
        addr.jump(0).unwrap();
        let raw = addr
            .scalar_handle(2)
            .expect("read raw")
            .read()
            .expect("read succeeds");
        assert_eq!(raw, 0x1234, "raw read matches expected {raw:04X}");

        addr.jump(0).unwrap();
        let value = addr
            .scalar_handle(2)
            .expect("read bits")
            .read_bits(0, 12)
            .expect("read bits");
        assert_eq!(value as u16, 0x123, "bit slice honors device endianness");
    }

    #[test]
    fn bit_writes_update_partial_ranges() {
        let bus = Arc::new(DeviceBus::new(8));
        let memory = Arc::new(RamMemory::new("ram", 0x20, Endianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();
        memory.write(0, &[0x00, 0xFF]).expect("seed memory");

        let mut addr = AddressHandle::new(bus.clone());
        addr.jump(0).unwrap();
        addr.scalar_handle(2)
            .expect("write bits")
            .write_bits(4, 8, 0x5Au64)
            .expect("write bits");
        addr.jump(0).unwrap();
        let raw = addr
            .scalar_handle(2)
            .expect("read raw")
            .read()
            .expect("read succeeds");
        assert_eq!(raw, 0x05AF, "raw data reflects bit write {raw:04X}");

        addr.jump(0).unwrap();
        let value = addr
            .scalar_handle(2)
            .expect("handle success")
            .read_bits(4, 8)
            .expect("read back bits");
        assert_eq!(value as u8, 0x5A, "bit range should retain written value");
    }
}
