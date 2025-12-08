use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::loader::isa::IsaLoader;
use crate::soc::core::specification::{CoreSpec, CoreSpecBuildError};
use crate::soc::core::state::{CoreState, StateError};
use crate::soc::device::Endianness;
use crate::soc::isa::error::IsaError;
use crate::soc::isa::machine::{
    DecodedInstruction, HostServices, MachineDescription, SoftwareHost,
};
use crate::soc::isa::semantics::ParameterBindings;
use crate::soc::isa::semantics::program::RegisterRef;
use crate::soc::isa::semantics::runtime::SemanticRuntime;
use crate::soc::isa::semantics::trace::{ExecutionTracer, TraceEvent};
use crate::soc::isa::semantics::value::SemanticValue;

/// Convenience wrapper that mirrors the ergonomics of emulators like Unicorn by
/// owning a machine description, core snapshot, and semantics runtime in one
/// place so tests can seed registers, feed instruction bytes, and observe the
/// resulting mutations.
pub struct ExecutionHarness<H: HostServices> {
    runtime: SemanticRuntime,
    machine: MachineDescription,
    core_spec: Arc<CoreSpec>,
    state: CoreState,
    host: H,
}

#[derive(Debug, Clone)]
pub struct InstructionExecution {
    pub address: u64,
    pub mnemonic: String,
    pub bits: u64,
    pub return_value: Option<SemanticValue>,
}

pub enum HarnessError {
    Isa(IsaError),
    Core(CoreSpecBuildError),
    State(StateError),
}

impl std::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HarnessError::Isa(err) => write!(f, "ISA error: {err}"),
            HarnessError::Core(err) => write!(f, "core spec error: {err}"),
            HarnessError::State(err) => write!(f, "core state error: {err}"),
        }
    }
}

impl std::error::Error for HarnessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            HarnessError::Isa(err) => Some(err),
            HarnessError::Core(err) => Some(err),
            HarnessError::State(err) => Some(err),
        }
    }
}

impl std::fmt::Debug for HarnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl From<IsaError> for HarnessError {
    fn from(value: IsaError) -> Self {
        HarnessError::Isa(value)
    }
}

impl From<CoreSpecBuildError> for HarnessError {
    fn from(value: CoreSpecBuildError) -> Self {
        HarnessError::Core(value)
    }
}

impl From<StateError> for HarnessError {
    fn from(value: StateError) -> Self {
        HarnessError::State(value)
    }
}

impl ExecutionHarness<SoftwareHost> {
    /// Loads the provided `.coredef` (plus its includes) and seeds the harness
    /// using the default software host helpers.
    pub fn from_coredef<P: AsRef<Path>>(
        core_name: impl Into<String>,
        definition: P,
        endianness_override: Option<Endianness>,
    ) -> Result<Self, HarnessError> {
        let mut loader = IsaLoader::new();
        let machine = loader.load_machine(definition)?;
        Self::from_machine(
            core_name,
            machine,
            endianness_override,
            SoftwareHost,
        )
    }
}

impl<H: HostServices> ExecutionHarness<H> {
    pub fn from_machine(
        core_name: impl Into<String>,
        machine: MachineDescription,
        endianness_override: Option<Endianness>,
        host: H,
    ) -> Result<Self, HarnessError> {
        let runtime = SemanticRuntime::new();
        let core_spec = Arc::new(CoreSpec::from_machine(
            core_name,
            &machine,
            endianness_override,
        )?);
        let state = CoreState::new(core_spec.clone())?;
        Ok(Self {
            runtime,
            machine,
            core_spec,
            state,
            host,
        })
    }

    pub fn machine(&self) -> &MachineDescription {
        &self.machine
    }

    pub fn core_spec(&self) -> &Arc<CoreSpec> {
        &self.core_spec
    }

    pub fn runtime(&self) -> &SemanticRuntime {
        &self.runtime
    }

    pub fn enable_tracer(&mut self, tracer: Box<dyn ExecutionTracer>) {
        self.runtime.set_tracer(Some(tracer));
    }

    pub fn disable_tracer(&mut self) {
        self.runtime.set_tracer(None);
    }

    pub fn state(&self) -> &CoreState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut CoreState {
        &mut self.state
    }

    pub fn read_register_value(
        &mut self,
        space: &str,
        name: &str,
        subfield: Option<&str>,
        index: Option<i64>,
    ) -> Result<SemanticValue, HarnessError> {
        let registers = self.runtime.register_access(&self.machine);
        let reference = RegisterRef {
            space: space.to_string(),
            name: name.to_string(),
            subfield: subfield.map(|value| value.to_string()),
            index: None,
            span: None,
        };
        let resolved = registers.resolve(&reference, index)?;
        Ok(resolved.read(&mut self.state)?)
    }

