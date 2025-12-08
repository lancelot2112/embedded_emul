use smallvec::SmallVec;

use crate::soc::prog::types::{ScalarType, StringId, TypeBuilder, TypeId, TypeRecord};

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

pub struct EnumBuilder<'builder, 'arena> {
    builder: &'builder mut TypeBuilder<'arena>,
    ty: EnumType,
}

impl<'builder, 'arena> EnumBuilder<'builder, 'arena> {
    pub(super) fn new(builder: &'builder mut TypeBuilder<'arena>, base: ScalarType) -> Self {
        Self {
            builder,
            ty: EnumType::new(base),
        }
    }

    pub fn variant(mut self, label: impl AsRef<str>, value: i64) -> Self {
        let label_id = self.builder.intern(label);
        self.ty.push_variant(EnumVariant {
            label: label_id,
            value,
        });
        self
    }

    pub fn finish(self) -> TypeId {
        self.builder.arena.push_record(TypeRecord::Enum(self.ty))
    }
}

impl<'arena> TypeBuilder<'arena> {
    pub fn enumeration(&mut self, base: ScalarType) -> EnumBuilder<'_, 'arena> {
        EnumBuilder::new(self, base)
    }
}

#[cfg(test)]
mod tests {
    use crate::soc::prog::types::{DisplayFormat, ScalarEncoding, TypeArena};

    use super::*;
    #[test]
    fn enum_lookup_resolves_label() {
        // confirm that label_for performs value-based search
        let mut arena = TypeArena::new();
        let label = arena.intern_string("Ready");
        let base = ScalarType::new(None, 1, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let mut enum_type = EnumType::new(base);
        enum_type.push_variant(EnumVariant { label, value: 1 });
        assert_eq!(
            enum_type.label_for(1),
            Some(label),
            "value lookup should return first matching label"
        );
    }

    #[test]
    fn enum_builder_collects_variants() {
        // enum builder should collect label/value pairs fluently
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let base = ScalarType::new(None, 1, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let enum_id = builder
            .enumeration(base)
            .variant("Ready", 1)
            .variant("Busy", 2)
            .finish();

        let TypeRecord::Enum(enum_ty) = arena.get(enum_id) else {
            panic!("expected enum type");
        };
        assert_eq!(
            enum_ty.variants.len(),
            2,
            "enum builder should store all variants"
        );
    }
}
