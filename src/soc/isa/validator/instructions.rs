use super::Validator;
use crate::soc::isa::ast::{InstructionDecl, MaskSelector, SpaceKind};

impl Validator {
    pub(super) fn validate_instruction(&mut self, instr: &InstructionDecl) {
        match self.space_kinds.get(&instr.space) {
            Some(SpaceKind::Logic) => {}
            Some(_) => {
                self.push_validation_diagnostic(
                    "validation.logic.instruction-space",
                    format!(
                        "instruction '{}' can only be declared inside logic spaces",
                        instr.name
                    ),
                    Some(instr.span.clone()),
                );
                return;
            }
            None => {
                self.push_validation_diagnostic(
                    "validation.logic.instruction-space",
                    format!(
                        "instruction '{}' declared in unknown space '{}'",
                        instr.name, instr.space
                    ),
                    Some(instr.span.clone()),
                );
                return;
            }
        }

        let Some(state) = self.logic_states.get(&instr.space) else {
            self.push_validation_diagnostic(
                "validation.logic.instruction-space",
                format!("logic space '{}' has no form state", instr.space),
                Some(instr.span.clone()),
            );
            return;
        };

        let Some(form_name) = &instr.form else {
            self.push_validation_diagnostic(
                "validation.logic.instruction-form-missing",
                format!(
                    "instruction '{}' must reference a form using '::<form>'",
                    instr.name
                ),
                Some(instr.span.clone()),
            );
            return;
        };

        let Some(form_info) = state.form(form_name) else {
            self.push_validation_diagnostic(
                "validation.logic.instruction-form",
                format!(
                    "instruction '{}' references undefined form '{}'",
                    instr.name, form_name
                ),
                Some(instr.span.clone()),
            );
            return;
        };

        let mut unknown_fields = Vec::new();
        if let Some(mask) = &instr.mask {
            for field in &mask.fields {
                if let MaskSelector::Field(name) = &field.selector
                    && !form_info.subfields.contains_key(name)
                {
                    unknown_fields.push(name.clone());
                }
            }
        }
        for name in unknown_fields {
            self.push_validation_diagnostic(
                "validation.logic.mask-field",
                format!(
                    "mask references unknown field '{}' for instruction '{}'",
                    name, instr.name
                ),
                Some(instr.span.clone()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use crate::soc::isa::ast::{SpaceAttribute, SpaceKind};

    #[test]
    fn logic_instruction_requires_existing_form() {
        let err = validate_src(
            ":space logic addr=32 word=32 type=logic\n:logic FORM subfields={\n    OPCD @(0..5)\n}\n:logic::UNKNOWN add mask={OPCD=31}",
        )
        .unwrap_err();
        expect_validation_diag(err, "references undefined form");
    }

    #[test]
    fn logic_mask_requires_known_field() {
        let err = validate_src(
            ":space logic addr=32 word=32 type=logic\n:logic FORM subfields={\n    OPCD @(0..5)\n}\n:logic::FORM add mask={XYZ=1}",
        )
        .unwrap_err();
        expect_validation_diag(err, "mask references unknown field");
    }

    #[test]
    fn logic_instruction_rejects_non_logic_space() {
        let err = validate_items(vec![
            space_decl(
                "reg",
                SpaceKind::Register,
                vec![
                    SpaceAttribute::AddressBits(32),
                    SpaceAttribute::WordSize(32),
                ],
            ),
            logic_instruction("reg", Some("FORM"), "add"),
        ])
        .unwrap_err();
        expect_validation_diag(
            err,
            "instruction 'add' can only be declared inside logic spaces",
        );
    }

    #[test]
    fn logic_instruction_requires_form_reference() {
        let err = validate_items(vec![
            space_decl(
                "logic",
                SpaceKind::Logic,
                vec![
                    SpaceAttribute::AddressBits(32),
                    SpaceAttribute::WordSize(32),
                ],
            ),
            logic_instruction("logic", None, "add"),
        ])
        .unwrap_err();
        expect_validation_diag(err, "must reference a form");
    }

    #[test]
    fn logic_instruction_accepts_inherited_fields() {
        validate_src(
            ":space logic addr=32 word=32 type=logic\n:logic BASE subfields={\n    OPCD @(0..5) op=func\n}\n:logic::BASE EXT subfields={\n    RT @(6..10) op=target\n}\n:logic::EXT add mask={OPCD=31}",
        )
        .expect("logic instruction referencing inherited form fields should validate");
    }
}
