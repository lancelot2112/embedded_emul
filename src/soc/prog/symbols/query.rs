//! Lightweight filtering utilities for iterating across the symbol table.

use super::id::{LabelId, SymbolHandle};
use super::source::SymbolSource;
use super::symbol::{SymbolBinding, SymbolRecord};
use super::table::SymbolTable;

pub struct SymbolQuery<'a> {
    table: &'a SymbolTable,
    binding: Option<SymbolBinding>,
    label: Option<LabelId>,
    source: Option<SymbolSource>,
    runtime_addr: Option<usize>,
    file_addr: Option<usize>,
}

impl<'a> SymbolQuery<'a> {
    pub(crate) fn new(table: &'a SymbolTable) -> Self {
        Self {
            table,
            binding: None,
            label: None,
            source: None,
            runtime_addr: None,
            file_addr: None,
        }
    }

    pub fn binding(mut self, binding: SymbolBinding) -> Self {
        self.binding = Some(binding);
        self
    }

    pub fn label(mut self, label: LabelId) -> Self {
        self.label = Some(label);
        self
    }

    pub fn source(mut self, source: SymbolSource) -> Self {
        self.source = Some(source);
        self
    }

    pub fn runtime_addr(mut self, addr: usize) -> Self {
        self.runtime_addr = Some(addr);
        self
    }

    pub fn file_addr(mut self, addr: usize) -> Self {
        self.file_addr = Some(addr);
        self
    }

    pub fn iter(self) -> SymbolQueryIter<'a> {
        SymbolQueryIter {
            table: self.table,
            binding: self.binding,
            label: self.label,
            source: self.source,
            runtime_addr: self.runtime_addr,
            file_addr: self.file_addr,
            index: 0,
        }
    }

    pub fn first(self) -> Option<(SymbolHandle, &'a SymbolRecord)> {
        self.iter().next()
    }
}

pub struct SymbolQueryIter<'a> {
    table: &'a SymbolTable,
    binding: Option<SymbolBinding>,
    label: Option<LabelId>,
    source: Option<SymbolSource>,
    runtime_addr: Option<usize>,
    file_addr: Option<usize>,
    index: usize,
}

impl<'a> Iterator for SymbolQueryIter<'a> {
    type Item = (SymbolHandle, &'a SymbolRecord);

    fn next(&mut self) -> Option<Self::Item> {
        let records = self.table.records();
        while self.index < records.len() {
            let record = &records[self.index];
            let handle = SymbolHandle::from_index(self.index);
            self.index += 1;
            if self.matches(record) {
                return Some((handle, record));
            }
        }
        None
    }
}

impl<'a> SymbolQueryIter<'a> {
    fn matches(&self, record: &SymbolRecord) -> bool {
        if let Some(binding) = self.binding
            && record.binding != binding
        {
            return false;
        }
        if let Some(label) = self.label
            && record.label != label
        {
            return false;
        }
        if let Some(source) = self.source
            && !record.provenance.sources.contains(source)
        {
            return false;
        }
        if let Some(addr) = self.runtime_addr
            && record.runtime_addr != Some(addr)
        {
            return false;
        }
        if let Some(addr) = self.file_addr
            && record.file_addr != Some(addr)
        {
            return false;
        }
        true
    }
}

impl SymbolTable {
    pub fn query(&self) -> SymbolQuery<'_> {
        SymbolQuery::new(self)
    }
}

#[cfg(test)]
mod tests {
    //! Ensures filters compose predictably so higher-level services can rely on them.
    use std::sync::Arc;

    use crate::soc::prog::symbols::source::SymbolSource;
    use crate::soc::prog::symbols::symbol::{SymbolBinding, SymbolState};
    use crate::soc::prog::symbols::table::SymbolTable;
    use crate::soc::prog::types::TypeArena;

    #[test]
    fn binding_filter_limits_results() {
        let types = Arc::new(TypeArena::new());
        let mut table = SymbolTable::new(types);
        table
            .builder()
            .label("GLOBAL_FN")
            .binding(SymbolBinding::Global)
            .runtime_addr(0x10)
            .finish();
        table
            .builder()
            .label("LOCAL_FN")
            .binding(SymbolBinding::Local)
            .runtime_addr(0x20)
            .finish();
        let mut found = table.query().binding(SymbolBinding::Global).iter();
        let (_, record) = found.next().expect("global result present");
        assert_eq!(
            record.binding,
            SymbolBinding::Global,
            "Iterator should only yield global bindings when filter is active"
        );
        assert!(
            found.next().is_none(),
            "Only one global symbol was inserted, so exactly one result should be produced"
        );
    }

    #[test]
    fn runtime_and_source_filter_can_stack() {
        let types = Arc::new(TypeArena::new());
        let mut table = SymbolTable::new(types);
        table
            .builder()
            .label("RUNTIME_MATCH")
            .runtime_addr(0xAA)
            .source(SymbolSource::TOOL)
            .state(SymbolState::Defined)
            .finish();
        table
            .builder()
            .label("RUNTIME_OTHER")
            .runtime_addr(0xBB)
            .source(SymbolSource::ELF)
            .finish();
        let result = table
            .query()
            .runtime_addr(0xAA)
            .source(SymbolSource::TOOL)
            .first();
        assert!(
            result.is_some(),
            "Combined filters should still locate the unique runtime + source match"
        );
        let (_, record) = result.unwrap();
        assert_eq!(
            record.runtime_addr,
            Some(0xAA),
            "Result should carry the requested runtime address"
        );
    }
}
