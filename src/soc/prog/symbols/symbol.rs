//! Core symbol data structures shared across the subsystem.

use bitflags::bitflags;

use crate::soc::device::endianness::Endianness;
use crate::soc::prog::types::TypeId;

use super::id::{LabelId, SymbolId};
use super::source::SymbolProvenance;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SymbolState {
    #[default]
    Declared,
    Defined,
    Imported,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SymbolKind {
    #[default]
    Object,
    Function,
    Section,
    Metadata,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SymbolBinding {
    #[default]
    Local,
    Global,
    Weak,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SymbolVisibility {
    #[default]
    Default,
    Hidden,
    Protected,
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct StorageClass: u16 {
        const ROM = 0b0001;
        const RAM = 0b0010;
        const METADATA = 0b0100;
        const RUNTIME_ONLY = 0b1000;
        const OFFLINE_ACCESSIBLE = 0b1_0000;
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct ToolFlags: u16 {
        const CALIBRATABLE = 0b0001;
        const DERIVED = 0b0010;
        const DIAGNOSTIC = 0b0100;
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SymbolInfo {
    pub description: Option<LabelId>,
    pub units: Option<LabelId>,
    pub tool_flags: ToolFlags,
    pub index_table: Option<u32>,
}

impl SymbolInfo {
    pub fn calibratable(index_table: u32) -> Self {
        Self {
            index_table: Some(index_table),
            tool_flags: ToolFlags::CALIBRATABLE,
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolRecord {
    pub label: LabelId,
    pub symbol_id: Option<SymbolId>,
    pub type_id: Option<TypeId>,
    pub state: SymbolState,
    pub binding: SymbolBinding,
    pub visibility: SymbolVisibility,
    pub provenance: SymbolProvenance,
    pub kind: SymbolKind,
    pub storage: StorageClass,
    pub runtime_addr: Option<usize>,
    pub file_addr: Option<usize>,
    pub size: Option<usize>,
    pub section: Option<LabelId>,
    pub info: SymbolInfo,
    pub byte_order: Endianness,
}

impl SymbolRecord {
    pub fn new(label: LabelId) -> Self {
        Self {
            label,
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
}

#[cfg(test)]
mod tests {
    //! Verifies that defaults stay aligned with the documented architecture.
    use super::*;
    use crate::soc::prog::symbols::id::LabelId;

    #[test]
    fn record_defaults_match_architecture() {
        let label = LabelId::from_index(0);
        let record = SymbolRecord::new(label);
        assert_eq!(
            record.state,
            SymbolState::Declared,
            "New symbols should start as declarations"
        );
        assert_eq!(
            record.binding,
            SymbolBinding::Local,
            "Locals should be the default binding"
        );
        assert_eq!(
            record.visibility,
            SymbolVisibility::Default,
            "Default visibility keeps discovery simple"
        );
        assert_eq!(
            record.kind,
            SymbolKind::Object,
            "Records default to objects until loaders override them"
        );
    }

    #[test]
    fn symbol_info_builder_sets_flags() {
        let info = SymbolInfo::calibratable(3);
        assert!(
            info.tool_flags.contains(ToolFlags::CALIBRATABLE),
            "Calibratable helper should apply the calibratable flag"
        );
        assert_eq!(
            info.index_table,
            Some(3),
            "Helper should store the provided index table id"
        );
    }
}
