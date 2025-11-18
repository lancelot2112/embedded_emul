//! Helpers for computing effective symbol and type sizes used by the bus bridge.

use crate::soc::prog::types::arena::{TypeArena, TypeId};
use crate::soc::prog::types::record::TypeRecord;
use crate::soc::prog::types::sequence::{SequenceCount, SequenceType};

pub fn type_size(arena: &TypeArena, ty: TypeId) -> Option<u32> {
    match arena.get(ty) {
        TypeRecord::Scalar(scalar) => Some(scalar.byte_size),
        TypeRecord::Enum(enum_type) => Some(enum_type.base.byte_size),
        TypeRecord::Fixed(fixed) => Some(fixed.base.byte_size),
        TypeRecord::Sequence(seq) => sequence_size(seq),
        TypeRecord::Aggregate(agg) => Some(agg.byte_size.bytes),
        TypeRecord::Opaque(opaque) => Some(opaque.byte_size),
        TypeRecord::Pointer(pointer) => Some(pointer.byte_size),
        _ => None,
    }
}

fn sequence_size(seq: &SequenceType) -> Option<u32> {
    match seq.count {
        SequenceCount::Static(count) => count.checked_mul(seq.stride_bytes),
        SequenceCount::Dynamic(_) => None,
    }
}
