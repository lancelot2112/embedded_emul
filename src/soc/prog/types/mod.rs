//! Entry point for the `soc::prog::types` subsystem which implements the architecture plan in `architecture.md`.

pub mod aggregate;
pub mod arena;
pub mod bitfield;
pub mod builder;
pub mod callable;
pub mod dynamic;
pub mod expr;
pub mod fmt;
pub mod literal;
pub mod pointer;
pub mod range;
pub mod record;
pub mod scalar;
pub mod sequence;
pub mod walker;

pub use aggregate::{AggregateKind, AggregateType};
pub use arena::{StringId, TypeArena, TypeId};
pub use bitfield::{BitFieldSegment, BitFieldSpec, BitFieldSpecBuilder, PadKind, PadSpec};
pub use builder::{AggregateBuilder, DebugTypeProvider, EnumBuilder, RawTypeDesc, TypeBuilder};
pub use callable::CallableType;
pub use dynamic::{DynamicAggregate, DynamicField};
pub use expr::{EvalContext, ExprProgram, OpCode};
pub use literal::{
    Literal, LiteralError, LiteralKind, parse_index_suffix, parse_u32_literal, parse_u64_literal,
};
pub use pointer::{PointerKind, PointerQualifiers, PointerType};
pub use range::{RangeSpec, RangeSpecError};
pub use record::{LayoutSize, MemberRecord, MemberSpan, OpaqueType, TypeRecord};
pub use scalar::{DisplayFormat, EnumType, ScalarEncoding, ScalarType};
pub use sequence::{CountSource, SequenceCount, SequenceType};
pub use walker::{MemberCursor, ResolvedMember, TypeWalker};
