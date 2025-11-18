//! Core abstract syntax tree nodes produced by the `.isa` parser.

use std::path::PathBuf;

use crate::soc::prog::types::bitfield::BitFieldSpec;

use super::semantics::SemanticBlock;

/// Represents a fully parsed ISA-like source file.
#[derive(Debug, Clone)]
pub struct IsaDocument {
    pub path: PathBuf,
    pub items: Vec<IsaItem>,
}

impl IsaDocument {
    pub fn new(path: PathBuf, items: Vec<IsaItem>) -> Self {
        Self { path, items }
    }
}

/// High level items supported by the specification.
#[derive(Debug, Clone)]
pub enum IsaItem {
    Parameter(ParameterDecl),
    Space(SpaceDecl),
    Instruction(InstructionDecl),
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
    pub members: Vec<SpaceMember>,
}

#[derive(Debug, Clone)]
pub enum SpaceKind {
    Memory,
    Logic,
}

#[derive(Debug, Clone)]
pub enum SpaceAttribute {
    Size(u32),
    AddressBits(u32),
    WordSize(u32),
    Alignment(u32),
    Endianness(Endianness),
}

#[derive(Debug, Clone)]
pub enum Endianness {
    Big,
    Little,
}

#[derive(Debug, Clone)]
pub enum SpaceMember {
    Field(FieldDecl),
    Instruction(InstructionDecl),
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub name: String,
    pub bitfield: BitFieldSpec,
}

#[derive(Debug, Clone)]
pub struct InstructionDecl {
    pub space: String,
    pub form: Option<String>,
    pub name: String,
    pub operands: Vec<String>,
    pub mask: Option<MaskSpec>,
    pub encoding: Option<BitFieldSpec>,
    pub semantics: Option<SemanticBlock>,
}

#[derive(Debug, Clone)]
pub struct MaskSpec {
    pub fields: Vec<MaskField>,
}

#[derive(Debug, Clone)]
pub struct MaskField {
    pub name: String,
    pub value: u64,
    pub width: u8,
}

#[derive(Debug, Clone)]
pub struct IncludeDecl {
    pub path: PathBuf,
    pub optional: bool,
}
