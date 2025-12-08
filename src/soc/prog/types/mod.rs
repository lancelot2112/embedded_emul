//! Entry point for the `soc::prog::types` subsystem which implements the architecture plan in `architecture.md`.

pub mod aggregate;
pub mod arena;
pub mod arena_record;
pub mod bitfield;
pub mod builder;
pub mod callable;
pub mod dynamic;
pub mod enum_scalar;
pub mod expr;
pub mod fmt;
pub mod literal;
pub mod pointer;
pub mod range;
pub mod scalar;
pub mod scalar_with_fields;
pub mod sequence;
pub mod walker;

pub use aggregate::{AggregateBuilder, AggregateKind, AggregateType};
pub use arena::{StringId, TypeArena, TypeId};
pub use arena_record::{ArenaSpan, FieldRecord, LayoutSize, MemberRecord, OpaqueType, TypeRecord};
pub use bitfield::{BitFieldSegment, BitFieldSpec, BitFieldSpecBuilder, PadKind, PadSpec};
pub use builder::{DebugTypeProvider, RawTypeDesc, TypeBuilder};
pub use callable::CallableType;
pub use dynamic::{DynamicAggregate, DynamicField};
pub use enum_scalar::{EnumBuilder, EnumType};
pub use expr::{EvalContext, ExprProgram, OpCode};
pub use literal::{
    Literal, LiteralError, LiteralKind, parse_index_suffix, parse_u32_literal, parse_u64_literal,
};
pub use pointer::{PointerKind, PointerQualifiers, PointerType};
pub use range::{RangeSpec, RangeSpecError};
pub use scalar::{DisplayFormat, ScalarEncoding, ScalarType};
pub use scalar_with_fields::{ScalarWithFieldsBuilder, ScalarWithFieldsRecord};
pub use sequence::{CountSource, SequenceCount, SequenceType};
pub use walker::{MemberCursor, ResolvedMember, TypeWalker};
