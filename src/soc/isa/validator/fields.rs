use super::Validator;
use crate::soc::isa::ast::{ContextReference, FieldDecl};
use crate::soc::isa::register::FieldRegistrationError;
use crate::soc::isa::space::resolve_reference_path;

impl Validator {
    pub(super) fn validate_field(&mut self, field: &FieldDecl) {
        if let Some(reference) = &field.redirect {
            self.ensure_redirect_target_defined(field, reference);
        }

        let Some(state) = self.space_states.get_mut(&field.space) else {
            self.push_validation_diagnostic(
                "validation.unknown-space-field",
                format!(
                    "field '{}' declared in unknown space '{}'",
                    field.name, field.space
                ),
                Some(field.span.clone()),
            );
            return;
        };

        match state.register_field(field) {
            Ok(()) => {}
            Err(FieldRegistrationError::DuplicateField { name }) => {
                self.push_validation_diagnostic(
                    "validation.duplicate-field",
                    format!("field '{}' declared multiple times", name),
                    Some(field.span.clone()),
                );
            }
            Err(FieldRegistrationError::MissingBaseField { name }) => {
                self.push_validation_diagnostic(
                    "validation.field.append-missing",
                    format!("cannot append subfields to undefined field '{}'", name),
                    Some(field.span.clone()),
                );
            }
            Err(FieldRegistrationError::EmptySubfieldAppend { name }) => {
                self.push_validation_diagnostic(
                    "validation.field.append-empty",
                    format!(
                        "field '{}' subfield-only declaration must list subfields",
                        name
                    ),
                    Some(field.span.clone()),
                );
            }
        }
    }

    fn ensure_redirect_target_defined(&mut self, field: &FieldDecl, reference: &ContextReference) {
        let (target_space, mut path) = resolve_reference_path(&field.space, reference);
        if path.is_empty() {
            self.push_validation_diagnostic(
                "validation.redirect.missing-field",
                "redirect requires a field name in its context reference",
                Some(field.span.clone()),
            );
            return;
        }
        let field_name = path.remove(0);
        let Some(space_state) = self.space_states.get(&target_space) else {
            self.push_validation_diagnostic(
                "validation.redirect.unknown-space",
                format!("redirect references undefined space '{}'", target_space),
                Some(field.span.clone()),
            );
            return;
        };
        let Some(field_info) = space_state.lookup_field(&field_name) else {
            self.push_validation_diagnostic(
                "validation.redirect.unknown-field",
                format!(
                    "redirect references undefined field '{}' in space '{}'",
                    field_name, target_space
                ),
                Some(field.span.clone()),
            );
            return;
        };
        if let Some(subfield_name) = path.first() {
            if !field_info.has_subfield(subfield_name) {
                self.push_validation_diagnostic(
                    "validation.redirect.unknown-subfield",
                    format!(
                        "redirect references undefined subfield '{}' on field '{}'",
                        subfield_name, field_name
                    ),
                    Some(field.span.clone()),
                );
                return;
            }
            if path.len() > 1 {
                self.push_validation_diagnostic(
                    "validation.redirect.depth",
                    "redirect context depth exceeds field::subfield",
                    Some(field.span.clone()),
                );
            }
        } else if !path.is_empty() {
            self.push_validation_diagnostic(
                "validation.redirect.depth",
                "redirect context depth exceeds field::subfield",
                Some(field.span.clone()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use crate::soc::isa::error::IsaError;

    #[test]
    fn redirect_requires_prior_definition_in_same_space() {
        let err = validate_src(
            ":space reg addr=32 word=64 type=register\n:reg alias redirect=PC\n:reg PC size=64",
        )
        .unwrap_err();
        expect_validation_diag(err, "undefined field 'PC'");
    }

    #[test]
    fn redirect_accepts_prior_definition() {
        validate_src(
            ":space reg addr=32 word=64 type=register\n:reg PC size=64\n:reg alias redirect=PC",
        )
        .expect("validation succeeds");
    }

    #[test]
    fn redirect_supports_cross_space_reference() {
        validate_src(
            ":space reg addr=32 word=64 type=register\n:reg PC size=64\n:space aux addr=32 word=64 type=register\n:aux backup redirect=$reg::PC",
        )
        .expect("cross space redirect succeeds");
    }

    #[test]
    fn redirect_errors_on_unknown_subfield() {
        let err = validate_src(
            ":space reg addr=32 word=64 type=register\n:reg PC size=64 subfields={\n    LSB @(0)\n}\n:reg alias redirect=PC::MSB",
        )
        .unwrap_err();
        expect_validation_diag(err, "undefined subfield 'MSB'");
    }

    #[test]
    fn validator_collects_multiple_errors() {
        let err = validate_src(
            ":space reg addr=32 word=64 type=register\n:reg alias redirect=PC\n:reg R0 size=64\n:reg R0 size=64",
        )
        .unwrap_err();
        match err {
            IsaError::Diagnostics { diagnostics, .. } => {
                assert!(
                    diagnostics.len() >= 2,
                    "expected multiple diagnostics: {diagnostics:?}"
                );
                assert!(
                    diagnostics
                        .iter()
                        .any(|diag| diag.message.contains("undefined field 'PC'")),
                    "missing redirect diagnostic: {diagnostics:?}"
                );
                assert!(
                    diagnostics
                        .iter()
                        .any(|diag| diag.message.contains("field 'R0' declared multiple times")),
                    "missing duplicate field diagnostic: {diagnostics:?}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn redirect_accepts_range_element() {
        validate_src(
            ":space reg addr=32 word=64 type=register\n:reg GPR[0..1] size=64\n:reg alias redirect=GPR1",
        )
        .expect("redirect to ranged element succeeds");
    }

    #[test]
    fn subfield_append_extends_existing_field() {
        validate_src(
            ":space reg addr=32 word=64 type=register\n:reg R0 size=64 subfields={\n    LSB @(0)\n}\n:reg R0 subfields={\n    MSB @(63)\n}",
        )
        .expect("subfield append succeeds");
    }

    #[test]
    fn subfield_append_requires_existing_base() {
        let err = validate_src(
            ":space reg addr=32 word=64 type=register\n:reg R0 subfields={\n    EXTRA @(0)\n}",
        )
        .unwrap_err();
        expect_validation_diag(err, "cannot append subfields to undefined field");
    }
}
