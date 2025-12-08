//! Aggregate type description for structs, unions, classes, and tagged variants.

use smallvec::SmallVec;

use crate::soc::prog::types::{TypeArena, TypeBuilder, TypeId, TypeRecord};

use super::arena::StringId;
use super::arena_record::{ArenaSpan, LayoutSize, MemberRecord};
use super::scalar_with_fields::ScalarWithFieldsBuilder;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggregateKind {
    Struct,
    Class,
    Union,
    Variant,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StaticMember {
    pub label: StringId,
    pub variable_id: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AggregateType {
    pub kind: AggregateKind,
    //Member records are stored in the type arena's member storage
    pub members: ArenaSpan,
    pub static_members: SmallVec<[StaticMember; 2]>,
    pub byte_size: LayoutSize,
    pub has_dynamic: bool,
}

impl AggregateType {
    pub fn new(kind: AggregateKind, members: ArenaSpan, byte_size: LayoutSize) -> Self {
        Self {
            kind,
            members,
            static_members: SmallVec::new(),
            byte_size,
            has_dynamic: false,
        }
    }

    pub fn is_union(&self) -> bool {
        matches!(self.kind, AggregateKind::Union)
    }

    pub fn push_static_member(&mut self, label: StringId, variable_id: i64) {
        self.static_members
            .push(StaticMember { label, variable_id });
    }
}

pub struct AggregateBuilder<'builder, 'arena> {
    builder: &'builder mut TypeBuilder<'arena>,
    kind: AggregateKind,
    members: Vec<PendingMember>,
    active_bit_store: Option<usize>,
    static_members: SmallVec<[StaticMember; 2]>,
    layout: LayoutSize,
    has_dynamic: bool,
}

impl<'builder, 'arena> AggregateBuilder<'builder, 'arena> {
    pub(super) fn new(builder: &'builder mut TypeBuilder<'arena>, kind: AggregateKind) -> Self {
        Self {
            builder,
            kind,
            members: Vec::new(),
            active_bit_store: None,
            static_members: SmallVec::new(),
            layout: LayoutSize::ZERO,
            has_dynamic: false,
        }
    }

    pub fn layout(mut self, bytes: usize, trailing_bits: usize) -> Self {
        self.layout = LayoutSize {
            bytes,
            trailing_bits,
        };
        self
    }

    pub fn mark_dynamic(mut self) -> Self {
        self.has_dynamic = true;
        self
    }

    pub fn member(mut self, name: impl AsRef<str>, ty: TypeId, byte_offset: usize) -> Self {
        let name_id = Some(self.builder.intern(name));
        let record = MemberRecord::new(name_id, ty, byte_offset * 8);
        self.members.push(PendingMember::new(record));
        self.active_bit_store = None;
        self
    }

    pub fn bit_store(mut self, byte_offset: usize, ty: TypeId) -> Self {
        let record = MemberRecord::new(None, ty, byte_offset * 8);
        let mut pending = PendingMember::new(record);
        let arena_ref: &TypeArena = &*self.builder.arena;
        pending.init_scalar_fields(ty, arena_ref);
        self.members.push(pending);
        self.active_bit_store = Some(self.members.len() - 1);
        self
    }

    pub fn member_bits(mut self, name: impl AsRef<str>, offset_bits: usize, bit_size: u16) -> Self {
        let store_index = self
            .active_bit_store
            .expect("member_bits must follow a bit_store declaration");
        let pending = self
            .members
            .get_mut(store_index)
            .expect("bit_store index should remain valid");
        let name_id = self.builder.intern(name);
        pending.push_bitfield(name_id, offset_bits, bit_size);
        self
    }

    pub fn member_record(mut self, record: MemberRecord) -> Self {
        self.members.push(PendingMember::new(record));
        self.active_bit_store = None;
        self
    }

    pub fn static_member(mut self, label: impl AsRef<str>, variable_id: i64) -> Self {
        let label_id = self.builder.intern(label);
        self.static_members.push(StaticMember {
            label: label_id,
            variable_id,
        });
        self
    }

    pub fn finish(self) -> TypeId {
        let finalized: Vec<MemberRecord> = self
            .members
            .into_iter()
            .map(|pending| pending.finalize(self.builder.arena))
            .collect();
        let span = if finalized.is_empty() {
            ArenaSpan::empty()
        } else {
            self.builder.arena.alloc_members(finalized)
        };
        let mut aggregate = AggregateType::new(self.kind, span, self.layout);
        aggregate.static_members = self.static_members;
        aggregate.has_dynamic = self.has_dynamic;
        self.builder
            .arena
            .push_record(TypeRecord::Aggregate(aggregate))
    }
}

struct PendingMember {
    record: MemberRecord,
    scalar_fields: Option<ScalarWithFieldsBuilder>,
}

impl PendingMember {
    fn new(record: MemberRecord) -> Self {
        Self {
            record,
            scalar_fields: None,
        }
    }

    fn init_scalar_fields(&mut self, storage: TypeId, arena: &TypeArena) {
        self.scalar_fields = Some(ScalarWithFieldsBuilder::from_scalar(arena, storage));
    }

    fn push_bitfield(&mut self, name_id: StringId, offset_bits: usize, bit_size: u16) {
        let plan = self
            .scalar_fields
            .as_mut()
            .expect("bitfield plan should be initialised by bit_store");
        let offset =
            u16::try_from(offset_bits).expect("bitfield offset exceeds supported u16 range");
        plan.push_range(name_id, offset, bit_size);
    }

    fn finalize(mut self, arena: &mut TypeArena) -> MemberRecord {
        if let Some(plan) = self.scalar_fields {
            if !plan.is_empty() {
                let ty = plan.build(arena);
                self.record.ty = ty;
            }
        }
        self.record
    }
}

impl<'arena> TypeBuilder<'arena> {
    pub fn aggregate(&mut self, kind: AggregateKind) -> AggregateBuilder<'_, 'arena> {
        AggregateBuilder::new(self, kind)
    }
}

#[cfg(test)]
mod tests {
    //! Ensures aggregate metadata mirrors the intended layout semantics.
    use crate::soc::prog::types::{DisplayFormat, ScalarEncoding, TypeArena};

    use super::*;

    #[test]
    fn unions_report_helpers() {
        // verifying simple union detection logic for walker heuristics
        let span = ArenaSpan::new(0, 0);
        let agg = AggregateType::new(AggregateKind::Union, span, LayoutSize::ZERO);
        assert!(
            agg.is_union(),
            "AggregateKind::Union must report true from is_union"
        );
    }

    #[test]
    fn aggregate_builder_tracks_padding_for_alignment() {
        // struct builder should allow explicit offsets to account for alignment/padding
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let u8_ty = builder.scalar(None, 1, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let u32_ty = builder.scalar(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let aggregate_id = builder
            .aggregate(AggregateKind::Struct)
            .layout(8, 0)
            .member("head", u8_ty, 0)
            .member("value", u32_ty, 4)
            .finish();

        let TypeRecord::Aggregate(agg) = arena.get(aggregate_id) else {
            panic!("expected aggregate type");
        };
        assert_eq!(
            agg.byte_size.bytes, 8,
            "struct layout should include padding up to 8 bytes"
        );
        let members = arena.members(agg.members);
        assert_eq!(
            members.len(),
            2,
            "struct should contain both declared members"
        );
        assert_eq!(
            members[0].offset_bits, 0,
            "first member should start at byte zero"
        );
        assert_eq!(
            members[1].offset_bits, 32,
            "second member should honor 4-byte alignment"
        );
    }

    #[test]
    fn aggregate_builder_chains_members() {
        // aggregate builder should allow fluent member definition and finish into the arena
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let word = builder.scalar(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let aggregate_id = builder
            .aggregate(AggregateKind::Struct)
            .layout(8, 0)
            .member("x", word, 0)
            .member("y", word, 4)
            .finish();

        let TypeRecord::Aggregate(agg) = arena.get(aggregate_id) else {
            panic!("expected aggregate type");
        };
        assert_eq!(
            arena.members(agg.members).len(),
            2,
            "struct builder should create two members"
        );
    }

    #[test]
    fn aggregate_builder_handles_bitfields() {
        // aggregate builder should store bitfield members correctly
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let byte = builder.scalar(None, 1, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let aggregate_id = builder
            .aggregate(AggregateKind::Struct)
            .layout(2, 0)
            .bit_store(0, byte)
            .member_bits("flags", 0, 3)
            .member_bits("value", 3, 5)
            .finish();

        let TypeRecord::Aggregate(agg) = arena.get(aggregate_id) else {
            panic!("expected aggregate type");
        };
        let members = arena.members(agg.members);
        assert_eq!(
            members.len(),
            1,
            "bit store should be represented by a single member"
        );
        let TypeRecord::ScalarWithFields(record) = arena.get(members[0].ty) else {
            panic!("bit store should materialise as ScalarWithFields record");
        };
        let fields = arena.fields(record.fields);
        assert_eq!(
            fields.len(),
            2,
            "struct should contain both bitfield members"
        );
    }
}
