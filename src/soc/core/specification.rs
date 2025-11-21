use std::collections::{HashMap, HashSet};

use crate::soc::device::Endianness;
use crate::soc::isa::ast::SpaceKind;
use crate::soc::isa::machine::{MachineDescription, RegisterInfo};

const DEFAULT_REGISTER_BITS: u32 = 64;

/// Declarative layout of a processor core captured as register bit-slices over a
/// contiguous backing store.
#[derive(Debug, Clone)]
pub struct CoreSpec {
    name: String,
    endianness: Endianness,
    registers: Vec<RegisterSpec>,
    index: HashMap<String, usize>,
    total_bits: u32,
}

impl CoreSpec {
    pub fn builder(name: impl Into<String>, endianness: Endianness) -> CoreSpecBuilder {
        CoreSpecBuilder::new(name, endianness)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn endianness(&self) -> Endianness {
        self.endianness
    }

    pub fn registers(&self) -> &[RegisterSpec] {
        &self.registers
    }

    pub fn register(&self, name: &str) -> Option<&RegisterSpec> {
        self.index
            .get(name)
            .and_then(|idx| self.registers.get(*idx))
    }

    pub fn total_bits(&self) -> u32 {
        self.total_bits
    }

    pub fn byte_len(&self) -> usize {
        ((self.total_bits as usize + 7) / 8).max(1)
    }
}

/// Structured representation of a named register encoded at a fixed bit offset
/// within a processor snapshot.
#[derive(Debug, Clone)]
pub struct RegisterSpec {
    pub name: String,
    pub bit_offset: u32,
    pub bit_len: u32,
}

impl RegisterSpec {
    fn extent(&self) -> u32 {
        self.bit_offset + self.bit_len
    }
}

#[derive(Debug)]
pub struct CoreSpecBuilder {
    name: String,
    endianness: Endianness,
    cursor: u32,
    registers: Vec<RegisterSpec>,
    seen: HashSet<String>,
    errors: Vec<CoreSpecError>,
}

impl CoreSpecBuilder {
    pub fn new(name: impl Into<String>, endianness: Endianness) -> Self {
        Self {
            name: name.into(),
            endianness,
            cursor: 0,
            registers: Vec::new(),
            seen: HashSet::new(),
            errors: Vec::new(),
        }
    }

    pub fn register(mut self, name: impl Into<String>, bit_len: u32) -> Self {
        let name = name.into();
        if bit_len == 0 {
            self.errors.push(CoreSpecError::InvalidWidth {
                name,
                width: bit_len,
            });
            return self;
        }
        let bit_offset = self.cursor;
        self.push_spec(name, bit_offset, bit_len);
        self.cursor += bit_len;
        self
    }

    pub fn register_at(mut self, name: impl Into<String>, bit_offset: u32, bit_len: u32) -> Self {
        let name = name.into();
        if bit_len == 0 {
            self.errors.push(CoreSpecError::InvalidWidth {
                name,
                width: bit_len,
            });
            return self;
        }
        self.push_spec(name, bit_offset, bit_len);
        self.cursor = self.cursor.max(bit_offset + bit_len);
        self
    }

    pub fn build(mut self) -> Result<CoreSpec, CoreSpecBuildError> {
        if !self.errors.is_empty() {
            return Err(CoreSpecBuildError {
                errors: self.errors,
            });
        }
        self.registers.sort_by_key(|spec| spec.bit_offset);
        let mut index = HashMap::with_capacity(self.registers.len());
        for (idx, spec) in self.registers.iter().enumerate() {
            index.insert(spec.name.clone(), idx);
        }
        let total_bits = self
            .registers
            .iter()
            .map(RegisterSpec::extent)
            .max()
            .unwrap_or(0);
        Ok(CoreSpec {
            name: self.name,
            endianness: self.endianness,
            registers: self.registers,
            index,
            total_bits,
        })
    }

