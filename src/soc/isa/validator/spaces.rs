use super::Validator;
use crate::soc::isa::ast::{SpaceAttribute, SpaceDecl, SpaceKind};
use crate::soc::isa::logic::LogicSpaceState;

impl Validator {
    pub(super) fn validate_space(&mut self, space: &SpaceDecl) {
        if !self.seen_spaces.insert(space.name.clone()) {
            self.push_validation_diagnostic(
                "validation.duplicate-space",
                format!("space '{}' defined multiple times", space.name),
                Some(space.span.clone()),
            );
            return;
        }
        self.space_kinds
            .insert(space.name.clone(), space.kind.clone());
        if matches!(space.kind, SpaceKind::Logic) {
            if let Some(word) = logic_word_size(space) {
                self.logic_states
                    .entry(space.name.clone())
                    .or_insert_with(|| LogicSpaceState::new(word));
                self.logic_sizes.insert(space.name.clone(), word);
            } else {
                self.push_validation_diagnostic(
                    "validation.logic.word-size",
                    format!("logic space '{}' missing word size", space.name),
                    Some(space.span.clone()),
                );
            }
            if space.enable.is_some() {
                self.space_enables.insert(space.name.clone());
            }
        }
        if !matches!(space.kind, SpaceKind::Logic) && space.enable.is_some() {
            self.push_validation_diagnostic(
                "validation.enable.logic-only",
                format!(
                    "space '{}' declares enbl expression but only logic spaces support it",
                    space.name
                ),
                Some(space.span.clone()),
            );
        }
        self.space_states
            .entry(space.name.clone())
            .or_default();
    }
}

fn logic_word_size(space: &SpaceDecl) -> Option<u32> {
    space.attributes.iter().find_map(|attr| match attr {
        SpaceAttribute::WordSize(bits) => Some(*bits),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use crate::soc::isa::ast::{SpaceAttribute, SpaceKind};
    use crate::soc::isa::error::IsaError;

    #[test]
    fn logic_space_missing_word_size_reports_all_errors() {
        let err = validate_items(vec![
            space_decl(
                "logic",
                SpaceKind::Logic,
                vec![SpaceAttribute::AddressBits(32)],
            ),
            logic_form("logic", "FORM"),
        ])
        .unwrap_err();
        match err {
            IsaError::Diagnostics { diagnostics, .. } => {
                assert!(
                    diagnostics
                        .iter()
                        .any(|diag| diag.message.contains("missing word size"))
                );
                assert!(
                    diagnostics
                        .iter()
                        .any(|diag| diag.message.contains("has no state"))
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
