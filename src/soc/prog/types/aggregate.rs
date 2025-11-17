//! Aggregate type description for structs, unions, classes, and tagged variants.

use smallvec::SmallVec;

use super::arena::StringId;
use super::record::{LayoutSize, MemberSpan};

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
    pub members: MemberSpan,
    pub static_members: SmallVec<[StaticMember; 2]>,
    pub byte_size: LayoutSize,
    pub has_dynamic: bool,
}

impl AggregateType {
    pub fn new(kind: AggregateKind, members: MemberSpan, byte_size: LayoutSize) -> Self {
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
        self.static_members.push(StaticMember { label, variable_id });
    }
}

#[cfg(test)]
mod tests {
    //! Ensures aggregate metadata mirrors the intended layout semantics.
    use super::*;

    #[test]
    fn unions_report_helpers() {
        // verifying simple union detection logic for walker heuristics
        let span = MemberSpan::new(0, 0);
        let agg = AggregateType::new(AggregateKind::Union, span, LayoutSize::ZERO);
        assert!(agg.is_union(), "AggregateKind::Union must report true from is_union");
    }
}
