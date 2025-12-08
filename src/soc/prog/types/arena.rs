//! Stores canonicalized type records plus auxiliary metadata used throughout the subsystem.

use std::num::NonZeroU32;

use ahash::AHashMap;

use super::arena_record::{ArenaSpan, FieldRecord, MemberRecord, TypeRecord};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TypeId(NonZeroU32);

impl TypeId {
    pub fn from_index(index: usize) -> Self {
        let raw = NonZeroU32::new((index as u32) + 1).expect("index overflow");
        Self(raw)
    }

    pub fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StringId(NonZeroU32);

impl StringId {
    fn from_index(index: usize) -> Self {
        let raw = NonZeroU32::new((index as u32) + 1).expect("string index overflow");
        Self(raw)
    }

    pub fn index(self) -> usize {
        (self.0.get() - 1) as usize
    }
}

#[derive(Default, Debug)]
struct StringPool {
    values: Vec<String>,
    lookup: AHashMap<String, StringId>,
}

impl StringPool {
    fn intern<S: AsRef<str>>(&mut self, value: S) -> StringId {
        let value_ref = value.as_ref();
        if let Some(id) = self.lookup.get(value_ref) {
            return *id;
        }
        let owned = value_ref.to_owned();
        let id = StringId::from_index(self.values.len());
        self.values.push(owned.clone());
        self.lookup.insert(owned, id);
        id
    }

    fn resolve(&self, id: StringId) -> &str {
        &self.values[id.index()]
    }
}

#[derive(Default)]
pub struct TypeArena {
    records: Vec<TypeRecord>,
    members: Vec<MemberRecord>,
    fields: Vec<FieldRecord>,
    strings: StringPool,
}

impl TypeArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_record(&mut self, record: TypeRecord) -> TypeId {
        self.records.push(record);
        TypeId::from_index(self.records.len() - 1)
    }

    pub fn get(&self, id: TypeId) -> &TypeRecord {
        &self.records[id.index()]
    }

    pub fn get_mut(&mut self, id: TypeId) -> &mut TypeRecord {
        &mut self.records[id.index()]
    }

    pub fn alloc_members<I>(&mut self, members: I) -> ArenaSpan
    where
        I: IntoIterator<Item = MemberRecord>,
    {
        let start = self.members.len();
        self.members.extend(members);
        ArenaSpan::new(start, self.members.len() - start)
    }

    pub fn members(&self, span: ArenaSpan) -> &[MemberRecord] {
        let start = span.start();
        let end = start + span.len();
        &self.members[start..end]
    }

    pub fn alloc_fields<I>(&mut self, fields: I) -> ArenaSpan
    where
        I: IntoIterator<Item = FieldRecord>,
    {
        let start = self.fields.len();
        self.fields.extend(fields);
        let len = self.fields.len() - start;
        if len == 0 {
            ArenaSpan::empty()
        } else {
            ArenaSpan::new(start, len)
        }
    }

    pub fn fields(&self, span: ArenaSpan) -> &[FieldRecord] {
        let start = span.start();
        let end = start + span.len();
        &self.fields[start..end]
    }

    pub fn intern_string<S: AsRef<str>>(&mut self, value: S) -> StringId {
        self.strings.intern(value)
    }

    pub fn resolve_string(&self, id: StringId) -> &str {
        self.strings.resolve(id)
    }
}

#[cfg(test)]
mod tests {
    //! Basic coverage for arena bookkeeping and string interning layers.
    use super::*;
    use crate::soc::prog::types::bitfield::BitFieldSpec;
    use crate::soc::prog::types::scalar::{DisplayFormat, ScalarEncoding, ScalarType};

    #[test]
    fn pushing_records_returns_dense_ids() {
        // The first pushed record should get index zero encoded as NonZeroU32::new(1)
        let mut arena = TypeArena::new();
        let scalar = ScalarType::new(None, 1, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let id = arena.push_record(scalar.into());
        assert_eq!(id.index(), 0, "Dense indices keep traversal cache-friendly");
    }

    #[test]
    fn string_interning_reuses_existing_entries() {
        // Interning the same string twice must return identical identities
        let mut arena = TypeArena::new();
        let first = arena.intern_string("status");
        let second = arena.intern_string("status");
        assert_eq!(
            first, second,
            "Intern pool should deduplicate identical strings"
        );
        assert_eq!(
            arena.resolve_string(first),
            "status",
            "Resolved string should match original token"
        );
    }

    #[test]
    fn allocating_fields_returns_span() {
        // ensure fields land in their own dense side table
        let mut arena = TypeArena::new();
        let spec = BitFieldSpec::from_range(32, 0, 8);
        let field_ty = arena.push_record(TypeRecord::BitField(spec));
        let label = arena.intern_string("bits");
        let span = arena.alloc_fields([FieldRecord::new(label, field_ty)]);
        assert_eq!(span.len(), 1, "field span should report inserted length");
        let slice = arena.fields(span);
        assert_eq!(slice.len(), 1, "slice should expose stored field");
        assert_eq!(slice[0].name_id, label, "field label should match");
    }
}
