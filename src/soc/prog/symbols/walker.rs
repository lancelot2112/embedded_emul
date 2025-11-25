//! Type-driven symbol traversal helpers shared across bus integrations and host tooling.

use smallvec::SmallVec;

use crate::soc::prog::types::aggregate::AggregateType;
use crate::soc::prog::types::arena::{StringId, TypeArena, TypeId};
use crate::soc::prog::types::record::TypeRecord;
use crate::soc::prog::types::scalar::{ScalarEncoding, ScalarType};
use crate::soc::prog::types::sequence::SequenceType;

/// Enumerates the primitive leaf shapes emitted by the walker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValueKind {
    Unsigned { bytes: usize },
    Signed { bytes: usize },
    Float32,
    Float64,
    Utf8 { bytes: usize },
    Enum,
    Fixed,
    Pointer { bytes: usize, target: TypeId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolWalkEntry {
    pub ty: TypeId,
    pub path: SymbolPath,
    pub offset_bits: usize,
    pub bit_len: usize,
    pub kind: ValueKind,
}

impl SymbolWalkEntry {
    pub fn byte_len(&self) -> usize {
        self.bit_len.div_ceil(8)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SymbolPath {
    segments: SmallVec<[PathSegment; 8]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PathSegment {
    Member(Option<StringId>),
    Index(usize),
}

impl SymbolPath {
    pub fn root() -> Self {
        Self {
            segments: SmallVec::new(),
        }
    }

    pub fn push_member(&self, name: Option<StringId>) -> Self {
        let mut next = self.clone();
        next.segments.push(PathSegment::Member(name));
        next
    }

    pub fn push_index(&self, index: usize) -> Self {
        let mut next = self.clone();
        next.segments.push(PathSegment::Index(index));
        next
    }

    pub fn to_string(&self, arena: &TypeArena) -> String {
        let mut text = String::new();
        for segment in &self.segments {
            match segment {
                PathSegment::Member(Some(id)) => {
                    if !text.is_empty() {
                        text.push('.');
                    }
                    text.push_str(arena.resolve_string(*id));
                }
                PathSegment::Member(None) => {
                    if !text.is_empty() {
                        text.push('.');
                    }
                    text.push_str("<unnamed>");
                }
                PathSegment::Index(idx) => {
                    text.push('[');
                    text.push_str(&idx.to_string());
                    text.push(']');
                }
            }
        }
        if text.is_empty() {
            "<root>".into()
        } else {
            text
        }
    }
}

#[derive(Clone, Debug)]
struct FrameState {
    ty: TypeId,
    offset_bits: usize,
    path: SymbolPath,
}

/// Stateful iterator that performs a depth-first walk of the provided type identifier.
pub struct SymbolWalker<'arena> {
    arena: &'arena TypeArena,
    stack: SmallVec<[FrameState; 8]>,
}

impl<'arena> SymbolWalker<'arena> {
    pub fn new(arena: &'arena TypeArena, root: TypeId) -> Self {
        let mut stack = SmallVec::new();
        stack.push(FrameState {
            ty: root,
            offset_bits: 0,
            path: SymbolPath::root(),
        });
        Self { arena, stack }
    }

    fn push_sequence(&mut self, frame: &FrameState, sequence: &SequenceType) {
        let Some(count) = sequence.element_count() else {
            return;
        };
        let stride = (sequence.stride_bytes) * 8;
        for index in (0..count).rev() {
            let offset_bits = frame.offset_bits + (index) * stride;
            self.stack.push(FrameState {
                ty: sequence.element,
                offset_bits,
                path: frame.path.push_index(index),
            });
        }
    }

    fn push_aggregate(&mut self, frame: &FrameState, aggregate: &AggregateType) {
        if aggregate.members.is_empty() {
            return;
        }
        let members = self.arena.members(aggregate.members);
        for member in members.iter().rev() {
            let offset_bits = frame.offset_bits + member.offset_bits as usize;
            let path = frame.path.push_member(member.name_id);
            self.stack.push(FrameState {
                ty: member.ty,
                offset_bits,
                path,
            });
        }
    }
}

impl<'arena> Iterator for SymbolWalker<'arena> {
    type Item = SymbolWalkEntry;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(frame) = self.stack.pop() {
            match self.arena.get(frame.ty) {
                TypeRecord::Scalar(scalar) => {
                    if let Some(entry) = walk_scalar(frame.ty, scalar, &frame) {
                        return Some(entry);
                    }
                }
                TypeRecord::Enum(_) => {
                    return Some(SymbolWalkEntry {
                        ty: frame.ty,
                        path: frame.path,
                        offset_bits: frame.offset_bits,
                        bit_len: scalar_bits(frame.ty, self.arena).unwrap_or(0),
                        kind: ValueKind::Enum,
                    });
                }
                TypeRecord::Fixed(_) => {
                    return Some(SymbolWalkEntry {
                        ty: frame.ty,
                        path: frame.path,
                        offset_bits: frame.offset_bits,
                        bit_len: scalar_bits(frame.ty, self.arena).unwrap_or(0),
                        kind: ValueKind::Fixed,
                    });
                }
                TypeRecord::Pointer(pointer) => {
                    let bytes = pointer.byte_size;
                    return Some(SymbolWalkEntry {
                        ty: frame.ty,
                        path: frame.path,
                        offset_bits: frame.offset_bits,
                        bit_len: bytes * 8,
                        kind: ValueKind::Pointer {
                            bytes,
                            target: pointer.target,
                        },
                    });
                }
                TypeRecord::Sequence(sequence) => {
                    self.push_sequence(&frame, sequence);
                }
                TypeRecord::Aggregate(aggregate) => {
                    self.push_aggregate(&frame, aggregate);
                }
                TypeRecord::BitField(bitfield) => {
                    return Some(SymbolWalkEntry {
                        ty: frame.ty,
                        path: frame.path,
                        offset_bits: frame.offset_bits,
                        bit_len: bitfield.total_width() as usize,
                        kind: ValueKind::Unsigned {
                            bytes: (bitfield.total_width() as usize).div_ceil(8),
                        },
                    });
                }
                TypeRecord::Callable(_) | TypeRecord::Dynamic(_) | TypeRecord::Opaque(_) => {
                    // Unsupported shapes are skipped entirely.
                }
            }
        }
        None
    }
}

fn walk_scalar(ty: TypeId, scalar: &ScalarType, frame: &FrameState) -> Option<SymbolWalkEntry> {
    let bit_len = scalar.byte_size * 8;
    let kind = match scalar.encoding {
        ScalarEncoding::Unsigned => ValueKind::Unsigned {
            bytes: scalar.byte_size,
        },
        ScalarEncoding::Signed => ValueKind::Signed {
            bytes: scalar.byte_size,
        },
        ScalarEncoding::Floating => match scalar.byte_size {
            4 => ValueKind::Float32,
            8 => ValueKind::Float64,
            _ => return None,
        },
        ScalarEncoding::Utf8String => ValueKind::Utf8 {
            bytes: scalar.byte_size,
        },
    };
    Some(SymbolWalkEntry {
        ty,
        path: frame.path.clone(),
        offset_bits: frame.offset_bits,
        bit_len,
        kind,
    })
}

fn scalar_bits(ty: TypeId, arena: &TypeArena) -> Option<usize> {
    match arena.get(ty) {
        TypeRecord::Scalar(scalar) => Some(scalar.byte_size * 8),
        TypeRecord::Enum(enum_type) => Some(enum_type.base.byte_size * 8),
        TypeRecord::Fixed(fixed) => Some(fixed.base.byte_size * 8),
        TypeRecord::Pointer(pointer) => Some(pointer.byte_size * 8),
        TypeRecord::BitField(bitfield) => Some(bitfield.total_width() as usize),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::prog::types::aggregate::AggregateKind;
    use crate::soc::prog::types::builder::TypeBuilder;
    use crate::soc::prog::types::pointer::PointerKind;
    use crate::soc::prog::types::record::{LayoutSize, MemberSpan, TypeRecord};
    use crate::soc::prog::types::scalar::{DisplayFormat, ScalarType};

    #[test]
    fn scalar_walk_emits_unsigned_entry() {
        let mut arena = TypeArena::new();
        let scalar = ScalarType::new(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Hex);
        let scalar_id = arena.push_record(TypeRecord::Scalar(scalar));
        let mut walker = SymbolWalker::new(&arena, scalar_id);
        let entry = walker.next().expect("entry");
        assert!(matches!(entry.kind, ValueKind::Unsigned { bytes: 4 }));
        assert_eq!(entry.path.to_string(&arena), "<root>");
        assert!(
            walker.next().is_none(),
            "walker should stop after the scalar leaf"
        );
    }

    #[test]
    fn aggregate_walks_members_in_order() {
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let a = builder.scalar(Some("a"), 4, ScalarEncoding::Unsigned, DisplayFormat::Hex);
        let b = builder.scalar(Some("b"), 4, ScalarEncoding::Unsigned, DisplayFormat::Hex);
        let agg_id = builder
            .aggregate(AggregateKind::Struct)
            .layout(8, 0)
            .member("a", a, 0)
            .member("b", b, 4)
            .finish();
        let mut walker = SymbolWalker::new(&arena, agg_id);
        let first = walker.next().expect("first member");
        assert_eq!(first.path.to_string(&arena), "a");
        let second = walker.next().expect("second member");
        assert_eq!(second.path.to_string(&arena), "b");
        assert!(walker.next().is_none());
    }

    #[test]
    fn sequence_walks_elements() {
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let word = builder.scalar(None, 2, ScalarEncoding::Unsigned, DisplayFormat::Hex);
        let seq_id = builder.sequence_static(word, 2, 3);
        let walker = SymbolWalker::new(&arena, seq_id);
        let mut paths = Vec::new();
        for entry in walker {
            paths.push(entry.path.to_string(&arena));
        }
        assert_eq!(paths, vec!["[0]", "[1]", "[2]"]);
    }

    #[test]
    fn pointer_walk_emits_pointer_entry() {
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let word = builder.scalar(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Hex);
        let ptr_id = builder.pointer(word, PointerKind::Data, 8);
        let mut walker = SymbolWalker::new(&arena, ptr_id);
        let entry = walker.next().expect("pointer");
        assert!(matches!(entry.kind, ValueKind::Pointer { bytes: 8, target } if target == word));
    }

    #[test]
    fn aggregate_without_members_is_skipped() {
        let mut arena = TypeArena::new();
        let span = MemberSpan::empty();
        let agg = AggregateType::new(AggregateKind::Struct, span, LayoutSize::ZERO);
        let agg_id = arena.push_record(TypeRecord::Aggregate(agg));
        let mut walker = SymbolWalker::new(&arena, agg_id);
        assert!(walker.next().is_none(), "empty aggregates have no leaves");
    }

    #[test]
    fn union_members_share_offsets() {
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let as_u32 = builder.scalar(
            Some("as_u32"),
            4,
            ScalarEncoding::Unsigned,
            DisplayFormat::Hex,
        );
        let as_float = builder.scalar(
            Some("as_f32"),
            4,
            ScalarEncoding::Floating,
            DisplayFormat::Default,
        );
        let union_id = builder
            .aggregate(AggregateKind::Union)
            .layout(4, 0)
            .member("as_u32", as_u32, 0)
            .member("as_f32", as_float, 0)
            .finish();
        let mut walker = SymbolWalker::new(&arena, union_id);
        let first = walker.next().expect("first union member");
        let second = walker.next().expect("second union member");
        assert_eq!(
            first.offset_bits, 0,
            "union elements should overlay at byte zero"
        );
        assert_eq!(
            second.offset_bits, 0,
            "all union elements overlay the same offset"
        );
        assert_eq!(first.path.to_string(&arena), "as_u32");
        assert_eq!(second.path.to_string(&arena), "as_f32");
        assert!(
            walker.next().is_none(),
            "union should only expose defined members"
        );
    }
}
