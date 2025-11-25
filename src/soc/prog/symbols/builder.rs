//! Fluent builder used by loaders and tests to author symbol records.

use crate::soc::device::endianness::Endianness;
use crate::soc::prog::types::TypeId;

use super::id::{LabelId, SymbolHandle, SymbolId};
use super::source::{SourceTrust, SymbolProvenance, SymbolSource};
use super::symbol::{
    StorageClass, SymbolBinding, SymbolInfo, SymbolKind, SymbolRecord, SymbolState,
    SymbolVisibility, ToolFlags,
};
use super::table::SymbolTable;

pub struct SymbolBuilder<'a> {
    table: &'a mut SymbolTable,
    label: Option<LabelId>,
    symbol_id: Option<SymbolId>,
    type_id: Option<TypeId>,
    state: SymbolState,
    binding: SymbolBinding,
    visibility: SymbolVisibility,
    provenance: SymbolProvenance,
    kind: SymbolKind,
    storage: StorageClass,
    runtime_addr: Option<usize>,
    file_addr: Option<usize>,
    size: Option<usize>,
    section: Option<LabelId>,
    info: SymbolInfo,
    byte_order: Endianness,
}

impl<'a> SymbolBuilder<'a> {
    pub(crate) fn new(table: &'a mut SymbolTable) -> Self {
        Self {
            table,
            label: None,
            symbol_id: None,
            type_id: None,
            state: SymbolState::default(),
            binding: SymbolBinding::default(),
            visibility: SymbolVisibility::default(),
            provenance: SymbolProvenance::default(),
            kind: SymbolKind::default(),
            storage: StorageClass::default(),
            runtime_addr: None,
            file_addr: None,
            size: None,
            section: None,
            info: SymbolInfo::default(),
            byte_order: Endianness::Little,
        }
    }

    pub fn label<S: AsRef<str>>(mut self, value: S) -> Self {
        let id = self.table.intern_label(value);
        self.label = Some(id);
        self
    }

    pub fn label_id(mut self, id: LabelId) -> Self {
        self.label = Some(id);
        self
    }

    pub fn symbol_id(mut self, id: SymbolId) -> Self {
        self.symbol_id = Some(id);
        self
    }

    pub fn type_id(mut self, id: TypeId) -> Self {
        self.type_id = Some(id);
        self
    }

    pub fn state(mut self, state: SymbolState) -> Self {
        self.state = state;
        self
    }

    pub fn binding(mut self, binding: SymbolBinding) -> Self {
        self.binding = binding;
        self
    }

    pub fn visibility(mut self, visibility: SymbolVisibility) -> Self {
        self.visibility = visibility;
        self
    }

    pub fn source(mut self, source: SymbolSource) -> Self {
        self.provenance.sources |= source;
        self
    }

    pub fn trust(mut self, trust: SourceTrust) -> Self {
        self.provenance.trust = trust;
        self
    }

    pub fn kind(mut self, kind: SymbolKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn storage(mut self, storage: StorageClass) -> Self {
        self.storage = storage;
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

    pub fn size(mut self, size: usize) -> Self {
        self.size = Some(size);
        self
    }

    pub fn section<S: AsRef<str>>(mut self, value: S) -> Self {
        let id = self.table.intern_label(value);
        self.section = Some(id);
        self
    }

    pub fn description<S: AsRef<str>>(mut self, value: S) -> Self {
        let id = self.table.intern_label(value);
        self.info.description = Some(id);
        self
    }

    pub fn units<S: AsRef<str>>(mut self, value: S) -> Self {
        let id = self.table.intern_label(value);
        self.info.units = Some(id);
        self
    }

    pub fn tool_flags(mut self, flags: ToolFlags) -> Self {
        self.info.tool_flags = flags;
        self
    }

    pub fn byte_order(mut self, order: Endianness) -> Self {
        self.byte_order = order;
        self
    }

    pub fn finish(self) -> SymbolHandle {
        let label = self
            .label
            .expect("symbol requires a label before finish() can be called");
        let mut record = SymbolRecord::new(label);
        record.symbol_id = self.symbol_id;
        record.type_id = self.type_id;
        record.state = self.state;
        record.binding = self.binding;
        record.visibility = self.visibility;
        record.provenance = self.provenance;
        record.kind = self.kind;
        record.storage = self.storage;
        record.runtime_addr = self.runtime_addr;
        record.file_addr = self.file_addr;
        record.size = self.size;
        record.section = self.section;
        record.info = self.info;
        record.byte_order = self.byte_order;
        self.table.commit(record)
    }
}

impl SymbolTable {
    pub fn builder(&mut self) -> SymbolBuilder<'_> {
        SymbolBuilder::new(self)
    }
}

#[cfg(test)]
mod tests {
    //! Builder API coverage to ensure fluent setters wire through to the SymbolRecord.
    use std::num::NonZeroU64;
    use std::sync::Arc;

    use crate::soc::device::endianness::Endianness;
    use crate::soc::prog::types::TypeArena;

    use super::*;
    use crate::soc::prog::symbols::source::SymbolSource;

    #[test]
    fn builder_assigns_common_fields() {
        let types = Arc::new(TypeArena::new());
        let mut table = SymbolTable::new(types);
        let handle = table
            .builder()
            .label("ENGINE_SPEED")
            .symbol_id(SymbolId::new(NonZeroU64::new(42).unwrap()))
            .binding(SymbolBinding::Global)
            .source(SymbolSource::ELF)
            .runtime_addr(0x4000_0000)
            .file_addr(0x0)
            .size(4)
            .description("calibrated")
            .finish();
        let record = table.get(handle);
        assert_eq!(
            table.resolve_label(record.label),
            "ENGINE_SPEED",
            "Label interning should keep the original spelling"
        );
        assert_eq!(
            record.binding,
            SymbolBinding::Global,
            "Builder should wire binding into the record"
        );
        assert_eq!(
            record.runtime_addr,
            Some(0x4000_0000),
            "Runtime address should persist onto the record"
        );
        assert!(
            record.provenance.sources.contains(SymbolSource::ELF),
            "Source flags need to track ingestion provenance"
        );
    }

    #[test]
    fn builder_handles_section_and_units_strings() {
        let types = Arc::new(TypeArena::new());
        let mut table = SymbolTable::new(types);
        let handle = table
            .builder()
            .label("OFFSET_TABLE")
            .section(".text")
            .units("rpm")
            .byte_order(Endianness::Big)
            .finish();
        let record = table.get(handle);
        assert_eq!(
            table.resolve_label(record.section.unwrap()),
            ".text",
            "Section labels should resolve to the provided name"
        );
        assert_eq!(
            table.resolve_label(record.info.units.unwrap()),
            "rpm",
            "Units text needs to survive the builder flow"
        );
        assert_eq!(
            record.byte_order,
            Endianness::Big,
            "Byte order overrides must be honored"
        );
    }
}
