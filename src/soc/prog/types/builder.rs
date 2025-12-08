//! Light-weight construction helpers that bridge debugger metadata into the arena and expose a fluent API for manual builders.

use crate::soc::prog::types::ScalarWithFieldsBuilder;

use super::arena::{StringId, TypeArena, TypeId};
use super::arena_record::TypeRecord;
use super::bitfield::BitFieldSpec;
use super::pointer::{PointerKind, PointerType};
use super::scalar::{DisplayFormat, ScalarEncoding, ScalarType};
use super::sequence::{SequenceCount, SequenceType};

pub struct TypeBuilder<'arena> {
    pub(super) arena: &'arena mut TypeArena,
}

impl<'arena> TypeBuilder<'arena> {
    pub fn new(arena: &'arena mut TypeArena) -> Self {
        Self { arena }
    }

    pub fn intern<S: AsRef<str>>(&mut self, name: S) -> StringId {
        self.arena.intern_string(name)
    }

    pub fn declare_scalar(
        &mut self,
        name: Option<StringId>,
        byte_size: usize,
        encoding: ScalarEncoding,
        display: DisplayFormat,
    ) -> TypeId {
        let scalar = ScalarType::new(name, byte_size, encoding, display);
        self.arena.push_record(TypeRecord::Scalar(scalar))
    }

    pub fn scalar(
        &mut self,
        name: Option<&str>,
        byte_size: usize,
        encoding: ScalarEncoding,
        display: DisplayFormat,
    ) -> TypeId {
        let name_id = name.map(|value| self.intern(value));
        self.declare_scalar(name_id, byte_size, encoding, display)
    }

    pub fn scalar_with_fields(&mut self, byte_size: usize) -> ScalarWithFieldsBuilder {
        let storage = self.scalar(
            None,
            byte_size,
            ScalarEncoding::Unsigned,
            DisplayFormat::Default,
        );
        let bit_size = u16::try_from(byte_size * 8)
            .expect("scalar_with_fields storage exceeds supported width");
        ScalarWithFieldsBuilder::new(storage, bit_size)
    }

    pub fn pointer(&mut self, target: TypeId, kind: PointerKind, byte_size: usize) -> TypeId {
        let pointer = PointerType::new(target, kind).with_byte_size(byte_size);
        self.arena.push_record(TypeRecord::Pointer(pointer))
    }

    pub fn sequence(
        &mut self,
        element: TypeId,
        stride_bytes: usize,
        count: SequenceCount,
    ) -> TypeId {
        let sequence = SequenceType::new(element, stride_bytes, count);
        self.arena.push_record(TypeRecord::Sequence(sequence))
    }

    pub fn sequence_static(
        &mut self,
        element: TypeId,
        stride_bytes: usize,
        count: usize,
    ) -> TypeId {
        self.sequence(element, stride_bytes, SequenceCount::Static(count))
    }

    pub fn bitfield(&mut self, spec: BitFieldSpec) -> TypeId {
        self.arena.push_record(TypeRecord::BitField(spec))
    }
}

pub trait DebugTypeProvider {
    fn resolve_type(&mut self, handle: RawTypeDesc, builder: &mut TypeBuilder<'_>) -> TypeId;
}

#[derive(Clone, Debug)]
pub enum RawTypeDesc {
    Scalar {
        name: Option<String>,
        byte_size: u32,
        encoding: ScalarEncoding,
        display: DisplayFormat,
    },
}

#[cfg(test)]
mod tests {
    //! Builder smoke tests to keep ingestion layers honest.
    use super::*;

    #[test]
    fn declare_scalar_returns_valid_id() {
        // ensures builder forwards declarations into the shared arena
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let name = builder.intern("pc_t");
        let id =
            builder.declare_scalar(Some(name), 8, ScalarEncoding::Unsigned, DisplayFormat::Hex);
        assert_eq!(
            arena.get(id).as_scalar().unwrap().byte_size,
            8,
            "scalar should honor requested byte size"
        );
    }

    #[test]
    fn sequence_builder_handles_static_count() {
        // sequence builder should store stride and static element counts verbatim
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let word = builder.scalar(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let seq_id = builder.sequence_static(word, 4, 8);

        let TypeRecord::Sequence(seq) = arena.get(seq_id) else {
            panic!("expected sequence type");
        };
        assert_eq!(
            seq.stride_bytes, 4,
            "stride bytes should match constructor argument"
        );
        assert_eq!(
            seq.element_count(),
            Some(8),
            "static sequence count should be accessible"
        );
    }
}
