//! Runtime representation of a validated ISA along with helpers for disassembly and semantics.

use std::collections::BTreeMap;

use crate::soc::prog::types::bitfield::BitFieldSpec;

use super::ast::{IsaDocument, InstructionDecl, IsaItem};
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

    pub fn from_documents(docs: Vec<IsaDocument>) -> Result<Self, IsaError> {
        let mut machine = MachineDescription::new();
        for doc in docs {
            for item in doc.items {
                if let IsaItem::Instruction(instr) = item {
                    machine.instructions.push(Instruction::from_decl(instr));
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
    pub name: String,
    pub mask: Option<InstructionMask>,
    pub encoding: Option<BitFieldSpec>,
    pub semantics: Option<SemanticBlock>,
}

impl Instruction {
    pub fn from_decl(decl: InstructionDecl) -> Self {
        Self {
            name: decl.name,
            mask: decl.mask.map(|mask| InstructionMask { fields: mask.fields }),
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
