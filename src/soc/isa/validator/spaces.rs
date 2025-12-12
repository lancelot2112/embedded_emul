use super::{literal_bit_width, max_value_for_width, Validator};
use crate::soc::isa::ast::{SpaceAttribute, SpaceDecl, SpaceKind};
use crate::soc::isa::logic::LogicSpaceState;
use crate::soc::isa::machine::parse_bit_spec;
use crate::soc::isa::semantics::{BinaryOperator, SemanticExpr};

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
        if let Some(bits) = word_size(space) {
            self.space_word_sizes.insert(space.name.clone(), bits);
        }
        let mut logic_bits = None;
        if matches!(space.kind, SpaceKind::Logic) {
            if let Some(word) = self.space_word_sizes.get(&space.name).copied() {
                self.logic_states
                    .entry(space.name.clone())
                    .or_insert_with(|| LogicSpaceState::new(word));
                self.logic_sizes.insert(space.name.clone(), word);
                logic_bits = Some(word);
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
        if matches!(space.kind, SpaceKind::Logic) {
            if let (Some(expr), Some(bits)) = (space.enable.as_ref(), logic_bits) {
                self.validate_enable_expression(&space.name, expr, bits);
            }
        }
        self.space_states.entry(space.name.clone()).or_default();
    }

    fn validate_enable_expression(
        &mut self,
        space_name: &str,
        expr: &SemanticExpr,
        word_bits: u32,
    ) {
        match expr {
            SemanticExpr::BitExpr {
                spec,
                span,
                bit_order,
            } => {
                if let Err(err) = parse_bit_spec(word_bits, spec, *bit_order) {
                    self.push_validation_diagnostic(
                        "validation.enable.bit-range",
                        format!(
                            "bit selector '{}' exceeds {}-bit word for logic space '{}': {err}",
                            spec, word_bits, space_name
                        ),
                        Some(span.clone()),
                    );
                }
            }
            SemanticExpr::BinaryOp { op, lhs, rhs } => {
                if matches!(op, BinaryOperator::Eq | BinaryOperator::Ne) {
                    self.validate_enable_literal_width(space_name, lhs, rhs, word_bits);
                }
                self.validate_enable_expression(space_name, lhs, word_bits);
                self.validate_enable_expression(space_name, rhs, word_bits);
            }
            _ => {}
        }
    }

    fn validate_enable_literal_width(
        &mut self,
        space_name: &str,
        lhs: &SemanticExpr,
        rhs: &SemanticExpr,
        word_bits: u32,
    ) {
        match (lhs, rhs) {
            (
                SemanticExpr::BitExpr { spec, bit_order, .. },
                SemanticExpr::Literal { value, text, span },
            )
            | (
                SemanticExpr::Literal { value, text, span },
                SemanticExpr::BitExpr { spec, bit_order, .. },
            ) => {
                let Ok(parsed) = parse_bit_spec(word_bits, spec, *bit_order) else {
                    return;
                };
                let width = parsed.data_width() as u32;
                if width == 0 {
                    return;
                }
                let max_value = max_value_for_width(width);
                if *value > max_value {
                    let bits_needed = literal_bit_width(*value);
                    self.push_validation_diagnostic(
                        "validation.enable.literal-width",
                        format!(
                            "literal '{}' requires {bits_needed} bit(s) but selector '{}' spans only {width} bit(s) in logic space '{}'",
                            text, spec, space_name
                        ),
                        Some(span.clone()),
                    );
                }
            }
            _ => {}
        }
    }
}

fn word_size(space: &SpaceDecl) -> Option<u32> {
    space.attributes.iter().find_map(|attr| match attr {
        SpaceAttribute::WordSize(bits) => Some(*bits),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use crate::soc::isa::ast::{IsaItem, SpaceAttribute, SpaceDecl, SpaceKind};
    use crate::soc::isa::error::IsaError;
    use crate::soc::isa::semantics::{BinaryOperator, SemanticExpr};
    use crate::soc::prog::types::bitfield::BitOrder;

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

    #[test]
    fn enable_bit_expr_respects_word_size() {
        let span = manual_span();
        let space = IsaItem::Space(SpaceDecl {
            name: "vle".into(),
            kind: SpaceKind::Logic,
            attributes: vec![SpaceAttribute::WordSize(16)],
            span: span.clone(),
            enable: Some(SemanticExpr::BitExpr {
                spec: "@(0|16)".into(),
                span: span.clone(),
                bit_order: BitOrder::Msb0,
            }),
        });
        let err = validate_items(vec![space]).expect_err("bit expr should fail");
        match err {
            IsaError::Diagnostics { diagnostics, .. } => {
                assert!(diagnostics
                    .iter()
                    .any(|diag| diag.code == "validation.enable.bit-range"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn enable_literal_respects_selector_width() {
        let span = manual_span();
        let space = IsaItem::Space(SpaceDecl {
            name: "vle".into(),
            kind: SpaceKind::Logic,
            attributes: vec![
                SpaceAttribute::WordSize(16),
                SpaceAttribute::AddressBits(16),
            ],
            span: span.clone(),
            enable: Some(SemanticExpr::BinaryOp {
                op: BinaryOperator::Eq,
                lhs: Box::new(SemanticExpr::BitExpr {
                    spec: "@(0|3)".into(),
                    span: span.clone(),
                    bit_order: BitOrder::Msb0,
                }),
                rhs: Box::new(SemanticExpr::Literal {
                    value: 0b110,
                    text: "0b110".into(),
                    span: span.clone(),
                }),
            }),
        });
        let err = validate_items(vec![space]).expect_err("literal width should fail");
        match err {
            IsaError::Diagnostics { diagnostics, .. } => {
                assert!(diagnostics
                    .iter()
                    .any(|diag| diag.code == "validation.enable.literal-width"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
