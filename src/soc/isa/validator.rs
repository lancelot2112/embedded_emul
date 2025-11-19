//! Semantic validation for parsed ISA documents and the merged machine description.

use std::collections::{BTreeMap, BTreeSet};

use super::ast::{
    ContextReference, FieldDecl, FormDecl, InstructionDecl, IsaDocument, IsaItem, MaskSelector,
    SpaceAttribute, SpaceDecl, SpaceKind, SpaceMember, SpaceMemberDecl,
};
use super::diagnostic::{DiagnosticLevel, DiagnosticPhase, IsaDiagnostic, SourceSpan};
use super::error::IsaError;
use super::logic::{LogicFormError, LogicSpaceState};
use super::machine::MachineDescription;
use super::register::FieldRegistrationError;
use super::space::{SpaceState, resolve_reference_path};

pub struct Validator {
    seen_spaces: BTreeSet<String>,
    parameters: BTreeMap<String, String>,
    space_states: BTreeMap<String, SpaceState>,
    logic_states: BTreeMap<String, LogicSpaceState>,
    space_kinds: BTreeMap<String, SpaceKind>,
    diagnostics: Vec<IsaDiagnostic>,
}

impl Validator {
    pub fn new() -> Self {
        Self {
            seen_spaces: BTreeSet::new(),
            parameters: BTreeMap::new(),
            space_states: BTreeMap::new(),
            logic_states: BTreeMap::new(),
            space_kinds: BTreeMap::new(),
            diagnostics: Vec::new(),
        }
    }

    pub fn validate(&mut self, docs: &[IsaDocument]) -> Result<(), IsaError> {
        for doc in docs {
            for item in &doc.items {
                match item {
                    IsaItem::Space(space) => self.validate_space(space),
                    IsaItem::Parameter(param) => {
                        self.parameters
                            .insert(param.name.clone(), format!("{:?}", param.value));
                    }
                    IsaItem::SpaceMember(member) => self.validate_space_member(member),
                    _ => {}
                }
            }
        }
        if self.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(IsaError::Diagnostics {
                phase: DiagnosticPhase::Validation,
                diagnostics: std::mem::take(&mut self.diagnostics),
            })
        }
    }

    pub fn finalize_machine(&self, docs: Vec<IsaDocument>) -> Result<MachineDescription, IsaError> {
        MachineDescription::from_documents(docs)
    }

    fn validate_space(&mut self, space: &SpaceDecl) {
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
            } else {
                self.push_validation_diagnostic(
                    "validation.logic.word-size",
                    format!("logic space '{}' missing word size", space.name),
                    Some(space.span.clone()),
                );
            }
        }
        self.space_states
            .entry(space.name.clone())
            .or_insert_with(SpaceState::new);
    }

    fn validate_space_member(&mut self, member: &SpaceMemberDecl) {
        match &member.member {
            SpaceMember::Field(field) => self.validate_field(field),
            SpaceMember::Form(form) => self.validate_form(form),
            SpaceMember::Instruction(instr) => self.validate_instruction(instr),
        }
    }

    fn validate_form(&mut self, form: &FormDecl) {
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

    fn validate_instruction(&mut self, instr: &InstructionDecl) {
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
                if let MaskSelector::Field(name) = &field.selector {
                    if !form_info.subfields.contains_key(name) {
                        unknown_fields.push(name.clone());
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

    fn validate_field(&mut self, field: &FieldDecl) {
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

    fn push_validation_diagnostic(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        span: Option<SourceSpan>,
    ) {
        self.diagnostics.push(IsaDiagnostic::new(
            DiagnosticPhase::Validation,
            DiagnosticLevel::Error,
            code,
            message,
            span,
        ));
    }

    fn logic_state(&mut self, space: &str) -> Option<&mut LogicSpaceState> {
        self.logic_states.get_mut(space)
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
        if let Some(subfield_name) = path.get(0) {
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
        } else if path.len() > 0 {
            self.push_validation_diagnostic(
                "validation.redirect.depth",
                "redirect context depth exceeds field::subfield",
                Some(field.span.clone()),
            );
        }
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
    use super::*;
    use crate::loader::isa::parse_str;
    use crate::soc::isa::diagnostic::DiagnosticPhase;
    use std::path::PathBuf;

    fn validate_src(source: &str) -> Result<(), IsaError> {
        let doc = parse_str(PathBuf::from("test.isa"), source)?;
        let mut validator = Validator::new();
        validator.validate(&[doc])
    }

    fn expect_validation_diag(err: IsaError, needle: &str) {
        match err {
            IsaError::Diagnostics {
                phase: DiagnosticPhase::Validation,
                diagnostics,
            } => {
                assert!(
                    diagnostics.iter().any(|diag| diag.message.contains(needle)),
                    "no diagnostic containing '{needle}': {diagnostics:?}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

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
            IsaError::Diagnostics {
                phase: DiagnosticPhase::Validation,
                diagnostics,
            } => {
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
    fn logic_form_requires_parent() {
        let err = validate_src(
            ":space logic addr=32 word=32 type=logic\n:logic::UNKNOWN child subfields={\n    OPCD @(0..5)\n}",
        )
        .unwrap_err();
        expect_validation_diag(err, "parent form 'UNKNOWN'");
    }

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
}
