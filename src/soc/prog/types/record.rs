//! Defines the canonical record structures stored inside the type arena.

use super::aggregate::AggregateType;
use super::arena::{StringId, TypeId};
use super::bitfield::BitFieldSpec;
use super::callable::CallableType;
use super::dynamic::DynamicAggregate;
use super::pointer::PointerType;
use super::scalar::{EnumType, FixedScalar, ScalarType};
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

/// Describes a contiguous slice of members stored inside the arena side table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemberSpan {
    start: u32,
    len: u32,
}

impl MemberSpan {
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberRecord {
    pub name_id: Option<StringId>,
    pub ty: TypeId,
    pub offset_bits: usize,
    pub bit_size: Option<usize>,
}

impl MemberRecord {
    pub fn new(name_id: Option<StringId>, ty: TypeId, offset_bits: usize) -> Self {
        Self {
            name_id,
            ty,
            offset_bits,
            bit_size: None,
        }
    }

    pub fn with_bitfield(mut self, bit_size: u16) -> Self {
        self.bit_size = Some(bit_size as usize);
        self
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
}

impl TypeRecord {
    pub fn as_scalar(&self) -> Option<&ScalarType> {
        if let TypeRecord::Scalar(value) = self {
            Some(value)
        } else {
            None
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
        let span = MemberSpan::new(4, 2);
        assert_eq!(
            span.start(),
            4,
            "start index should match constructor argument"
        );
        assert_eq!(span.len(), 2, "length should match constructor argument");
    }

    #[test]
    fn member_record_supports_bitfields() {
        // verify with_bitfield flips the optional bit size as expected
        let mut arena = TypeArena::new();
        let scalar_id = dummy_scalar(&mut arena);
        let record = MemberRecord::new(None, scalar_id, 0).with_bitfield(3);
        assert_eq!(
            record.bit_size,
            Some(3),
            "bit size should be set to three bits"
        );
    }
}
