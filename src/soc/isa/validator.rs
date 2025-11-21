//! Semantic validation for parsed ISA documents and the merged machine description.

use std::collections::{BTreeMap, BTreeSet};

use super::ast::{
    ContextReference, FieldDecl, FormDecl, InstructionDecl, IsaItem, IsaSpecification,
    MaskSelector, SpaceAttribute, SpaceDecl, SpaceKind, SpaceMember, SpaceMemberDecl,
};
use super::diagnostic::{DiagnosticLevel, DiagnosticPhase, IsaDiagnostic, SourceSpan};
use super::error::IsaError;
use super::logic::{LogicFormError, LogicSpaceState};
use super::machine::MachineDescription;
use super::register::FieldRegistrationError;
use super::space::{SpaceState, resolve_reference_path};

#[derive(Default)]
pub struct Validator {
    seen_spaces: BTreeSet<String>,
    parameters: BTreeMap<String, String>,
    space_states: BTreeMap<String, SpaceState>,
    logic_states: BTreeMap<String, LogicSpaceState>,
    space_kinds: BTreeMap<String, SpaceKind>,
    logic_sizes: BTreeMap<String, u32>,
    space_enables: BTreeSet<String>,
    diagnostics: Vec<IsaDiagnostic>,
}

impl Validator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&mut self, docs: &[IsaSpecification]) -> Result<(), IsaError> {
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
        self.ensure_enable_coverage();
        if self.diagnostics.is_empty() {
            Ok(())
        } else {
            Err(IsaError::Diagnostics {
                phase: DiagnosticPhase::Validation,
                diagnostics: std::mem::take(&mut self.diagnostics),
            })
        }
    }

    pub fn finalize_machine(
        &self,
        docs: Vec<IsaSpecification>,
    ) -> Result<MachineDescription, IsaError> {
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
        self.space_states.entry(space.name.clone()).or_default();
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

    fn ensure_enable_coverage(&mut self) {
        if self.logic_sizes.len() <= 1 {
            return;
        }
        let mut by_size: BTreeMap<u32, Vec<String>> = BTreeMap::new();
        for (space, bits) in &self.logic_sizes {
            by_size.entry(*bits).or_default().push(space.clone());
        }
        if by_size.len() <= 1 {
            return;
        }
        let max_size = *by_size.keys().next_back().unwrap();
        for (bits, spaces) in by_size.iter().filter(|(bits, _)| **bits != max_size) {
            let covered = spaces.iter().any(|space| self.space_enables.contains(space));
            if !covered {
                let joined = spaces.join(", ");
                self.push_validation_diagnostic(
                    "validation.enable.missing",
                    format!(
                        "logic space(s) {joined} ({bits}-bit) require an 'enbl={{...}}' predicate when multiple instruction widths exist",
                    ),
                    None,
                );
            }
        }
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
    use crate::soc::isa::ast::{
        FormDecl, InstructionDecl, IsaItem, IsaSpecification, SpaceAttribute, SpaceDecl, SpaceKind,
        SpaceMember, SpaceMemberDecl, SubFieldDecl,
    };
    use crate::soc::isa::diagnostic::{DiagnosticPhase, SourcePosition, SourceSpan};
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

    fn manual_span() -> SourceSpan {
        SourceSpan::point(PathBuf::from("manual.isa"), SourcePosition::new(1, 1))
    }

    fn validate_items(items: Vec<IsaItem>) -> Result<(), IsaError> {
        let doc = IsaSpecification::new(PathBuf::from("manual.isa"), items);
        let mut validator = Validator::new();
        validator.validate(&[doc])
    }

    fn space_decl(name: &str, kind: SpaceKind, attributes: Vec<SpaceAttribute>) -> IsaItem {
        IsaItem::Space(SpaceDecl {
            name: name.to_string(),
            kind,
            attributes,
            span: manual_span(),
            enable: None,
        })
    }

    fn logic_form(space: &str, name: &str) -> IsaItem {
        IsaItem::SpaceMember(SpaceMemberDecl {
            space: space.to_string(),
            member: SpaceMember::Form(FormDecl {
                space: space.to_string(),
                name: name.to_string(),
                parent: None,
                description: None,
                display: None,
                subfields: vec![simple_subfield("OPCD")],
                span: manual_span(),
            }),
        })
    }

    fn logic_instruction(space: &str, form: Option<&str>, name: &str) -> IsaItem {
        IsaItem::SpaceMember(SpaceMemberDecl {
            space: space.to_string(),
            member: SpaceMember::Instruction(InstructionDecl {
                space: space.to_string(),
                form: form.map(|f| f.to_string()),
                name: name.to_string(),
                description: None,
                operands: Vec::new(),
                mask: None,
                encoding: None,
                semantics: None,
                display: None,
                operator: None,
                span: manual_span(),
            }),
        })
    }

    fn simple_subfield(name: &str) -> SubFieldDecl {
        SubFieldDecl {
            name: name.to_string(),
            bit_spec: "@(0..5)".to_string(),
            operations: Vec::new(),
            description: None,
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
    fn logic_instruction_accepts_inherited_fields() {
        validate_src(
            ":space logic addr=32 word=32 type=logic\n:logic BASE subfields={\n    OPCD @(0..5) op=func\n}\n:logic::BASE EXT subfields={\n    RT @(6..10) op=target\n}\n:logic::EXT add mask={OPCD=31}",
        )
        .expect("logic instruction referencing inherited form fields should validate");
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
            IsaError::Diagnostics {
                phase: DiagnosticPhase::Validation,
                diagnostics,
            } => {
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
