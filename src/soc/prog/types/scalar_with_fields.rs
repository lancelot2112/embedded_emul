//! Helper types for modelling scalar containers with named bitfield views.

use super::arena::{StringId, TypeArena, TypeId};
use super::arena_record::{ArenaSpan, FieldRecord, TypeRecord};
use super::bitfield::{BitFieldSpec, BitFieldSpecBuilder};
use super::builder::TypeBuilder;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalarWithFieldsRecord {
    pub storage: TypeId,
    pub fields: ArenaSpan,
}

pub struct ScalarWithFieldsBuilder {
    storage: TypeId,
    storage_bits: u16,
    fields: Vec<ScalarFieldInit>,
}

impl ScalarWithFieldsBuilder {
    /// Creates a new builder for the provided scalar storage type.
    pub fn new(storage: TypeId, storage_bits: u16) -> Self {
        Self {
            storage,
            storage_bits,
            fields: Vec::new(),
        }
    }

    /// Convenience constructor that infers the storage width from the arena record.
    pub fn from_scalar(arena: &TypeArena, storage: TypeId) -> Self {
        let storage_bits = match arena.get(storage) {
            TypeRecord::Scalar(scalar) => scalar.bit_size,
            other => panic!("scalar_with_fields storage must be Scalar, got {:?}", other),
        };
        Self::new(storage, storage_bits)
    }

    /// Adds a named range using the underlying BitFieldSpecBuilder for mask generation.
    pub fn push_range<N>(&mut self, name: N, offset_bits: u16, bit_size: u16) -> &mut Self
    where
        N: Into<FieldName>,
    {
        let limit = u32::from(offset_bits) + u32::from(bit_size);
        assert!(
            limit <= u32::from(self.storage_bits),
            "bitfield exceeds storage width"
        );
        self.push_spec_builder(name, |builder| builder.range(offset_bits, bit_size))
    }

    /// Adds a field by providing a closure that configures the BitFieldSpecBuilder.
    pub fn push_spec_builder<N, F>(&mut self, name: N, build: F) -> &mut Self
    where
        N: Into<FieldName>,
        F: FnOnce(BitFieldSpecBuilder) -> BitFieldSpecBuilder,
    {
        let builder = build(BitFieldSpec::builder(self.storage_bits));
        self.fields.push(ScalarFieldInit {
            name: name.into(),
            kind: FieldKind::Builder(builder),
        });
        self
    }

    /// Adds a field using a fully constructed BitFieldSpec.
    pub fn push_spec<N>(&mut self, name: N, spec: BitFieldSpec) -> &mut Self
    where
        N: Into<FieldName>,
    {
        assert_eq!(
            spec.storage_bits(),
            self.storage_bits,
            "bitfield spec must match storage width"
        );
        self.fields.push(ScalarFieldInit {
            name: name.into(),
            kind: FieldKind::Spec(spec),
        });
        self
    }

    /// Adds a field that reuses an existing type identifier (e.g. pre-built bitfield).
    pub fn push_type<N>(&mut self, name: N, ty: TypeId) -> &mut Self
    where
        N: Into<FieldName>,
    {
        self.fields.push(ScalarFieldInit {
            name: name.into(),
            kind: FieldKind::Prebuilt(ty),
        });
        self
    }

    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    pub fn build(self, arena: &mut TypeArena) -> TypeId {
        let mut records = Vec::with_capacity(self.fields.len());
        for field in self.fields {
            let (name_id, ty) = field.finalize(arena, self.storage_bits);
            records.push(FieldRecord::new(name_id, ty));
        }
        let span = arena.alloc_fields(records);
        let record = ScalarWithFieldsRecord {
            storage: self.storage,
            fields: span,
        };
        arena.push_record(TypeRecord::ScalarWithFields(record))
    }

    pub fn finish_with(self, builder: &mut TypeBuilder<'_>) -> TypeId {
        self.build(builder.arena)
    }
}

struct ScalarFieldInit {
    name: FieldName,
    kind: FieldKind,
}

impl ScalarFieldInit {
    fn finalize(self, arena: &mut TypeArena, storage_bits: u16) -> (StringId, TypeId) {
        let name_id = self.name.into_id(arena);
        let ty = match self.kind {
            FieldKind::Builder(builder) => {
                let spec = builder.finish();
                debug_assert_eq!(
                    spec.storage_bits(),
                    storage_bits,
                    "builder storage width changed during construction"
                );
                arena.push_record(TypeRecord::BitField(spec))
            }
            FieldKind::Spec(spec) => arena.push_record(TypeRecord::BitField(spec)),
            FieldKind::Prebuilt(ty) => ty,
        };
        (name_id, ty)
    }
}

enum FieldKind {
    Builder(BitFieldSpecBuilder),
    Spec(BitFieldSpec),
    Prebuilt(TypeId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldName {
    Interned(StringId),
    Owned(String),
}

impl FieldName {
    fn into_id(self, arena: &mut TypeArena) -> StringId {
        match self {
            FieldName::Interned(id) => id,
            FieldName::Owned(name) => arena.intern_string(name),
        }
    }
}

impl From<StringId> for FieldName {
    fn from(value: StringId) -> Self {
        FieldName::Interned(value)
    }
}

impl<S> From<S> for FieldName
where
    S: Into<String>,
{
    fn from(value: S) -> Self {
        FieldName::Owned(value.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::prog::types::scalar::{DisplayFormat, ScalarEncoding, ScalarType};

    #[test]
    fn plan_builds_bitfields_and_record() {
        let mut arena = TypeArena::new();
        let storage = ScalarType::new(None, 1, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let storage_id = arena.push_record(TypeRecord::Scalar(storage));
        let storage_bits = match arena.get(storage_id) {
            TypeRecord::Scalar(scalar) => scalar.bit_size,
            _ => unreachable!(),
        };
        let mut plan = ScalarWithFieldsBuilder::new(storage_id, storage_bits);
        plan.push_range("flags", 0, 3);
        let ty = plan.build(&mut arena);
        match arena.get(ty) {
            TypeRecord::ScalarWithFields(record) => {
                let fields = arena.fields(record.fields);
                assert_eq!(fields.len(), 1, "plan should allocate one field record");
                assert_eq!(arena.resolve_string(fields[0].name_id), "flags");
            }
            other => panic!("expected ScalarWithFields record, got {:?}", other),
        }
    }

    #[test]
    fn push_spec_builder_exposes_full_builder() {
        let mut arena = TypeArena::new();
        let storage = ScalarType::new(None, 2, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let storage_id = arena.push_record(TypeRecord::Scalar(storage));
        let mut plan = ScalarWithFieldsBuilder::from_scalar(&arena, storage_id);
        plan.push_spec_builder("complex", |builder| builder.range(0, 3).literal(0b10, 2));
        let ty = plan.build(&mut arena);
        let TypeRecord::ScalarWithFields(record) = arena.get(ty) else {
            panic!("expected scalar with fields record");
        };
        let fields = arena.fields(record.fields);
        assert_eq!(fields.len(), 1, "builder should produce one field");
        let TypeRecord::BitField(spec) = arena.get(fields[0].ty) else {
            panic!("field should materialise as bitfield spec");
        };
        assert_eq!(spec.total_width(), 5, "builder should honour literal width");
    }
}