    pub fn write_register_value(
        &mut self,
        space: &str,
        name: &str,
        subfield: Option<&str>,
        index: Option<i64>,
        value: i64,
    ) -> Result<(), HarnessError> {
        let registers = self.runtime.register_access(&self.machine);
        let reference = RegisterRef {
            space: space.to_string(),
            name: name.to_string(),
            subfield: subfield.map(|value| value.to_string()),
            index: None,
            span: None,
        };
        let resolved = registers.resolve(&reference, index)?;
        resolved.write(&mut self.state, value)?;
        Ok(())
    }

    pub fn read(&mut self, register: &str) -> Result<u64, HarnessError> {
        let reference = Self::parse_register_reference(register)?;
        let registers = self.runtime.register_access(&self.machine);
        let resolved = registers.resolve(&reference, None)?;
        Ok(resolved.read_bits(&mut self.state)?)
    }

    pub fn write(&mut self, register: &str, value: u64) -> Result<(), HarnessError> {
        let reference = Self::parse_register_reference(register)?;
        let registers = self.runtime.register_access(&self.machine);
        let resolved = registers.resolve(&reference, None)?;
        resolved.write_bits(&mut self.state, value)?;
        Ok(())
    }

    pub fn execute_block(
        &mut self,
        base_address: u64,
        rom: &[u8],
    ) -> Result<Vec<InstructionExecution>, HarnessError> {
        let decoded = self.machine.decode_instructions(rom, base_address);
        let disassembly = self.machine.disassemble_from(rom, base_address);
        let mut executions = Vec::with_capacity(decoded.len());
        for (entry, listing) in decoded.into_iter().zip(disassembly.into_iter()) {
            let mnemonic = entry.instruction().name.clone();
            let detail = listing
                .display
                .clone()
                .unwrap_or_else(|| listing.operands.join(", "));
            self.runtime.emit_trace(TraceEvent::Fetch {
                address: listing.address,
                opcode: listing.opcode,
                mnemonic: listing.mnemonic.clone(),
                detail,
            });
            let return_value = if let Some(block) = entry.instruction().semantics.as_ref() {
                let program = block.ensure_program()?;
                let params = self.bind_parameters(&entry)?;
                self.runtime.execute_program(
                    &self.machine,
                    &mut self.state,
                    &mut self.host,
                    &params,
                    program,
                )?
            } else {
                None
            };
            executions.push(InstructionExecution {
                address: entry.address(),
                mnemonic,
                bits: entry.bits(),
                return_value,
            });
        }
        Ok(executions)
    }

    fn bind_parameters(
        &self,
        decoded: &DecodedInstruction<'_>,
    ) -> Result<HashMap<String, SemanticValue>, IsaError> {
        let mut bindings = ParameterBindings::new();
        bindings.extend_from_parameters(
            self.machine
                .parameters
                .iter()
                .map(|(name, value)| (name.as_str(), value)),
        )?;
        if let Some(form_name) = decoded.form_name() {
            let space = self.machine.spaces.get(decoded.space()).ok_or_else(|| {
                IsaError::Machine(format!(
                    "instruction '{}' references unknown space '{}'",
                    decoded.instruction().name,
                    decoded.space()
                ))
            })?;
            let form = space.forms.get(form_name).ok_or_else(|| {
                IsaError::Machine(format!(
                    "instruction '{}::{}' references undefined form '{}::{}'",
                    decoded.space(),
                    decoded.instruction().name,
                    decoded.space(),
                    form_name,
                ))
            })?;
            for field in form.field_iter() {
                let value = field.spec.read_from(decoded.bits()) as i64;
                bindings.insert_int(field.name.clone(), value);
            }
        }
        Ok(bindings.into_inner())
    }
}

impl<H: HostServices> ExecutionHarness<H> {
    fn parse_register_reference(register: &str) -> Result<RegisterRef, HarnessError> {
        let trimmed = register.trim();
        let mut parts = trimmed.split("::");
        let space = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                HarnessError::Isa(IsaError::Machine(format!(
                    "register reference '{register}' missing space prefix"
                )))
            })?;
        let name = parts
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                HarnessError::Isa(IsaError::Machine(format!(
                    "register reference '{register}' missing name"
                )))
            })?;
        let subfield = match parts.next() {
            Some(val) if !val.is_empty() => {
                if parts.next().is_some() {
                    return Err(HarnessError::Isa(IsaError::Machine(format!(
                        "register reference '{register}' has too many segments"
                    ))));
                }
                Some(val.to_string())
            }
            Some(_) => None,
            None => None,
        };
        Ok(RegisterRef {
            space: space.to_string(),
            name: name.to_string(),
            subfield,
            index: None,
            span: None,
        })
    }
}
