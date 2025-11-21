use super::Validator;
use crate::soc::isa::ast::{FormDecl, SpaceKind};
use crate::soc::isa::logic::LogicFormError;

impl Validator {
    pub(super) fn validate_form(&mut self, form: &FormDecl) {
        match self.space_kinds.get(&form.space) {
            Some(SpaceKind::Logic) => {}
            Some(_) => {
                self.push_validation_diagnostic(
                    "validation.logic.form-space",
                    format!(
                        "form '{}' can only be declared inside logic spaces",
                        form.name
                    ),
                    Some(form.span.clone()),
                );
                return;
            }
            None => {
                self.push_validation_diagnostic(
                    "validation.logic.form-space",
                    format!(
                        "form '{}' declared in unknown space '{}'",
                        form.name, form.space
                    ),
                    Some(form.span.clone()),
                );
                return;
            }
        }

        let Some(state) = self.logic_state(&form.space) else {
            self.push_validation_diagnostic(
                "validation.logic.form-space",
                format!("logic space '{}' has no state", form.space),
                Some(form.span.clone()),
            );
            return;
        };

        match state.register_form(form) {
            Ok(()) => {}
            Err(LogicFormError::DuplicateForm { name }) => self.push_validation_diagnostic(
                "validation.logic.form-duplicate",
                format!("form '{}' declared multiple times", name),
                Some(form.span.clone()),
            ),
            Err(LogicFormError::MissingSubfields { name }) => self.push_validation_diagnostic(
                "validation.logic.form-empty",
                format!("form '{}' must declare at least one subfield", name),
                Some(form.span.clone()),
            ),
            Err(LogicFormError::MissingParent { parent }) => self.push_validation_diagnostic(
                "validation.logic.form-parent",
                format!(
                    "parent form '{}' must be declared before it can be extended",
                    parent
                ),
                Some(form.span.clone()),
            ),
            Err(LogicFormError::DuplicateSubfield { name }) => self.push_validation_diagnostic(
                "validation.logic.form-subfield-duplicate",
                format!(
                    "subfield '{}' already exists on inherited form; duplicates not allowed",
                    name
                ),
                Some(form.span.clone()),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use crate::soc::isa::ast::{SpaceAttribute, SpaceKind};

    #[test]
    fn logic_form_requires_parent() {
        let err = validate_src(
            ":space logic addr=32 word=32 type=logic\n:logic::UNKNOWN child subfields={\n    OPCD @(0..5)\n}",
        )
        .unwrap_err();
        expect_validation_diag(err, "parent form 'UNKNOWN'");
    }

    #[test]
    fn logic_form_duplicate_definition() {
        let err = validate_src(
            ":space logic addr=32 word=32 type=logic\n:logic FORM subfields={\n    OPCD @(0..5)\n}\n:logic FORM subfields={\n    OPCD @(0..5)\n}",
        )
        .unwrap_err();
        expect_validation_diag(err, "declared multiple times");
    }

    #[test]
    fn logic_form_inheritance_duplicate_subfield() {
        let err = validate_src(
            ":space logic addr=32 word=32 type=logic\n:logic BASE subfields={\n    OPCD @(0..5)\n}\n:logic::BASE EXT subfields={\n    OPCD @(6..10)\n}",
        )
        .unwrap_err();
        expect_validation_diag(err, "subfield 'OPCD' already exists");
    }

    #[test]
    fn logic_form_requires_subfield_entries() {
        let err = validate_src(":space logic addr=32 word=32 type=logic\n:logic FORM subfields={}")
            .unwrap_err();
        expect_validation_diag(err, "must declare at least one subfield");
    }

    #[test]
    fn logic_form_rejects_non_logic_space() {
        let err = validate_items(vec![
            space_decl(
                "reg",
                SpaceKind::Register,
                vec![
                    SpaceAttribute::AddressBits(32),
                    SpaceAttribute::WordSize(32),
                ],
            ),
            logic_form("reg", "FORM"),
        ])
        .unwrap_err();
        expect_validation_diag(err, "form 'FORM' can only be declared inside logic spaces");
    }
}
