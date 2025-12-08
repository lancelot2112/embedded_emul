//! Defines the canonical record structures stored inside the type arena.
use super::aggregate::AggregateType;
use super::arena::{StringId, TypeId};
use super::bitfield::BitFieldSpec;
use super::callable::CallableType;
use super::dynamic::DynamicAggregate;
use super::enum_scalar::EnumType;
use super::pointer::PointerType;
use super::scalar::{FixedScalar, ScalarType};
use super::scalar_with_fields::ScalarWithFieldsRecord;
use super::sequence::SequenceType;

/// Compact representation of the byte size and trailing bit padding of a layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutSize {
    pub bytes: usize,
    pub trailing_bits: usize,
}

impl LayoutSize {
    pub const ZERO: Self = Self {
        bytes: 0,
        trailing_bits: 0,
    };

    pub fn total_bits(self) -> usize {
        (self.bytes << 3) + self.trailing_bits
    }
}

/// Describes a contiguous slice of members or fields stored inside the arena side table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArenaSpan {
    start: u32,
    len: u32,
}

impl ArenaSpan {
    pub fn empty() -> Self {
        Self { start: 0, len: 0 }
    }

    pub fn new(start: usize, len: usize) -> Self {
        Self {
            start: start as u32,
            len: len as u32,
        }
    }

    pub fn start(&self) -> usize {
        self.start as usize
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// POD metadata for a single aggregate member.
/// Stored in TypeArena member pool to avoid type data growing onto the heap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberRecord {
    pub name_id: Option<StringId>,
    pub ty: TypeId,
    pub offset_bits: usize, // from start of parent aggregate
    pub bit_size: Option<usize>,
    pub fields: Option<ArenaSpan>,
}

impl MemberRecord {
    pub fn new(name_id: Option<StringId>, ty: TypeId, offset_bits: usize) -> Self {
        Self {
            name_id,
            ty,
            offset_bits,
            bit_size: None,
            fields: None,
        }
    }

    pub fn with_fields(mut self, span: ArenaSpan) -> Self {
        self.fields = Some(span);
        self
    }

    pub fn set_fields(&mut self, span: ArenaSpan) {
        self.fields = Some(span);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldRecord {
    pub name_id: StringId,
    pub ty: TypeId,
}

impl FieldRecord {
    pub fn new(name_id: StringId, ty: TypeId) -> Self {
        Self { name_id, ty }
    }
}

/// Fallback for debugger entries we cannot yet model precisely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpaqueType {
    pub name_id: Option<StringId>,
    pub byte_size: usize,
}

/// All supported type shapes.
#[derive(Clone, Debug, PartialEq)]
pub enum TypeRecord {
    Scalar(ScalarType),
    Enum(EnumType),
    BitField(BitFieldSpec),
    Fixed(FixedScalar),
    Sequence(SequenceType),
    Pointer(PointerType),
    Aggregate(AggregateType),
    Callable(CallableType),
    Dynamic(DynamicAggregate),
    Opaque(OpaqueType),
    ScalarWithFields(ScalarWithFieldsRecord),
}

impl TypeRecord {
    pub fn as_scalar(&self) -> Option<&ScalarType> {
        match self {
            TypeRecord::Scalar(value) => Some(value),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests for record bookkeeping utilities used across the arena.
    use super::*;
    use crate::soc::prog::types::arena::{TypeArena, TypeId};
    use crate::soc::prog::types::scalar::{DisplayFormat, ScalarEncoding, ScalarType};

    fn dummy_scalar(arena: &mut TypeArena) -> TypeId {
        let scalar = ScalarType::new(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Default);
        arena.push_record(TypeRecord::Scalar(scalar))
    }

    #[test]
    fn span_construction_tracks_length() {
        // ensure MemberSpan::new stores the requested bounds verbatim
        let span = ArenaSpan::new(4, 2);
        assert_eq!(
            span.start(),
            4,
            "start index should match constructor argument"
        );
        assert_eq!(span.len(), 2, "length should match constructor argument");
    }

    #[test]
    fn member_record_attaches_field_spans() {
        // verify members can attach field spans allocated in the arena
        let mut arena = TypeArena::new();
        let scalar_id = dummy_scalar(&mut arena);
        let bit_spec = BitFieldSpec::from_range(32, 0, 3);
        let bitfield_id = arena.push_record(TypeRecord::BitField(bit_spec));
        let name_id = arena.intern_string("field");
        let span = arena.alloc_fields([FieldRecord::new(name_id, bitfield_id)]);
        let record = MemberRecord::new(None, scalar_id, 0).with_fields(span);

        let fields = arena.fields(record.fields.expect("field span missing"));
        assert_eq!(fields.len(), 1, "member should expose one field span entry");
        assert_eq!(fields[0].name_id, name_id, "field name should match");
        assert_eq!(fields[0].ty, bitfield_id, "field type should match");
    }
}
