//! Shared utilities for decoding symbol-backed type records into high-level values.

use crate::soc::prog::symbols::walker::SymbolWalkEntry;
use crate::soc::prog::types::arena::TypeArena;
use crate::soc::prog::types::bitfield::{BitFieldSegment, BitFieldSpec, PadKind};
use crate::soc::prog::types::pointer::PointerType;
use crate::soc::prog::types::record::TypeRecord;
use crate::soc::prog::types::scalar::{EnumType, FixedScalar, ScalarEncoding, ScalarType};
use crate::soc::system::bus::ext::{ArbSizeDataHandleExt, FloatDataHandleExt, StringDataHandleExt};
use crate::soc::system::bus::DataHandle;

use super::value::{SymbolAccessError, SymbolValue};

pub struct ReadContext<'ctx, 'arena> {
    pub data: &'ctx mut DataHandle,
    pub arena: &'arena TypeArena,
    pub entry: Option<&'ctx SymbolWalkEntry>,
    pub field_address: u64,
    pub symbol_base: u64,
    pub size_hint: Option<u32>,
}

impl<'ctx, 'arena> ReadContext<'ctx, 'arena> {
    pub fn new(
        data: &'ctx mut DataHandle,
        arena: &'arena TypeArena,
        entry: Option<&'ctx SymbolWalkEntry>,
        field_address: u64,
        symbol_base: u64,
        size_hint: Option<u32>,
    ) -> Self {
        Self {
            data,
            arena,
            entry,
            field_address,
            symbol_base,
            size_hint,
        }
    }
}

pub trait SymbolReadable {
    fn read_symbol_value(
        &self,
        ctx: &mut ReadContext<'_, '_>,
    ) -> Result<Option<SymbolValue>, SymbolAccessError>;
}

impl SymbolReadable for ScalarType {
    fn read_symbol_value(
        &self,
        ctx: &mut ReadContext<'_, '_>,
    ) -> Result<Option<SymbolValue>, SymbolAccessError> {
        ctx.data.address_mut().jump(ctx.field_address)?;
        let width = self.byte_size as usize;
        let value = match self.encoding {
            ScalarEncoding::Unsigned => {
                let value = if width == 0 { 0 } else { ctx.data.read_unsigned(width)? };
                Some(SymbolValue::Unsigned(value))
            }
            ScalarEncoding::Signed => {
                let value = if width == 0 { 0 } else { ctx.data.read_signed(width)? };
                Some(SymbolValue::Signed(value))
            }
            ScalarEncoding::Floating => match width {
                4 => {
                    let value = ctx.data.read_f32()?;
                    Some(SymbolValue::Float(value as f64))
                }
                8 => {
                    let value = ctx.data.read_f64()?;
                    Some(SymbolValue::Float(value))
                }
                _ => None,
            },
            ScalarEncoding::Utf8String => {
                if width == 0 {
                    return Ok(Some(SymbolValue::Utf8(String::new())));
                }
                let value = ctx.data.read_utf8(width)?;
                Some(SymbolValue::Utf8(value))
            }
        };
        Ok(value)
    }
}

impl SymbolReadable for EnumType {
    fn read_symbol_value(
        &self,
        ctx: &mut ReadContext<'_, '_>,
    ) -> Result<Option<SymbolValue>, SymbolAccessError> {
        ctx.data.address_mut().jump(ctx.field_address)?;
        let width = self.base.byte_size as usize;
        let value = if width == 0 { 0 } else { ctx.data.read_signed(width)? };
        let label = self
            .label_for(value)
            .map(|id| ctx.arena.resolve_string(id).to_string());
        Ok(Some(SymbolValue::Enum { label, value }))
    }
}

impl SymbolReadable for FixedScalar {
    fn read_symbol_value(
        &self,
        ctx: &mut ReadContext<'_, '_>,
    ) -> Result<Option<SymbolValue>, SymbolAccessError> {
        ctx.data.address_mut().jump(ctx.field_address)?;
        let width = self.base.byte_size as usize;
        if width == 0 {
            return Ok(Some(SymbolValue::Float(self.apply(0))));
        }
        let raw = ctx.data.read_signed(width)?;
        Ok(Some(SymbolValue::Float(self.apply(raw))))
    }
}

impl SymbolReadable for PointerType {
    fn read_symbol_value(
        &self,
        ctx: &mut ReadContext<'_, '_>,
    ) -> Result<Option<SymbolValue>, SymbolAccessError> {
        ctx.data.address_mut().jump(ctx.field_address)?;
        let width = self
            .byte_size
            .max(ctx.size_hint.unwrap_or(self.byte_size))
            as usize;
        if width > 8 {
            return Ok(None);
        }
        let value = if width == 0 {
            0
        } else {
            ctx.data.read_unsigned(width)?
        };
        Ok(Some(SymbolValue::Unsigned(value)))
    }
}