    fn push_spec(&mut self, name: String, bit_offset: u32, bit_len: u32) {
        if !self.seen.insert(name.clone()) {
            self.errors.push(CoreSpecError::DuplicateRegister(name));
            return;
        }
        self.registers.push(RegisterSpec {
            name,
            bit_offset,
            bit_len,
        });
    }
}

impl CoreSpec {
    pub fn from_machine(
        name: impl Into<String>,
        machine: &MachineDescription,
        endianness_override: Option<Endianness>,
    ) -> Result<Self, CoreSpecBuildError> {
        let resolved_endian = endianness_override
            .or_else(|| register_space_endianness(machine))
            .unwrap_or(Endianness::Little);
        let mut builder = CoreSpec::builder(name, resolved_endian);
        for (space_name, space) in &machine.spaces {
            if space.kind != SpaceKind::Register {
                continue;
            }
            let default_bits = space.size_bits.unwrap_or(DEFAULT_REGISTER_BITS);
            for info in space.registers.values() {
                let width = register_bit_width(info, default_bits);
                let register_name = format!("{}::{}", space_name, info.name);
                builder = builder.register(register_name, width);
            }
        }
        builder.build()
    }
}

fn register_space_endianness(machine: &MachineDescription) -> Option<Endianness> {
    machine
        .spaces
        .values()
        .find(|space| space.kind == SpaceKind::Register)
        .map(|space| space.endianness)
}

fn register_bit_width(info: &RegisterInfo, fallback_bits: u32) -> u32 {
    let width = info.size_bits().unwrap_or(fallback_bits);
    if width == 0 {
        DEFAULT_REGISTER_BITS
    } else {
        width
    }
}

#[derive(Debug, Clone)]
pub enum CoreSpecError {
    DuplicateRegister(String),
    InvalidWidth { name: String, width: u32 },
}

#[derive(Debug)]
pub struct CoreSpecBuildError {
    pub errors: Vec<CoreSpecError>,
}

impl std::fmt::Display for CoreSpecBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.errors.is_empty() {
            return write!(f, "descriptor build failed");
        }
        writeln!(
            f,
            "descriptor build failed with {} error(s):",
            self.errors.len()
        )?;
        for err in &self.errors {
            writeln!(f, "  - {err}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CoreSpecBuildError {}

impl std::fmt::Display for CoreSpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreSpecError::DuplicateRegister(name) => {
                write!(f, "register '{name}' declared multiple times")
            }
            CoreSpecError::InvalidWidth { name, width } => {
                write!(
                    f,
                    "register '{name}' must reserve at least 1 bit (got {width})"
                )
            }
        }
    }
}

impl std::error::Error for CoreSpecError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::isa::machine::MachineDescription;
    use crate::soc::isa::machine::RegisterInfo;
    use crate::soc::isa::machine::SpaceInfo;
    use std::collections::BTreeMap;

    #[test]
    fn builder_detects_duplicate_registers() {
        let build = CoreSpec::builder("demo", Endianness::Little)
            .register("r0", 32)
            .register("r0", 32)
            .build();
        assert!(build.is_err(), "duplicate names should error");
    }

    #[test]
    fn descriptor_records_offsets() {
        let descriptor = CoreSpec::builder("demo", Endianness::Big)
            .register("pc", 64)
            .register("sp", 64)
            .register_at("flags", 16, 8)
            .build()
            .expect("descriptor");
        assert_eq!(descriptor.name(), "demo");
        assert_eq!(descriptor.total_bits(), 128, "auto layout extends cursor");
        let flags = descriptor.register("flags").expect("flags spec");
        assert_eq!(flags.bit_offset, 16);
        assert_eq!(flags.bit_len, 8);
        assert_eq!(descriptor.byte_len(), 16, "128 bits -> 16 bytes");
    }

    #[test]
    fn zero_width_register_fails_build() {
        let build = CoreSpec::builder("demo", Endianness::Little)
            .register("invalid", 0)
            .build();
        assert!(build.is_err(), "zero-width register should fail");
    }

    #[test]
    fn derives_registers_from_machine_description() {
        let mut machine = MachineDescription::new();
        let mut registers = BTreeMap::new();
        registers.insert("r0".into(), RegisterInfo::with_size("r0", Some(32)));
        registers.insert("r1".into(), RegisterInfo::with_size("r1", Some(32)));
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

        let spec = CoreSpec::from_machine("demo", &machine, None).expect("core spec");
        assert_eq!(spec.total_bits(), 64);
        assert_eq!(spec.registers().len(), 2);
    }
}
