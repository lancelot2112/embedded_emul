//! Bit-level helpers layered on top of `BusCursor` for odd widths and alignments.

use crate::soc::bus::softmmu::MMUFlags;
use crate::soc::bus::{BusCursor, BusError, BusResult};

const MAX_NATIVE_BYTES: usize = 16;

pub trait BitsCursorExt {
    /// Reads `bit_len` bits starting at `bit_offset` relative to the current cursor address.
    /// The cursor advances by the minimal number of bytes required to service the read.
    fn read_bits(&mut self, bit_offset: u8, bit_len: usize) -> BusResult<u128>;

    /// Writes `bit_len` bits starting at `bit_offset`, preserving all surrounding bits.
    /// The cursor advances by the minimal number of bytes touched by the write.
    fn write_bits(&mut self, bit_offset: u8, bit_len: usize, value: u128) -> BusResult<()>;
}

impl BitsCursorExt for BusCursor {
    fn read_bits(&mut self, bit_offset: u8, bit_len: usize) -> BusResult<u128> {
        if bit_len == 0 {
            return Ok(0);
        }
        if bit_len > MAX_NATIVE_BYTES * 8 {
            return Err(BusError::UnsupportedWidth {
                bytes: bit_len.div_ceil(8),
            });
        }

        let start = self.get_position();
        let total_bits = bit_offset as usize + bit_len;
        let total_bytes = total_bits.div_ceil(8);
        let word_bytes = select_word_bytes(total_bytes)?;
        let flags = self.flags_at(start)?;
        let shift = base_shift(bit_offset, total_bytes, word_bytes, flags);
        let raw = read_chunk(self, start, word_bytes)?;
        let value = (raw >> shift) & bit_mask(bit_len);
        self.goto(start + total_bytes)?;
        Ok(value)
    }

    fn write_bits(&mut self, bit_offset: u8, bit_len: usize, value: u128) -> BusResult<()> {
        if bit_len == 0 {
            return Ok(());
        }
        if bit_len > MAX_NATIVE_BYTES * 8 {
            return Err(BusError::UnsupportedWidth {
                bytes: bit_len.div_ceil(8),
            });
        }

        let start = self.get_position();
        let total_bits = bit_offset as usize + bit_len;
        let total_bytes = total_bits.div_ceil(8);
        let word_bytes = select_word_bytes(total_bytes)?;
        let flags = self.flags_at(start)?;
        let shift = base_shift(bit_offset, total_bytes, word_bytes, flags);
        let mut chunk = read_chunk(self, start, word_bytes)?;
        let value_mask = bit_mask(bit_len);
        let masked_value = value & value_mask;
        let mask = value_mask << shift;
        chunk = (chunk & !mask) | (masked_value << shift);
        write_chunk(self, start, word_bytes, chunk)?;
        self.goto(start + total_bytes)?;
        Ok(())
    }
}

fn select_word_bytes(total_bytes: usize) -> BusResult<usize> {
    let bytes = match total_bytes {
        0 => 0,
        1 => 1,
        2 => 2,
        3 | 4 => 4,
        5..=8 => 8,
        9..=MAX_NATIVE_BYTES => MAX_NATIVE_BYTES,
        _ => return Err(BusError::UnsupportedWidth { bytes: total_bytes }),
    };
    Ok(bytes)
}

fn base_shift(bit_offset: u8, total_bytes: usize, word_bytes: usize, flags: MMUFlags) -> u32 {
    let extra_low_bytes = if flags.contains(MMUFlags::BIGENDIAN) {
        word_bytes.saturating_sub(total_bytes)
    } else {
        0
    };
    (bit_offset as u32) + (extra_low_bytes as u32 * 8)
}

fn bit_mask(bits: usize) -> u128 {
    match bits {
        0 => 0,
        128 => u128::MAX,
        _ => (1u128 << bits) - 1,
    }
}

fn read_chunk(cursor: &mut BusCursor, address: usize, bytes: usize) -> BusResult<u128> {
    let value = match bytes {
        0 => 0,
        1 => cursor.peek_at::<u8>(address)? as u128,
        2 => cursor.peek_at::<u16>(address)? as u128,
        4 => cursor.peek_at::<u32>(address)? as u128,
        8 => cursor.peek_at::<u64>(address)? as u128,
        16 => cursor.peek_at::<u128>(address)?,
        _ => return Err(BusError::UnsupportedWidth { bytes }),
    };
    Ok(value)
}

fn write_chunk(cursor: &mut BusCursor, address: usize, bytes: usize, value: u128) -> BusResult<()> {
    match bytes {
        0 => Ok(()),
        1 => cursor.write_at::<u8>(address, value as u8),
        2 => cursor.write_at::<u16>(address, value as u16),
        4 => cursor.write_at::<u32>(address, value as u32),
        8 => cursor.write_at::<u64>(address, value as u64),
        16 => cursor.write_at::<u128>(address, value),
        _ => Err(BusError::UnsupportedWidth { bytes }),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::soc::bus::{BusCursor, DeviceBus};
    use crate::soc::device::{AccessContext, Device, Endianness as DeviceEndianness, RamMemory};

    fn make_cursor(bytes: &[u8], endian: DeviceEndianness) -> BusCursor {
        let mut bus = DeviceBus::new(32);
        let memory = RamMemory::new("ram", 0x40, endian);
        memory.write(0, bytes, AccessContext::DEBUG).unwrap();
        bus.map_device(memory, 0, 0).unwrap();
        BusCursor::attach_to_bus(Arc::new(bus), 0, AccessContext::CPU)
    }

    #[test]
    fn read_bits_handles_unaligned_little_endian() {
        let data = [0xF0, 0x0F];
        let mut cursor = make_cursor(&data, DeviceEndianness::Little);
        cursor.goto(0).unwrap();
        let value = cursor.read_bits(4, 8).expect("read bits");
        assert_eq!(value, 0xFF, "value should span both source bytes");
        assert_eq!(cursor.get_position(), 2, "cursor advances by touched bytes");
    }

    #[test]
    fn read_bits_respects_big_endian_ordering() {
        let data = [0x12, 0x34, 0x56, 0x78];
        let mut cursor = make_cursor(&data, DeviceEndianness::Big);
        cursor.goto(0).unwrap();
        let value = cursor.read_bits(0, 24).expect("read 24 bits");
        assert_eq!(value, 0x123456, "upper bytes retain big-endian order");
        assert_eq!(
            cursor.get_position(),
            3,
            "cursor advances by requested bytes"
        );
    }

    #[test]
    fn write_bits_updates_partial_region() {
        let data = [0x00, 0x00];
        let mut cursor = make_cursor(&data, DeviceEndianness::Little);
        cursor.goto(0).unwrap();
        cursor
            .write_bits(0, 12, 0x0ABC)
            .expect("write 12-bit value");
        cursor.goto(0).unwrap();
        let value = cursor.read_bits(0, 16).expect("read word");
        assert_eq!(
            value, 0x0ABC,
            "write should preserve high nibble beyond target width"
        );
    }
}
