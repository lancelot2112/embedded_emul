//! Bitflag helpers describing where a symbol originated from and how trustworthy it is.

use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct SymbolSource: u32 {
        const ELF = 0b0001;
        const DWARF = 0b0010;
        const A2L = 0b0100;
        const TOOL = 0b1000;
        const MANUAL = 0b1_0000;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceTrust {
    #[default]
    Unknown,
    Trusted,
    Suspicious,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SymbolProvenance {
    pub sources: SymbolSource,
    pub trust: SourceTrust,
}

impl SymbolProvenance {
    pub fn from_source(source: SymbolSource) -> Self {
        Self {
            sources: source,
            trust: SourceTrust::Unknown,
        }
    }

    pub fn with_trust(mut self, trust: SourceTrust) -> Self {
        self.trust = trust;
        self
    }

    pub fn merge(self, other: SymbolProvenance) -> Self {
        let trust = match (self.trust, other.trust) {
            (SourceTrust::Suspicious, _) | (_, SourceTrust::Suspicious) => SourceTrust::Suspicious,
            (SourceTrust::Trusted, SourceTrust::Trusted) => SourceTrust::Trusted,
            (SourceTrust::Trusted, SourceTrust::Unknown)
            | (SourceTrust::Unknown, SourceTrust::Trusted) => SourceTrust::Trusted,
            _ => SourceTrust::Unknown,
        };
        Self {
            sources: self.sources | other.sources,
            trust,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Sanity checks for provenance arithmetic so loaders can rely on deterministic merges.
    use super::*;

    #[test]
    fn provenance_accumulates_sources() {
        let elf = SymbolProvenance::from_source(SymbolSource::ELF);
        let tool =
            SymbolProvenance::from_source(SymbolSource::TOOL).with_trust(SourceTrust::Trusted);
        let merged = elf.merge(tool);
        assert!(
            merged.sources.contains(SymbolSource::ELF),
            "ELF bit should be preserved when merging"
        );
        assert!(
            merged.sources.contains(SymbolSource::TOOL),
            "TOOL bit should be added when merging sources"
        );
        assert_eq!(
            merged.trust,
            SourceTrust::Trusted,
            "Trusted trust level should dominate Unknown"
        );
    }

    #[test]
    fn suspicious_trust_is_sticky() {
        let base = SymbolProvenance::from_source(SymbolSource::A2L);
        let suspicious =
            SymbolProvenance::from_source(SymbolSource::MANUAL).with_trust(SourceTrust::Suspicious);
        let merged = base.merge(suspicious);
        assert_eq!(
            merged.trust,
            SourceTrust::Suspicious,
            "Any suspicious contributor should mark the combined symbol as suspicious"
        );
    }
}
