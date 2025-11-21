use std::sync::Arc;

use crate::soc::core::specification::{CoreSpec, CoreSpecBuildError};
use crate::soc::device::Endianness;
use crate::soc::isa::machine::{Disassembly, Instruction, MachineDescription};
use crate::soc::isa::semantics::SemanticBlock;

#[derive(Clone)]
pub struct IsaSpec {
    machine: Arc<MachineDescription>,
    core: Arc<CoreSpec>,
    instructions: Vec<InstructionSemantics>,
}

impl IsaSpec {
    pub fn from_machine(
        core_name: impl Into<String>,
        machine: MachineDescription,
        endianness: Option<Endianness>,
    ) -> Result<Self, IsaSpecError> {
        let machine = Arc::new(machine);
        let core = Arc::new(
            CoreSpec::from_machine(core_name, &machine, endianness).map_err(IsaSpecError::Core)?,
        );
        let instructions = machine
            .instructions
            .iter()
            .map(InstructionSemantics::from_instruction)
            .collect();
        Ok(Self {
            machine,
            core,
            instructions,
        })
    }

    pub fn machine(&self) -> &Arc<MachineDescription> {
        &self.machine
    }

    pub fn core_spec(&self) -> &Arc<CoreSpec> {
        &self.core
    }

    pub fn instructions(&self) -> &[InstructionSemantics] {
        &self.instructions
    }

    pub fn disassemble(&self, bytes: &[u8]) -> Vec<Disassembly> {
        self.machine.disassemble(bytes)
    }
}

#[derive(Debug, Clone)]
pub struct InstructionSemantics {
    pub name: String,
    pub space: String,
    pub operands: Vec<String>,
    pub semantics: Option<SemanticBlock>,
}

impl InstructionSemantics {
    fn from_instruction(instr: &Instruction) -> Self {
        Self {
            name: instr.name.clone(),
            space: instr.space.clone(),
            operands: instr.operands.clone(),
            semantics: instr.semantics.clone(),
        }
    }
}

#[derive(Debug)]
pub enum IsaSpecError {
    Core(CoreSpecBuildError),
}

impl std::fmt::Display for IsaSpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IsaSpecError::Core(err) => write!(f, "core spec build failed: {err}"),
        }
    }
}

impl std::error::Error for IsaSpecError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            IsaSpecError::Core(err) => Some(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::isa::ast::SpaceKind;
    use crate::soc::isa::machine::{RegisterInfo, SpaceInfo};
    use std::collections::BTreeMap;

    fn sample_machine() -> MachineDescription {
        let mut machine = MachineDescription::new();
        let mut registers = BTreeMap::new();
        registers.insert("r0".into(), RegisterInfo::with_size("r0", Some(32)));
        let space = SpaceInfo {
            name: "regs".into(),
            kind: SpaceKind::Register,
            size_bits: Some(32),
            endianness: Endianness::Little,
            forms: BTreeMap::new(),
            registers,
            enable: None,
        };
        machine.spaces.insert("regs".into(), space);

        machine.instructions.push(Instruction {
            space: "logic".into(),
            name: "nop".into(),
            form: None,
            description: None,
            operands: Vec::new(),
            display: None,
            operator: None,
            mask: None,
            encoding: None,
            semantics: Some(SemanticBlock::empty()),
        });
        machine
    }

    #[test]
    fn isa_spec_exposes_machine_and_core() {
        let machine = sample_machine();
        let isa = IsaSpec::from_machine("demo", machine, None).expect("isa spec");
        assert_eq!(isa.core_spec().registers().len(), 1);
        assert_eq!(isa.instructions().len(), 1);
        assert_eq!(isa.instructions()[0].name, "nop");
    }
}
