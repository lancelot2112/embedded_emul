use super::{literal_bit_width, max_value_for_width, Validator};
use crate::soc::isa::ast::{InstructionDecl, MaskField, MaskSelector, SpaceKind, SubFieldDecl};
use crate::soc::isa::machine::parse_bit_spec;

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
        let form_fields = form_info.subfields.clone();

        let mut unknown_fields = Vec::new();
        if let Some(mask) = &instr.mask {
            let word_bits = self.logic_sizes.get(&instr.space).copied();
            for field in &mask.fields {
                match &field.selector {
                    MaskSelector::Field(name) => {
                        if let Some(subfield) = form_fields.get(name) {
                            if let Some(bits) = word_bits {
                                self.ensure_mask_value_within_field(
                                    instr,
                                    name,
                                    subfield,
                                    field,
                                    bits,
                                );
                            }
                        } else {
                            unknown_fields.push(name.clone());
                        }
                    }
                    MaskSelector::BitExpr(spec) => {
                        if let Some(bits) = word_bits {
                            self.ensure_mask_value_within_selector(instr, spec, field, bits);
                        }
                    }
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

    fn ensure_mask_value_within_field(
        &mut self,
        instr: &InstructionDecl,
        field_name: &str,
        subfield: &SubFieldDecl,
        mask_field: &MaskField,
        word_bits: u32,
    ) {
        let Ok(spec) = parse_bit_spec(word_bits, &subfield.bit_spec) else {
            return;
        };
        let width = spec.data_width() as u32;
        let context = match instr.form.as_ref() {
            Some(form) => format!("field '{field_name}' on form '{form}'"),
            None => format!("field '{field_name}'"),
        };
        self.maybe_report_mask_literal(instr, mask_field, width, context);
    }

    fn ensure_mask_value_within_selector(
        &mut self,
        instr: &InstructionDecl,
        spec: &str,
        mask_field: &MaskField,
        word_bits: u32,
    ) {
        let Ok(parsed) = parse_bit_spec(word_bits, spec) else {
            return;
        };
        let width = parsed.data_width() as u32;
        let context = format!("selector '{spec}'");
        self.maybe_report_mask_literal(instr, mask_field, width, context);
    }

    fn maybe_report_mask_literal(
        &mut self,
        instr: &InstructionDecl,
        mask_field: &MaskField,
        width: u32,
        context: String,
    ) {
        if width == 0 {
            return;
        }
        let max_value = max_value_for_width(width);
        if mask_field.value <= max_value {
            return;
        }
        let literal_bits = literal_bit_width(mask_field.value);
        let literal = mask_field
            .value_text
            .as_ref()
            .cloned()
            .unwrap_or_else(|| mask_field.value.to_string());
        let span = mask_field
            .value_span
            .clone()
            .or_else(|| Some(instr.span.clone()));
        self.push_validation_diagnostic(
            "validation.mask.literal-width",
            format!(
                "literal '{}' requires {literal_bits} bit(s) but {context} spans only {width} bit(s) for instruction '{}'",
                literal, instr.name
            ),
            span,
        );
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

    #[test]
    fn mask_literal_respects_field_width() {
        let err = validate_src(
            ":space logic addr=32 word=16 type=logic\n:logic FORM subfields={\n    LK @(7) op=func\n}\n:logic::FORM jump mask={LK=2}",
        )
        .unwrap_err();
        expect_validation_diag(err, "literal '2' requires");
    }
}
