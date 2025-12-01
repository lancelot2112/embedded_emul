//! Light-weight construction helpers that bridge debugger metadata into the arena and expose a fluent API for manual builders.

use smallvec::SmallVec;

use super::aggregate::{AggregateKind, AggregateType, StaticMember};
use super::arena::{StringId, TypeArena, TypeId};
use super::bitfield::BitFieldSpec;
use super::pointer::{PointerKind, PointerType};
use super::record::{LayoutSize, MemberRecord, MemberSpan, TypeRecord};
use super::scalar::{DisplayFormat, EnumType, EnumVariant, ScalarEncoding, ScalarType};
use super::sequence::{SequenceCount, SequenceType};

pub struct TypeBuilder<'arena> {
    arena: &'arena mut TypeArena,
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

    pub fn aggregate(&mut self, kind: AggregateKind) -> AggregateBuilder<'_, 'arena> {
        AggregateBuilder::new(self, kind)
    }

    pub fn enumeration(&mut self, base: ScalarType) -> EnumBuilder<'_, 'arena> {
        EnumBuilder::new(self, base)
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

pub struct AggregateBuilder<'builder, 'arena> {
    builder: &'builder mut TypeBuilder<'arena>,
    kind: AggregateKind,
    members: Vec<MemberRecord>,
    static_members: SmallVec<[StaticMember; 2]>,
    layout: LayoutSize,
    has_dynamic: bool,
}

impl<'builder, 'arena> AggregateBuilder<'builder, 'arena> {
    fn new(builder: &'builder mut TypeBuilder<'arena>, kind: AggregateKind) -> Self {
        Self {
            builder,
            kind,
            members: Vec::new(),
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
        self.members.push(record);
        self
    }

    pub fn member_bits(
        mut self,
        name: impl AsRef<str>,
        ty: TypeId,
        offset_bits: usize,
        bit_size: u16,
    ) -> Self {
        let name_id = Some(self.builder.intern(name));
        let record = MemberRecord::new(name_id, ty, offset_bits).with_bitfield(bit_size);
        self.members.push(record);
        self
    }

    pub fn member_record(mut self, record: MemberRecord) -> Self {
        self.members.push(record);
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
        let span = if self.members.is_empty() {
            MemberSpan::empty()
        } else {
            self.builder.arena.alloc_members(self.members)
        };
        let mut aggregate = AggregateType::new(self.kind, span, self.layout);
        aggregate.static_members = self.static_members;
        aggregate.has_dynamic = self.has_dynamic;
        self.builder
            .arena
            .push_record(TypeRecord::Aggregate(aggregate))
    }
}

pub struct EnumBuilder<'builder, 'arena> {
    builder: &'builder mut TypeBuilder<'arena>,
    ty: EnumType,
}

impl<'builder, 'arena> EnumBuilder<'builder, 'arena> {
    fn new(builder: &'builder mut TypeBuilder<'arena>, base: ScalarType) -> Self {
        Self {
            builder,
            ty: EnumType::new(base),
        }
    }

    pub fn variant(mut self, label: impl AsRef<str>, value: i64) -> Self {
        let label_id = self.builder.intern(label);
        self.ty.push_variant(EnumVariant {
            label: label_id,
            value,
        });
        self
    }

    pub fn finish(self) -> TypeId {
        self.builder.arena.push_record(TypeRecord::Enum(self.ty))
    }
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
    fn enum_builder_collects_variants() {
        // enum builder should collect label/value pairs fluently
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let base = ScalarType::new(None, 1, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let enum_id = builder
            .enumeration(base)
            .variant("Ready", 1)
            .variant("Busy", 2)
            .finish();

        let TypeRecord::Enum(enum_ty) = arena.get(enum_id) else {
            panic!("expected enum type");
        };
        assert_eq!(
            enum_ty.variants.len(),
            2,
            "enum builder should store all variants"
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
}
