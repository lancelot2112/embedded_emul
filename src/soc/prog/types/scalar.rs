//! Scalar, enumeration, and bitfield helpers derived from the .NET implementation.

use crate::soc::{
    bus::{BusCursor, BusResult},
    prog::types::TypeRecord,
};

use super::arena::StringId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarEncoding {
    Unsigned,
    Signed,
    Floating,
    Utf8String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayFormat {
    Default,
    Decimal,
    Hex,
    DotNotation,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScalarType {
    pub name_id: Option<StringId>,
    pub bit_size: u16,
    pub byte_size: usize,
    pub encoding: ScalarEncoding,
    pub display: DisplayFormat,
    storage: ScalarStorage,
}

impl ScalarType {
    pub fn new(
        name_id: Option<StringId>,
        byte_size: usize,
        encoding: ScalarEncoding,
        display: DisplayFormat,
    ) -> Self {
        let storage = ScalarStorage::for_bytes(byte_size);
        Self {
            name_id,
            bit_size: (byte_size as u16) * 8,
            byte_size,
            encoding,
            display,
            storage,
        }
    }

    pub fn with_bits(
        name_id: Option<StringId>,
        bit_size: u16,
        encoding: ScalarEncoding,
        display: DisplayFormat,
    ) -> Self {
        let byte_size = ((bit_size as usize) + 7) / 8;
        let storage = ScalarStorage::for_bytes(byte_size);
        Self {
            name_id,
            bit_size,
            byte_size,
            encoding,
            display,
            storage,
        }
    }

    pub fn is_signed(&self) -> bool {
        matches!(self.encoding, ScalarEncoding::Signed)
    }

    pub fn format_unsigned(&self, value: u64) -> String {
        match self.display {
            DisplayFormat::Hex => {
                format!("0x{value:0width$x}", width = (self.byte_size * 2) as usize)
            }
            DisplayFormat::DotNotation => format_dot_notation(value, self.byte_size),
            _ => value.to_string(),
        }
    }

    pub fn read_unsigned(&self, cursor: &mut BusCursor) -> BusResult<u128> {
        let raw = self.storage.read(cursor)?;
        Ok(raw & self.value_mask())
    }

    pub fn read_signed(&self, cursor: &mut BusCursor) -> BusResult<i128> {
        let unsigned = self.read_unsigned(cursor)?;
        if self.bit_size == 0 {
            return Ok(0);
        }
        let shift = 128 - self.bit_size as u32;
        Ok(((unsigned << shift) as i128) >> shift)
    }

    pub fn write_unsigned(&self, cursor: &mut BusCursor, value: u128) -> BusResult<()> {
        let masked = value & self.value_mask();
        self.storage.write(cursor, masked)
    }

    pub fn write_signed(&self, cursor: &mut BusCursor, value: i128) -> BusResult<()> {
        let masked = if self.bit_size == 0 {
            0
        } else {
            let shift = 128 - self.bit_size as u32;
            ((value << shift) as u128) >> shift
        };
        self.storage.write(cursor, masked)
    }

    fn value_mask(&self) -> u128 {
        match self.bit_size {
            0 => 0,
            128 => u128::MAX,
            bits => (1u128 << bits) - 1,
        }
    }
}

impl From<ScalarType> for TypeRecord {
    fn from(value: ScalarType) -> Self {
        TypeRecord::Scalar(value)
    }
}

fn format_dot_notation(value: u64, byte_size: usize) -> String {
    let mut parts = Vec::with_capacity(byte_size);
    for idx in (0..byte_size).rev() {
        let shift = idx * 8;
        parts.push(((value >> shift) & 0xFF) as u8);
    }
    parts
        .into_iter()
        .map(|byte| byte.to_string())
        .collect::<Vec<_>>()
        .join(".")
}

#[derive(Clone, Debug, PartialEq)]
pub struct FixedScalar {
    pub base: ScalarType,
    pub scale: f64,
    pub offset: f64,
}

impl FixedScalar {
    pub fn new(base: ScalarType, scale: f64, offset: f64) -> Self {
        Self {
            base,
            scale,
            offset,
        }
    }

    pub fn apply(&self, raw: i64) -> f64 {
        (raw as f64) * self.scale + self.offset
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScalarStorage {
    Zero,
    U8,
    U16,
    U32,
    U64,
    U128,
}

impl ScalarStorage {
    pub(crate) fn for_bytes(bytes: usize) -> Self {
        match bytes {
            0 => ScalarStorage::Zero,
            1 => ScalarStorage::U8,
            2 => ScalarStorage::U16,
            4 => ScalarStorage::U32,
            8 => ScalarStorage::U64,
            16 => ScalarStorage::U128,
            other => panic!("unsupported scalar width: {other} bytes"),
        }
    }

    pub(crate) fn for_bits(bits: usize) -> Self {
        let bytes = (bits + 7) / 8;
        Self::for_bytes(bytes)
    }

    pub(crate) fn bit_size(self) -> u16 {
        match self {
            ScalarStorage::Zero => 0,
            ScalarStorage::U8 => 8,
            ScalarStorage::U16 => 16,
            ScalarStorage::U32 => 32,
            ScalarStorage::U64 => 64,
            ScalarStorage::U128 => 128,
        }
    }

    pub(crate) fn byte_size(self) -> usize {
        match self {
            ScalarStorage::Zero => 0,
            ScalarStorage::U8 => 1,
            ScalarStorage::U16 => 2,
            ScalarStorage::U32 => 4,
            ScalarStorage::U64 => 8,
            ScalarStorage::U128 => 16,
        }
    }

    pub(crate) fn read(self, cursor: &mut BusCursor) -> BusResult<u128> {
        match self {
            ScalarStorage::Zero => Ok(0),
            ScalarStorage::U8 => cursor.read_u8().map(|v| v as u128),
            ScalarStorage::U16 => cursor.read_u16().map(|v| v as u128),
            ScalarStorage::U32 => cursor.read_u32().map(|v| v as u128),
            ScalarStorage::U64 => cursor.read_u64().map(|v| v as u128),
            ScalarStorage::U128 => cursor.read::<u128>().map(|v| v as u128),
        }
    }

    pub(crate) fn write(self, cursor: &mut BusCursor, value: u128) -> BusResult<()> {
        match self {
            ScalarStorage::Zero => Ok(()),
            ScalarStorage::U8 => cursor.write_u8(value as u8),
            ScalarStorage::U16 => cursor.write_u16(value as u16),
            ScalarStorage::U32 => cursor.write_u32(value as u32),
            ScalarStorage::U64 => cursor.write_u64(value as u64),
            ScalarStorage::U128 => cursor.write::<u128>(value),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Validates scalar helpers behave deterministically for debugging scenarios.
    use super::*;

    #[test]
    fn scalar_formatting_respects_hex_display() {
        // ensures zero-alloc-ish formatting logic emits padded hex strings
        let scalar = ScalarType::new(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Hex);
        let rendered = scalar.format_unsigned(0x34);
        assert_eq!(
            rendered, "0x00000034",
            "hex formatting must include byte padding"
        );
    }
}
