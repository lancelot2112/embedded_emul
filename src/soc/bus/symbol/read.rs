//! Shared utilities for decoding symbol-backed type records into high-level values.

use crate::soc::prog::symbols::walker::SymbolWalkEntry;
use crate::soc::prog::types::arena::TypeArena;
use crate::soc::prog::types::bitfield::BitFieldSpec;
use crate::soc::prog::types::pointer::PointerType;
use crate::soc::prog::types::record::TypeRecord;
use crate::soc::prog::types::scalar::{EnumType, FixedScalar, ScalarEncoding, ScalarType};
use crate::soc::bus::DataHandle;
use crate::soc::bus::ext::{FloatDataHandleExt, IntDataHandleExt, StringDataHandleExt};

use super::value::{SymbolAccessError, SymbolValue};

pub struct ReadContext<'ctx, 'arena> {
    pub data: &'ctx mut DataHandle,
    pub arena: &'arena TypeArena,
    pub entry: Option<&'ctx SymbolWalkEntry>,
    pub field_address: usize,
    pub symbol_base: usize,
    pub size_hint: Option<usize>,
}

impl<'ctx, 'arena> ReadContext<'ctx, 'arena> {
    pub fn new(
        data: &'ctx mut DataHandle,
        arena: &'arena TypeArena,
        entry: Option<&'ctx SymbolWalkEntry>,
        field_address: usize,
        symbol_base: usize,
        size_hint: Option<usize>,
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
                let value = if width == 0 {
                    0
                } else {
                    ctx.data.read_unsigned(0, width * 8)?
                };
                Some(SymbolValue::Unsigned(value))
            }
            ScalarEncoding::Signed => {
                let value = if width == 0 {
                    0
                } else {
                    ctx.data.read_signed(0, width * 8)?
                };
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
        let value = if width == 0 {
            0
        } else {
            ctx.data.read_signed(0, width * 8)?
        };
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
        let raw = ctx.data.read_signed(0, width * 8)?;
        Ok(Some(SymbolValue::Float(self.apply(raw))))
    }
}

impl SymbolReadable for PointerType {
    fn read_symbol_value(
        &self,
        ctx: &mut ReadContext<'_, '_>,
    ) -> Result<Option<SymbolValue>, SymbolAccessError> {
        ctx.data.address_mut().jump(ctx.field_address)?;
        let width = self.byte_size.max(ctx.size_hint.unwrap_or(self.byte_size)) as usize;
        if width > 8 {
            return Ok(None);
        }
        let value = if width == 0 {
            0
        } else {
            ctx.data.read_unsigned(0, width * 8)?
        };
        Ok(Some(SymbolValue::Unsigned(value)))
    }
}

impl SymbolReadable for BitFieldSpec {
    fn read_symbol_value(
        &self,
        ctx: &mut ReadContext<'_, '_>,
    ) -> Result<Option<SymbolValue>, SymbolAccessError> {
        let entry = ctx
            .entry
            .ok_or_else(|| SymbolAccessError::UnsupportedTraversal {
                label: "bitfield requires symbol walk entry".into(),
            })?;
        let width = self.total_width();
        if width == 0 {
            return Ok(Some(SymbolValue::Unsigned(0)));
        }
        if width > 64 {
            return Err(SymbolAccessError::UnsupportedTraversal {
                label: "bitfield wider than 64 bits".into(),
            });
        }
        let mut container_bits = 0u64;
        if let Some((_, max_bit)) = self.bit_span() {
            let entry_bit_base = entry.offset_bits;
            let aligned_bit_base = entry_bit_base & !7;
            let bit_offset = (entry_bit_base - aligned_bit_base) as u8;
            let byte_address = ctx.symbol_base + (aligned_bit_base / 8);
            ctx.data.address_mut().jump(byte_address)?;
            let bits = ctx.data.read_unsigned(bit_offset, max_bit as usize)?;
            container_bits = bits;
        }
        let (raw_value, actual_width) = self.read_bits(container_bits);
        debug_assert_eq!(self.total_width(), actual_width);
        let value = if self.is_signed() {
            let shift = 64 - actual_width;
            let signed = ((raw_value << shift) as i64) >> shift;
            SymbolValue::Signed(signed)
        } else {
            SymbolValue::Unsigned(raw_value)
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
