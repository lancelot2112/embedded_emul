use super::Validator;
use crate::soc::isa::ast::{ContextReference, FieldDecl};
use crate::soc::isa::machine::parse_bit_spec;
use crate::soc::isa::register::FieldRegistrationError;
use crate::soc::isa::space::resolve_reference_path;

impl Validator {
    pub(super) fn validate_field(&mut self, field: &FieldDecl) {
        if let Some(reference) = &field.redirect {
            self.ensure_redirect_target_defined(field, reference);
        }

        self.ensure_subfield_within_bounds(field);

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

    fn ensure_subfield_within_bounds(&mut self, field: &FieldDecl) {
        let Some(size_bits) = field
            .size
            .or_else(|| self.space_word_sizes.get(&field.space).copied())
        else {
            return;
        };
        for subfield in &field.subfields {
            if let Err(err) = parse_bit_spec(size_bits, &subfield.bit_spec) {
                let span = subfield
                    .bit_spec_span
                    .clone()
                    .or_else(|| Some(field.span.clone()));
                self.push_validation_diagnostic(
                    "validation.subfield.bit-range",
                    format!(
                        "subfield '{}' bit spec {} exceeds {}-bit field '{}': {err}",
                        subfield.name, subfield.bit_spec, size_bits, field.name
                    ),
                    span,
                );
            }
        }
    }

    fn ensure_redirect_target_defined(&mut self, field: &FieldDecl, reference: &ContextReference) {
        let (target_space, mut path) = resolve_reference_path(&field.space, reference);
        let skip_segments = if reference
            .segments
            .first()
            .map(|segment| segment.starts_with('$'))
            .unwrap_or(false)
        {
            1
        } else {
            0
        };
        let mut path_spans: Vec<_> = reference
            .segment_spans
            .iter()
            .skip(skip_segments)
            .cloned()
            .collect();
        if path.is_empty() {
            self.push_validation_diagnostic(
                "validation.redirect.missing-field",
                "redirect requires a field name in its context reference",
                Some(reference.span.clone()),
            );
            return;
        }
        let field_span = path_spans
            .get(0)
            .cloned()
            .unwrap_or_else(|| reference.span.clone());
        let field_name = path.remove(0);
        if !path_spans.is_empty() {
            path_spans.remove(0);
        }
        let Some(space_state) = self.space_states.get(&target_space) else {
            let span = reference
                .segment_spans
                .first()
                .cloned()
                .unwrap_or_else(|| reference.span.clone());
            self.push_validation_diagnostic(
                "validation.redirect.unknown-space",
                format!("redirect references undefined space '{}'", target_space),
                Some(span),
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
                Some(field_span.clone()),
            );
            return;
        };
        if let Some(subfield_name) = path.first() {
            let subfield_span = path_spans
                .get(0)
                .cloned()
                .unwrap_or_else(|| reference.span.clone());
            if !field_info.has_subfield(subfield_name) {
                self.push_validation_diagnostic(
                    "validation.redirect.unknown-subfield",
                    format!(
                        "redirect references undefined subfield '{}' on field '{}'",
                        subfield_name, field_name
                    ),
                    Some(subfield_span),
                );
                return;
            }
            if path.len() > 1 {
                let extra_span = path_spans
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| reference.span.clone());
                self.push_validation_diagnostic(
                    "validation.redirect.depth",
                    "redirect context depth exceeds field::subfield",
                    Some(extra_span),
                );
            }
        } else if !path.is_empty() {
            let span = path_spans
                .first()
                .cloned()
                .unwrap_or_else(|| reference.span.clone());
            self.push_validation_diagnostic(
                "validation.redirect.depth",
                "redirect context depth exceeds field::subfield",
                Some(span),
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
    fn subfield_bit_spec_respects_field_size() {
        let source = ":space reg addr=32 word=64 type=register\n:reg MSR size=64 subfields={\n    CM @(32)\n    RI @(65)\n}";
        let err = validate_src(source).expect_err("expected bit range failure");
        let diagnostics = match err {
            IsaError::Diagnostics { diagnostics, .. } => diagnostics,
            other => panic!("unexpected error: {other:?}"),
        };
        let diag = diagnostics
            .iter()
            .find(|diag| diag.code == "validation.subfield.bit-range")
            .expect("missing bit range diagnostic");
        let span = diag.span.as_ref().expect("diagnostic span");
        assert_eq!(snippet_from_span(source, span), "@(65)");
    }

    #[test]
    fn field_without_size_defaults_to_space_word_bits() {
        let source = ":space reg addr=32 word=16 type=register\n:reg MSR subfields={\n    OK @(0)\n    BAD @(16)\n}";
        let err = validate_src(source).expect_err("expected default size failure");
        let diagnostics = match err {
            IsaError::Diagnostics { diagnostics, .. } => diagnostics,
            other => panic!("unexpected error: {other:?}"),
        };
        let diag = diagnostics
            .iter()
            .find(|diag| diag.code == "validation.subfield.bit-range")
            .expect("missing bit range diagnostic");
        let span = diag.span.as_ref().expect("diagnostic span");
        assert_eq!(snippet_from_span(source, span), "@(16)");
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

    #[test]
    fn redirect_errors_highlight_reference_span() {
        let source = ":space reg addr=32 word=64 type=register\n:reg PC size=64 subfields={\n    LSB @(0)\n}\n:reg alias redirect=PC::MSB";
        let err = validate_src(source).expect_err("redirect should fail");
        let diagnostics = match err {
            IsaError::Diagnostics { diagnostics, .. } => diagnostics,
            other => panic!("unexpected error: {other:?}"),
        };
        let diag = diagnostics
            .iter()
            .find(|diag| diag.code == "validation.redirect.unknown-subfield")
            .expect("missing redirect diagnostic");
        let span = diag.span.as_ref().expect("diagnostic span");
        assert_eq!(snippet_from_span(source, span), "MSB");
    }

    fn snippet_from_span(source: &str, span: &crate::soc::isa::diagnostic::SourceSpan) -> String {
        let lines: Vec<&str> = source.split('\n').collect();
        let start_line = span.start.line.saturating_sub(1);
        let end_line = span.end.line.saturating_sub(1);
        let mut snippet = String::new();
        for line_idx in start_line..=end_line {
            if line_idx >= lines.len() {
                break;
            }
            let line = lines[line_idx];
            let start_col = if line_idx == start_line {
                span.start.column.saturating_sub(1)
            } else {
                0
            };
            let end_col = if line_idx == end_line {
                span.end.column.saturating_sub(1)
            } else {
                line.chars().count()
            };
            if end_col <= start_col {
                continue;
            }
            let slice: String = line
                .chars()
                .skip(start_col)
                .take(end_col - start_col)
                .collect();
            if !snippet.is_empty() {
                snippet.push('\n');
            }
            snippet.push_str(&slice);
        }
        snippet
    }
}
