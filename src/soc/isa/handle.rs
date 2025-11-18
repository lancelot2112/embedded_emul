//! Public entry point that upstream systems use to interpret binaries via `.isa` metadata.

use std::path::Path;

use crate::soc::system::bus::DataHandle;

use super::error::IsaError;
use super::loader::IsaLoader;
use super::machine::{Disassembly, MachineDescription};
use super::semantics::SemanticBlock;

pub struct IsaHandle {
    machine: MachineDescription,
}

impl IsaHandle {
    pub fn from_files<P: AsRef<Path>>(entry: P) -> Result<Self, IsaError> {
        let mut loader = IsaLoader::new();
        let machine = loader.load_machine(entry)?;
        Ok(Self { machine })
    }

    /// Disassembles len bytes starting at the current address exposed by the `DataHandle`.
    pub fn disassemble_range(
        &self,
        data: &mut DataHandle,
        len: usize,
    ) -> Result<Vec<Disassembly>, IsaError> {
        let mut buf = vec![0u8; len];
        data.read_bytes(&mut buf)?;
        Ok(self.machine.disassemble(&buf))
    }

    /// Emits semantic IR for a previously decoded instruction mnemonic. In the future this will
    /// look up the instruction metadata by opcode and return a typed IR tree.
    pub fn semantics_for(&self, mnemonic: &str) -> Option<&SemanticBlock> {
        self.machine
            .instructions
            .iter()
            .find(|instr| instr.name == mnemonic)
            .and_then(|instr| instr.semantics.as_ref())
    }
}
