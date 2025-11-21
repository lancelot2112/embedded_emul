#![cfg(test)]

use std::path::PathBuf;

use crate::loader::isa::parse_str;
use crate::soc::isa::ast::{
    FormDecl, InstructionDecl, IsaItem, IsaSpecification, SpaceAttribute, SpaceDecl, SpaceKind,
    SpaceMember, SpaceMemberDecl, SubFieldDecl,
};
use crate::soc::isa::diagnostic::{DiagnosticPhase, SourcePosition, SourceSpan};

use super::super::error::IsaError;
use super::Validator;

pub(super) fn validate_src(source: &str) -> Result<(), IsaError> {
    let doc = parse_str(PathBuf::from("test.isa"), source)?;
    let mut validator = Validator::new();
    validator.validate(&[doc])
}

pub(super) fn expect_validation_diag(err: IsaError, needle: &str) {
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

pub(super) fn manual_span() -> SourceSpan {
    SourceSpan::point(PathBuf::from("manual.isa"), SourcePosition::new(1, 1))
}

pub(super) fn validate_items(items: Vec<IsaItem>) -> Result<(), IsaError> {
    let doc = IsaSpecification::new(PathBuf::from("manual.isa"), items);
    let mut validator = Validator::new();
    validator.validate(&[doc])
}

pub(super) fn space_decl(name: &str, kind: SpaceKind, attributes: Vec<SpaceAttribute>) -> IsaItem {
    IsaItem::Space(SpaceDecl {
        name: name.to_string(),
        kind,
        attributes,
        span: manual_span(),
        enable: None,
    })
}

pub(super) fn logic_form(space: &str, name: &str) -> IsaItem {
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

pub(super) fn logic_instruction(space: &str, form: Option<&str>, name: &str) -> IsaItem {
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

pub(super) fn simple_subfield(name: &str) -> SubFieldDecl {
    SubFieldDecl {
        name: name.to_string(),
        bit_spec: "@(0..5)".to_string(),
        operations: Vec::new(),
        description: None,
    }
}
