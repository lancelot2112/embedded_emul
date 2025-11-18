//! Entry point for the `soc::prog::types` subsystem which implements the architecture plan in `architecture.md`.

pub mod arena;
pub mod record;
pub mod scalar;
pub mod bitfield;
pub mod aggregate;
pub mod sequence;
pub mod pointer;
pub mod callable;
pub mod expr;
pub mod dynamic;
pub mod walker;
pub mod builder;
pub mod fmt;

pub use arena::{StringId, TypeArena, TypeId};
pub use aggregate::{AggregateKind, AggregateType};
pub use builder::{AggregateBuilder, DebugTypeProvider, EnumBuilder, RawTypeDesc, TypeBuilder};
pub use callable::CallableType;
pub use dynamic::{DynamicAggregate, DynamicField};
pub use expr::{EvalContext, ExprProgram, OpCode};
pub use pointer::{PointerKind, PointerQualifiers, PointerType};
pub use record::{LayoutSize, MemberRecord, MemberSpan, OpaqueType, TypeRecord};
pub use scalar::{DisplayFormat, EnumType, ScalarEncoding, ScalarType};
pub use bitfield::{BitFieldSegment, BitFieldSpec, BitFieldSpecBuilder, PadKind, PadSpec};
pub use sequence::{CountSource, SequenceCount, SequenceType};
pub use walker::{MemberCursor, ResolvedMember, TypeWalker};
