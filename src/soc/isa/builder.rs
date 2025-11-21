//! Helpers for constructing `IsaDocument`s programmatically without routing through the file parser.
//!
//! The builder keeps source spans consistent so downstream diagnostics can still attach to
//! deterministic locations even when the ISA metadata is produced in memory.

use std::path::PathBuf;

use crate::soc::isa::ast::{
    FormDecl, InstructionDecl, IsaItem, IsaSpecification, MaskField, MaskSelector, MaskSpec,
    SpaceAttribute, SpaceDecl, SpaceKind, SpaceMember, SpaceMemberDecl, SubFieldDecl, SubFieldOp,
};
use crate::soc::isa::diagnostic::{SourcePosition, SourceSpan};

/// Convenience wrapper for assembling a full ISA document in memory.
pub struct IsaBuilder {
    path: PathBuf,
    span: SourceSpan,
    items: Vec<IsaItem>,
}

impl IsaBuilder {
    /// Creates a new builder that pretends every element originated from `path`.
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        let path = path.into();
        let span = SourceSpan::point(path.clone(), SourcePosition::new(1, 1));
        Self {
            path,
            span,
            items: Vec::new(),
        }
    }

    /// Appends a space declaration with the provided attributes.
    pub fn add_space(
        &mut self,
        name: impl Into<String>,
        kind: SpaceKind,
        attributes: impl Into<Vec<SpaceAttribute>>,
    ) -> &mut Self {
        let space = SpaceDecl {
            name: name.into(),
            kind,
            attributes: attributes.into(),
            span: self.span.clone(),
            enable: None,
        };
        self.items.push(IsaItem::Space(space));
        self
    }

    /// Appends a form declaration to the builder.
    pub fn add_form(
        &mut self,
        space: impl Into<String>,
        name: impl Into<String>,
        parent: Option<String>,
        subfields: impl IntoIterator<Item = SubFieldDecl>,
    ) -> &mut Self {
        self.add_form_with_display(space, name, parent, None, subfields)
    }

    /// Appends a form declaration with an optional display template.
    pub fn add_form_with_display(
        &mut self,
        space: impl Into<String>,
        name: impl Into<String>,
        parent: Option<String>,
        display: Option<String>,
        subfields: impl IntoIterator<Item = SubFieldDecl>,
    ) -> &mut Self {
        let space = space.into();
        let name = name.into();
        let subfields: Vec<SubFieldDecl> = subfields.into_iter().collect();
        let form_decl = FormDecl {
            space: space.clone(),
            name,
            parent,
            description: None,
            display,
            subfields,
            span: self.span.clone(),
        };
        let form = SpaceMemberDecl {
            space,
            member: SpaceMember::Form(form_decl),
        };
        self.items.push(IsaItem::SpaceMember(form));
        self
    }

    /// Begins an instruction declaration; call [`InstructionBuilder::finish`] to push it.
    pub fn instruction(
        &mut self,
        space: impl Into<String>,
        name: impl Into<String>,
    ) -> InstructionBuilder<'_> {
        let decl = InstructionDecl {
            space: space.into(),
            form: None,
            name: name.into(),
            description: None,
            operands: Vec::new(),
            mask: None,
            encoding: None,
            semantics: None,
            display: None,
            operator: None,
            span: self.span.clone(),
        };
        InstructionBuilder {
            builder: self,
            decl,
        }
    }

    fn push_instruction(&mut self, decl: InstructionDecl) {
        self.items.push(IsaItem::SpaceMember(SpaceMemberDecl {
            space: decl.space.clone(),
            member: SpaceMember::Instruction(decl),
        }));
    }

    /// Finishes building and returns the assembled document.
    pub fn build(self) -> IsaSpecification {
        IsaSpecification::new(self.path, self.items)
    }

}

/// Builder for the richer `InstructionDecl` structure.
pub struct InstructionBuilder<'a> {
    builder: &'a mut IsaBuilder,
    decl: InstructionDecl,
}

impl<'a> InstructionBuilder<'a> {
    /// Sets the form reference used by the instruction.
    pub fn form(mut self, form: impl Into<String>) -> Self {
        self.decl.form = Some(form.into());
        self
    }

    /// Replaces the operand list.
    pub fn operands<I, S>(mut self, operands: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.decl.operands = operands.into_iter().map(|s| s.into()).collect();
        self
    }

    /// Applies a full mask specification.
    pub fn mask(mut self, mask: MaskSpec) -> Self {
        self.decl.mask = Some(mask);
        self
    }

    /// Adds a single mask field selector/value pair.
    pub fn mask_field(mut self, selector: MaskSelector, value: u64) -> Self {
        let mask = self
            .decl
            .mask
            .get_or_insert(MaskSpec { fields: Vec::new() });
        mask.fields.push(MaskField { selector, value });
        self
    }

    /// Assigns a human-readable description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.decl.description = Some(description.into());
        self
    }

    /// Assigns a display template used during disassembly.
    pub fn display(mut self, template: impl Into<String>) -> Self {
        self.decl.display = Some(template.into());
        self
    }

    /// Supplies an operator token referenced by display templates.
    pub fn operator(mut self, op: impl Into<String>) -> Self {
        self.decl.operator = Some(op.into());
        self
    }

    /// Completes the builder and pushes the instruction into the owning document.
    pub fn finish(self) -> &'a mut IsaBuilder {
        self.builder.push_instruction(self.decl);
        self.builder
    }
}

/// Utility for defining a subfield without spelling out the struct each time.
pub fn subfield(name: impl Into<String>, bit_spec: impl Into<String>) -> SubFieldDecl {
    SubFieldDecl {
        name: name.into(),
        bit_spec: bit_spec.into(),
        operations: Vec::new(),
        description: None,
    }
}

/// Utility for creating an operation entry used inside a subfield declaration.
pub fn subfield_op(kind: impl Into<String>, subtype: Option<impl Into<String>>) -> SubFieldOp {
    SubFieldOp {
        kind: kind.into(),
        subtype: subtype.map(|value| value.into()),
    }
}

/// Convenience helper for mask selectors aimed at named fields.
pub fn mask_field_selector(name: impl Into<String>) -> MaskSelector {
    MaskSelector::Field(name.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::isa::validator::Validator;

    #[test]
    fn builds_logic_space_with_instruction() {
        let mut builder = IsaBuilder::new("builder.isa");
        builder.add_space(
            "logic",
            SpaceKind::Logic,
            vec![
                SpaceAttribute::AddressBits(32),
                SpaceAttribute::WordSize(32),
            ],
        );
        builder.add_form("logic", "BASE", None, vec![subfield("OPCD", "@(0..5)")]);
        builder
            .instruction("logic", "add")
            .form("BASE")
            .mask_field(mask_field_selector("OPCD"), 0x1f)
            .finish();
        let doc = builder.build();

        let mut validator = Validator::new();
        validator
            .validate(&[doc])
            .expect("builder-generated doc should validate");
    }
}
