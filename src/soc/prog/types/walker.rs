//! Encapsulated visitor utilities for traversing nested types without manual tree walking.

use smallvec::SmallVec;

use super::aggregate::AggregateType;
use super::arena::{TypeArena, TypeId};
use super::record::{MemberRecord, TypeRecord};

#[derive(Clone, Debug)]
pub struct ResolvedMember<'a> {
    pub record: &'a MemberRecord,
    pub absolute_offset_bits: u32,
}

pub struct TypeWalker<'arena> {
    arena: &'arena TypeArena,
}

impl<'arena> TypeWalker<'arena> {
    pub fn new(arena: &'arena TypeArena) -> Self {
        Self { arena }
    }

    pub fn cursor(&'arena self, root: TypeId) -> MemberCursor<'arena> {
        MemberCursor::new(self.arena, root)
    }
}

#[derive(Clone, Debug)]
struct CursorFrame<'a> {
    members: &'a [MemberRecord],
    index: usize,
    base_offset_bits: u32,
}

pub struct MemberCursor<'arena> {
    arena: &'arena TypeArena,
    stack: SmallVec<[CursorFrame<'arena>; 4]>,
}

impl<'arena> MemberCursor<'arena> {
    pub fn new(arena: &'arena TypeArena, root: TypeId) -> Self {
        let mut cursor = Self {
            arena,
            stack: SmallVec::new(),
        };
        cursor.push_type(root, 0);
        cursor
    }

    fn push_type(&mut self, ty: TypeId, base_offset_bits: u32) {
        if let TypeRecord::Aggregate(AggregateType { members, .. }) = self.arena.get(ty) {
            let slice = self.arena.members(*members);
            self.stack.push(CursorFrame {
                members: slice,
                index: 0,
                base_offset_bits,
            });
        }
    }

    pub fn next(&mut self) -> Option<ResolvedMember<'arena>> {
        while let Some(frame) = self.stack.last_mut() {
            if frame.index >= frame.members.len() {
                self.stack.pop();
                continue;
            }
            let member = &frame.members[frame.index];
            frame.index += 1;
            let absolute_offset = frame.base_offset_bits + member.offset_bits;
            self.push_type(member.ty, absolute_offset);
            return Some(ResolvedMember {
                record: member,
                absolute_offset_bits: absolute_offset,
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    //! Validates traversal logic stays isolated from aggregate internals.
    use super::*;
    use crate::soc::prog::types::aggregate::AggregateKind;
    use crate::soc::prog::types::arena::TypeArena;
    use crate::soc::prog::types::record::{LayoutSize, MemberRecord, TypeRecord};
    use crate::soc::prog::types::scalar::{DisplayFormat, ScalarEncoding, ScalarType};

    #[test]
    fn walker_iterates_aggregate_members() {
        // ensures MemberCursor performs DFS over aggregate members and sub-members
        let mut arena = TypeArena::new();
        let scalar = ScalarType::new(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let scalar_id = arena.push_record(TypeRecord::Scalar(scalar));
        let members = arena.alloc_members([
            MemberRecord::new(None, scalar_id, 0),
            MemberRecord::new(None, scalar_id, 32),
        ]);
        let aggregate = AggregateType::new(AggregateKind::Struct, members, LayoutSize { bytes: 8, trailing_bits: 0 });
        let agg_id = arena.push_record(TypeRecord::Aggregate(aggregate));
        let walker = TypeWalker::new(&arena);
        let mut cursor = walker.cursor(agg_id);
        let mut count = 0;
        while cursor.next().is_some() {
            count += 1;
        }
        assert_eq!(count, 2, "walker should visit each member exactly once");
    }
}
