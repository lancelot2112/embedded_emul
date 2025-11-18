//! Scalar, enumeration, and bitfield helpers derived from the .NET implementation.

use smallvec::SmallVec;

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
    pub byte_size: u32,
    pub encoding: ScalarEncoding,
    pub display: DisplayFormat,
}

impl ScalarType {
    pub fn new(
        name_id: Option<StringId>,
        byte_size: u32,
        encoding: ScalarEncoding,
        display: DisplayFormat,
    ) -> Self {
        Self {
            name_id,
            byte_size,
            encoding,
            display,
        }
    }

    pub fn format_unsigned(&self, value: u64) -> String {
        match self.display {
            DisplayFormat::Hex => format!("0x{value:0width$x}", width = (self.byte_size * 2) as usize),
            DisplayFormat::DotNotation => format_dot_notation(value, self.byte_size),
            _ => value.to_string(),
        }
    }
}

fn format_dot_notation(value: u64, byte_size: u32) -> String {
    let mut parts = Vec::with_capacity(byte_size as usize);
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumVariant {
    pub label: StringId,
    pub value: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumType {
    pub base: ScalarType,
    pub variants: SmallVec<[EnumVariant; 4]>,
}

impl EnumType {
    pub fn new(base: ScalarType) -> Self {
        Self {
            base,
            variants: SmallVec::new(),
        }
    }

    pub fn push_variant(&mut self, variant: EnumVariant) {
        self.variants.push(variant);
    }

    pub fn label_for(&self, value: i64) -> Option<StringId> {
        self.variants
            .iter()
            .find(|entry| entry.value == value)
            .map(|entry| entry.label)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FixedScalar {
    pub base: ScalarType,
    pub scale: f64,
    pub offset: f64,
}

impl FixedScalar {
    pub fn new(base: ScalarType, scale: f64, offset: f64) -> Self {
        Self { base, scale, offset }
    }

    pub fn apply(&self, raw: i64) -> f64 {
        (raw as f64) * self.scale + self.offset
    }
}

#[cfg(test)]
mod tests {
    //! Validates scalar helpers behave deterministically for debugging scenarios.
    use super::*;
    use crate::soc::prog::types::arena::TypeArena;

    #[test]
    fn scalar_formatting_respects_hex_display() {
        // ensures zero-alloc-ish formatting logic emits padded hex strings
        let scalar = ScalarType::new(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Hex);
        let rendered = scalar.format_unsigned(0x34);
        assert_eq!(rendered, "0x00000034", "hex formatting must include byte padding");
    }

    #[test]
    fn enum_lookup_resolves_label() {
        // confirm that label_for performs value-based search
        let mut arena = TypeArena::new();
        let label = arena.intern_string("Ready");
        let base = ScalarType::new(None, 1, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let mut enum_type = EnumType::new(base);
        enum_type.push_variant(EnumVariant { label, value: 1 });
        assert_eq!(enum_type.label_for(1), Some(label), "value lookup should return first matching label");
    }
}
