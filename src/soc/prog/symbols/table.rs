//! Dense storage for symbol records plus the indices required by higher level queries.

use std::sync::Arc;

use ahash::AHashMap;
use smallvec::SmallVec;

use crate::soc::prog::types::TypeArena;

use super::id::{LabelId, SymbolHandle, SymbolId};
use super::symbol::SymbolRecord;

#[derive(Default)]
struct LabelPool {
    values: Vec<String>,
    lookup: AHashMap<String, LabelId>,
}

impl LabelPool {
    fn intern<S: AsRef<str>>(&mut self, value: S) -> LabelId {
        let value_ref = value.as_ref();
        if let Some(existing) = self.lookup.get(value_ref) {
            return *existing;
        }
        let owned = value_ref.to_owned();
        let id = LabelId::from_index(self.values.len());
        self.values.push(owned.clone());
        self.lookup.insert(owned, id);
        id
    }

    fn resolve(&self, id: LabelId) -> &str {
        &self.values[id.index()]
    }

    fn lookup<S: AsRef<str>>(&self, value: S) -> Option<LabelId> {
        self.lookup.get(value.as_ref()).copied()
    }
}

pub struct SymbolTable {
    types: Arc<TypeArena>,
    records: Vec<SymbolRecord>,
    by_label: AHashMap<LabelId, SmallVec<[SymbolHandle; 2]>>,
    by_symbol_id: AHashMap<SymbolId, SymbolHandle>,
    labels: LabelPool,
}

impl SymbolTable {
    pub fn new(types: Arc<TypeArena>) -> Self {
        Self {
            types,
            records: Vec::new(),
            by_label: AHashMap::new(),
            by_symbol_id: AHashMap::new(),
            labels: LabelPool::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn get(&self, handle: SymbolHandle) -> &SymbolRecord {
        &self.records[handle.index()]
    }

    pub fn resolve_label(&self, label: LabelId) -> &str {
        self.labels.resolve(label)
    }

    pub fn handles_by_label(&self, label: LabelId) -> Option<&[SymbolHandle]> {
        self.by_label.get(&label).map(|handles| handles.as_slice())
    }

    pub fn lookup_label<S: AsRef<str>>(&self, value: S) -> Option<LabelId> {
        self.labels.lookup(value)
    }

    pub fn handle_by_symbol_id(&self, id: SymbolId) -> Option<SymbolHandle> {
        self.by_symbol_id.get(&id).copied()
    }

    pub fn type_arena(&self) -> &Arc<TypeArena> {
        &self.types
    }

    pub(crate) fn intern_label<S: AsRef<str>>(&mut self, value: S) -> LabelId {
        self.labels.intern(value)
    }

    pub(crate) fn commit(&mut self, record: SymbolRecord) -> SymbolHandle {
        let handle = SymbolHandle::from_index(self.records.len());
        if let Some(symbol_id) = record.symbol_id {
            self.by_symbol_id.insert(symbol_id, handle);
        }
        self.by_label.entry(record.label).or_default().push(handle);
        self.records.push(record);
        handle
    }

    pub(crate) fn records(&self) -> &[SymbolRecord] {
        &self.records
    }
}

#[cfg(test)]
mod tests {
    //! Table-level guarantees that protect the higher-level builder and query APIs.
    use std::sync::Arc;

    use crate::soc::prog::types::TypeArena;

    use super::*;
    use crate::soc::prog::symbols::symbol::SymbolRecord;

    #[test]
    fn label_pool_reuses_interned_strings() {
        let types = Arc::new(TypeArena::new());
        let mut table = SymbolTable::new(types);
        let first = table.intern_label("speed");
        let second = table.intern_label("speed");
        assert_eq!(
            first, second,
            "Intern pool must deduplicate labels to keep ids stable"
        );
        assert_eq!(
            table.resolve_label(first),
            "speed",
            "Resolved label should match the original token"
        );
    }

    #[test]
    fn commit_updates_primary_indices() {
        let types = Arc::new(TypeArena::new());
        let mut table = SymbolTable::new(types);
        let label = table.intern_label("RPM");
        let mut record = SymbolRecord::new(label);
        record.symbol_id = SymbolId::from_u64(7);
        let handle = table.commit(record);
        assert_eq!(
            handle.index(),
            0,
            "First committed record should get handle zero"
        );
        assert!(
            table
                .handle_by_symbol_id(SymbolId::from_u64(7).unwrap())
                .is_some(),
            "SymbolId index should track the new record"
        );
        let handles = table.handles_by_label(label).expect("label entry");
        assert_eq!(
            handles.len(),
            1,
            "Label index should record the handle for subsequent lookups"
        );
    }
}
