//! Runtime representation of a validated ISA along with helpers for disassembly and semantics.

use std::collections::BTreeMap;

use crate::soc::prog::types::bitfield::BitFieldSpec;

use super::ast::{InstructionDecl, IsaSpecification, IsaItem, SpaceMember};
use super::error::IsaError;
use super::semantics::SemanticBlock;

#[derive(Debug, Clone)]
pub struct MachineDescription {
    pub instructions: Vec<Instruction>,
    pub spaces: BTreeMap<String, SpaceInfo>,
}

impl MachineDescription {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            spaces: BTreeMap::new(),
        }
    }

    pub fn from_documents(docs: Vec<IsaSpecification>) -> Result<Self, IsaError> {
        let mut machine = MachineDescription::new();
        for doc in docs {
            for item in doc.items {
                match item {
                    IsaItem::Instruction(instr) => {
                        machine.instructions.push(Instruction::from_decl(instr));
                    }
                    IsaItem::SpaceMember(member) => {
                        if let SpaceMember::Instruction(instr) = member.member {
                            machine.instructions.push(Instruction::from_decl(instr));
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(machine)
    }

    pub fn disassemble(&self, _bytes: &[u8]) -> Vec<Disassembly> {
        Vec::new()
    }
}

#[derive(Debug, Clone)]
pub struct SpaceInfo {
    pub name: String,
    pub size_bits: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub space: String,
    pub name: String,
    pub form: Option<String>,
    pub description: Option<String>,
    pub operands: Vec<String>,
    pub mask: Option<InstructionMask>,
    pub encoding: Option<BitFieldSpec>,
    pub semantics: Option<SemanticBlock>,
}

impl Instruction {
    pub fn from_decl(decl: InstructionDecl) -> Self {
        Self {
            space: decl.space,
            name: decl.name,
            form: decl.form,
            description: decl.description,
            operands: decl.operands,
            mask: decl.mask.map(|mask| InstructionMask {
                fields: mask.fields,
            }),
            encoding: decl.encoding,
            semantics: decl.semantics,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstructionMask {
    pub fields: Vec<super::ast::MaskField>,
}

#[derive(Debug, Clone)]
pub struct Disassembly {
    pub address: u64,
    pub opcode: u32,
    pub mnemonic: String,
    pub operands: Vec<String>,
}
