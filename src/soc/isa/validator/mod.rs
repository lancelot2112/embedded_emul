//! Semantic validation for parsed ISA documents and the merged machine description.

mod fields;
mod forms;
mod instructions;
mod spaces;

#[cfg(test)]
pub(super) mod test_support;

use std::collections::{BTreeMap, BTreeSet};

use super::ast::{IsaItem, IsaSpecification, SpaceKind, SpaceMember, SpaceMemberDecl};
use super::diagnostic::{DiagnosticLevel, DiagnosticPhase, IsaDiagnostic, SourceSpan};
use super::error::IsaError;
use super::logic::LogicSpaceState;
use super::machine::MachineDescription;
use super::space::SpaceState;

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

    fn validate_space_member(&mut self, member: &SpaceMemberDecl) {
        match &member.member {
            SpaceMember::Field(field) => self.validate_field(field),
            SpaceMember::Form(form) => self.validate_form(form),
            SpaceMember::Instruction(instr) => self.validate_instruction(instr),
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
}
