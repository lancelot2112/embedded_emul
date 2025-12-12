//! Core abstract syntax tree nodes produced by the `.isa` parser.

use std::path::PathBuf;

use crate::soc::device::endianness::Endianness;
use crate::soc::isa::diagnostic::SourceSpan;
use crate::soc::prog::types::bitfield::{BitFieldSpec, BitOrder};

use super::semantics::{SemanticBlock, SemanticExpr};

/// Represents a fully parsed ISA-like source file.
#[derive(Debug, Clone)]
pub struct IsaSpecification {
    pub path: PathBuf,
    pub items: Vec<IsaItem>,
    pub extends: Vec<PathBuf>,
    pub bit_order: BitOrder,
}

impl IsaSpecification {
    pub fn new(path: PathBuf, items: Vec<IsaItem>, extends: Vec<PathBuf>) -> Self {
        Self {
            path,
            items,
            extends,
            bit_order: BitOrder::Msb0,
        }
    }
}

/// High level items supported by the specification.
#[derive(Debug, Clone)]
pub enum IsaItem {
    Parameter(ParameterDecl),
    Space(SpaceDecl),
    SpaceMember(SpaceMemberDecl),
    Instruction(InstructionDecl),
    Macro(MacroDecl),
    Include(IncludeDecl),
}

#[derive(Debug, Clone)]
pub struct ParameterDecl {
    pub name: String,
    pub value: ParameterValue,
}

#[derive(Debug, Clone)]
pub enum ParameterValue {
    Word(String),
    Number(u64),
}

#[derive(Debug, Clone)]
pub struct SpaceDecl {
    pub name: String,
    pub kind: SpaceKind,
    pub attributes: Vec<SpaceAttribute>,
    pub span: SourceSpan,
    pub enable: Option<SemanticExpr>,
}

#[derive(Debug, Clone)]
pub struct SpaceMemberDecl {
    pub space: String,
    pub member: SpaceMember,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpaceKind {
    ReadWrite,
    ReadOnly,
    MemoryMappedIo,
    Register,
    Logic,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpaceAttribute {
    AddressBits(u32),
    WordSize(u32),
    Alignment(u32),
    Endianness(Endianness),
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)] // Variants mirror DSL syntax, so boxing would complicate parsing
pub enum SpaceMember {
    Field(FieldDecl),
    Form(FormDecl),
    Instruction(InstructionDecl),
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub space: String,
    pub name: String,
    pub range: Option<FieldIndexRange>,
    pub offset: Option<u64>,
    pub size: Option<u32>,
    pub reset: Option<u64>,
    pub description: Option<String>,
    pub redirect: Option<ContextReference>,
    pub subfields: Vec<SubFieldDecl>,
    pub span: SourceSpan,
    pub display: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FormDecl {
    pub space: String,
    pub name: String,
    pub parent: Option<String>,
    pub description: Option<String>,
    pub display: Option<String>,
    pub subfields: Vec<SubFieldDecl>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct FieldIndexRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone)]
pub struct ContextReference {
    pub segments: Vec<String>,
    pub segment_spans: Vec<SourceSpan>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct SubFieldDecl {
    pub name: String,
    pub bit_spec: String,
    pub operations: Vec<SubFieldOp>,
    pub description: Option<String>,
    pub bit_spec_span: Option<SourceSpan>,
    pub bit_order: BitOrder,
}

#[derive(Debug, Clone)]
pub struct SubFieldOp {
    pub kind: String,
    pub subtype: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InstructionDecl {
    pub space: String,
    pub form: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub operands: Vec<String>,
    pub mask: Option<MaskSpec>,
    pub encoding: Option<BitFieldSpec>,
    pub semantics: Option<SemanticBlock>,
    pub display: Option<String>,
    pub operator: Option<String>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct MacroDecl {
    pub name: String,
    pub parameters: Vec<String>,
    pub semantics: SemanticBlock,
    pub span: SourceSpan,
}

#[derive(Debug, Clone)]
pub struct MaskSpec {
    pub fields: Vec<MaskField>,
}

#[derive(Debug, Clone)]
pub enum MaskSelector {
    Field(String),
    BitExpr(String),
}

#[derive(Debug, Clone)]
pub struct MaskField {
    pub selector: MaskSelector,
    pub value: u64,
    pub value_text: Option<String>,
    pub value_span: Option<SourceSpan>,
    pub bit_order: BitOrder,
}

#[derive(Debug, Clone)]
pub struct IncludeDecl {
    pub path: PathBuf,
    pub optional: bool,
}