impl SymbolReadable for BitFieldSpec {
    fn read_symbol_value(
        &self,
        ctx: &mut ReadContext<'_, '_>,
    ) -> Result<Option<SymbolValue>, SymbolAccessError> {
        let entry = ctx.entry.ok_or_else(|| SymbolAccessError::UnsupportedTraversal {
            label: "bitfield requires symbol walk entry".into(),
        })?;
        let width = self.total_width() as u32;
        if width == 0 {
            return Ok(Some(SymbolValue::Unsigned(0)));
        }
        if width > 64 {
            return Err(SymbolAccessError::UnsupportedTraversal {
                label: "bitfield wider than 64 bits".into(),
            });
        }
        let mut aligned_bit_base = 0u64;
        let mut backing = 0u128;
        let mut has_range = false;
        let mut min_bit = u64::MAX;
        let mut max_bit = 0u64;
        for segment in &self.segments {
            if let BitFieldSegment::Range { offset, width } = segment {
                if *width == 0 {
                    continue;
                }
                has_range = true;
                let start = entry.offset_bits + (*offset as u64);
                let end = start + (*width as u64);
                min_bit = min_bit.min(start);
                max_bit = max_bit.max(end);
            }
        }
        if has_range {
            let aligned_address_bit = min_bit & !7;
            let byte_address = ctx.symbol_base + (aligned_address_bit / 8);
            let bit_span = max_bit.saturating_sub(aligned_address_bit);
            let byte_span = ((bit_span + 7) / 8) as usize;
            let mut buf = vec![0u8; byte_span];
            ctx.data.address_mut().jump(byte_address)?;
            if !buf.is_empty() {
                ctx.data.read_bytes(&mut buf)?;
            }
            for (idx, byte) in buf.iter().enumerate() {
                backing |= (*byte as u128) << (idx * 8);
            }
            aligned_bit_base = aligned_address_bit;
        }
        let mut acc: u128 = 0;
        let mut acc_width: u32 = 0;
        for segment in &self.segments {
            match segment {
                BitFieldSegment::Range { offset, width } => {
                    if *width == 0 {
                        continue;
                    }
                    let abs_start = entry.offset_bits + (*offset as u64);
                    let shift = abs_start
                        .checked_sub(aligned_bit_base)
                        .unwrap_or(0) as u32;
                    let mask = if *width as u32 == 64 {
                        u128::MAX
                    } else {
                        (1u128 << (*width as u32)) - 1
                    };
                    let raw = (backing >> shift) & mask;
                    acc |= raw << acc_width;
                    acc_width += *width as u32;
                }
                BitFieldSegment::Literal { value, width } => {
                    if *width == 0 {
                        continue;
                    }
                    let width_u32 = *width as u32;
                    let mask = if width_u32 == 64 {
                        u64::MAX
                    } else {
                        (1u64 << width_u32) - 1
                    };
                    let raw = (*value) & mask;
                    acc |= (raw as u128) << acc_width;
                    acc_width += width_u32;
                }
            }
        }
        let mut result_width = acc_width;
        if let Some(pad) = self.pad {
            let pad_width = pad.width as u32;
            if pad_width > 0 {
                if matches!(pad.kind, PadKind::Sign) && acc_width > 0 {
                    let sign_bit = ((acc >> (acc_width - 1)) & 1) != 0;
                    if sign_bit {
                        let mask = ((1u128 << pad_width) - 1) << acc_width;
                        acc |= mask;
                    }
                }
                result_width += pad_width;
            }
        }
        let total_width = result_width;
        debug_assert_eq!(u32::from(self.total_width()), total_width);
        let value_u64 = acc as u64;
        let value = if self.is_signed() {
            let shift = 64 - total_width;
            let signed = ((value_u64 << shift) as i64) >> shift;
            SymbolValue::Signed(signed)
        } else {
            SymbolValue::Unsigned(value_u64)
        };
        Ok(Some(value))
    }
}

pub fn read_type_record(
    record: &TypeRecord,
    ctx: &mut ReadContext<'_, '_>,
) -> Result<Option<SymbolValue>, SymbolAccessError> {
    match record {
        TypeRecord::Scalar(scalar) => scalar.read_symbol_value(ctx),
        TypeRecord::Enum(enum_type) => enum_type.read_symbol_value(ctx),
        TypeRecord::Fixed(fixed) => fixed.read_symbol_value(ctx),
        TypeRecord::Pointer(pointer) => pointer.read_symbol_value(ctx),
        TypeRecord::BitField(bitfield) => bitfield.read_symbol_value(ctx),
        _ => Ok(None),
    }
}