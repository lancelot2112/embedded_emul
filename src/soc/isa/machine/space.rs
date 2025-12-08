//! Space metadata and helpers for form/field management, bit-spec parsing,
//! and register field integration.

use std::collections::BTreeMap;

use crate::soc::device::endianness::Endianness;
use crate::soc::isa::ast::{FieldDecl, FormDecl, SpaceAttribute, SpaceDecl, SpaceKind, SubFieldOp};
use crate::soc::isa::error::IsaError;
use crate::soc::prog::types::{BitFieldSegment, BitFieldSpec, bitfield::BitFieldError};

use super::register::{RegisterBinding, RegisterInfo, derive_register_binding};

#[derive(Debug, Clone)]
pub struct SpaceInfo {
    pub name: String,
    pub kind: SpaceKind,
    pub size_bits: Option<u32>,
    pub endianness: Endianness,
    pub forms: BTreeMap<String, FormInfo>,
    pub registers: BTreeMap<String, RegisterInfo>,
    pub enable: Option<crate::soc::isa::semantics::SemanticExpr>,
}

impl SpaceInfo {
    pub fn from_decl(space: SpaceDecl) -> Self {
        let mut size_bits = None;
        let mut endianness = Endianness::Big;
        for attr in &space.attributes {
            match attr {
                SpaceAttribute::WordSize(bits) => size_bits = Some(*bits),
                SpaceAttribute::Endianness(value) => endianness = *value,
                _ => {}
            }
        }
        Self {
            name: space.name,
            kind: space.kind,
            size_bits,
            endianness,
            forms: BTreeMap::new(),
            registers: BTreeMap::new(),
            enable: space.enable,
        }
    }

    pub fn word_bits(&self) -> Result<u32, IsaError> {
        self.size_bits.ok_or_else(|| {
            IsaError::Machine(format!(
                "logic space '{}' missing required word size attribute",
                self.name
            ))
        })
    }

    pub fn add_form(&mut self, form: FormDecl) -> Result<(), IsaError> {
        let word_bits = self.word_bits()?;
        let mut info = if let Some(parent) = &form.parent {
            self.forms.get(parent).cloned().ok_or_else(|| {
                IsaError::Machine(format!(
                    "form '{}' inherits from undefined form '{}::{}'",
                    form.name, self.name, parent
                ))
            })?
        } else {
            FormInfo::new(form.name.clone())
        };

        for sub in form.subfields {
            if info.contains(&sub.name) {
                return Err(IsaError::Machine(format!(
                    "form '{}::{}' redeclares subfield '{}'",
                    self.name, form.name, sub.name
                )));
            }
            let spec = parse_bit_spec(word_bits, &sub.bit_spec).map_err(|err| {
                IsaError::Machine(format!(
                    "invalid bit spec '{}' on field '{}::{}::{}': {err}",
                    sub.bit_spec, self.name, form.name, sub.name
                ))
            })?;
            let register = derive_register_binding(&sub.operations);
            let operand_kind = classify_operand_kind(register.as_ref(), &sub.operations);
            info.push_field(FieldEncoding {
                name: sub.name,
                spec,
                operations: sub.operations,
                register,
                kind: operand_kind,
            });
        }

        if let Some(template) = form.display.clone() {
            info.display = Some(template);
        }

        self.forms.insert(form.name, info);
        Ok(())
    }

    pub fn add_register_field(&mut self, field: FieldDecl) {
        if self.kind != SpaceKind::Register {
            return;
        }
        let info = RegisterInfo::from_decl(field);
        self.registers.insert(info.name.clone(), info);
    }
}

#[derive(Debug, Clone)]
pub struct FormInfo {
    fields: Vec<FieldEncoding>,
    field_index: BTreeMap<String, usize>,
    pub operand_order: Vec<String>,
    pub display: Option<String>,
}

impl FormInfo {
    pub fn new(_name: String) -> Self {
        Self {
            fields: Vec::new(),
            field_index: BTreeMap::new(),
            operand_order: Vec::new(),
            display: None,
        }
    }

    pub fn contains(&self, name: &str) -> bool {
        self.field_index.contains_key(name)
    }

    pub fn push_field(&mut self, field: FieldEncoding) {
        if !field.is_function_only() {
            self.operand_order.push(field.name.clone());
        }
        self.field_index
            .insert(field.name.clone(), self.fields.len());
        self.fields.push(field);
    }

    pub fn subfield(&self, name: &str) -> Option<&FieldEncoding> {
        self.field_index
            .get(name)
            .and_then(|index| self.fields.get(*index))
    }

    pub fn field_iter(&self) -> impl Iterator<Item = &FieldEncoding> {
        self.fields.iter()
    }
}

#[derive(Debug, Clone)]
pub struct FieldEncoding {
    pub name: String,
    pub spec: BitFieldSpec,
    pub operations: Vec<SubFieldOp>,
    pub register: Option<RegisterBinding>,
    pub kind: OperandKind,
}

impl FieldEncoding {
    pub fn is_function_only(&self) -> bool {
        !self
            .operations
            .iter()
            .any(|op| !op.kind.eq_ignore_ascii_case("func"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandKind {
    Register,
    Immediate,
    Other,
}

pub fn parse_bit_spec(word_bits: u32, spec: &str) -> Result<BitFieldSpec, BitFieldSpecParseError> {
    BitFieldSpec::from_spec_str(word_bits as u16, spec).map_err(BitFieldSpecParseError::SpecError)
}

pub fn encode_constant(
    spec: &BitFieldSpec,
    value: u64,
) -> Result<(u64, u64), BitFieldSpecParseError> {
    let mask = spec
        .segments
        .iter()
        .fold(0u64, |acc, segment| match segment {
            BitFieldSegment::Slice(slice) => acc | slice.mask,
            BitFieldSegment::Literal { .. } => acc,
        });
    let encoded = spec
        .write_to(0, value)
        .map_err(BitFieldSpecParseError::SpecError)?;
    Ok((mask, encoded & mask))
}

pub fn ensure_byte_aligned(word_bits: u32, instr: &str) -> Result<usize, IsaError> {
    if word_bits % 8 != 0 {
        return Err(IsaError::Machine(format!(
            "instruction '{}' width ({word_bits} bits) is not byte-aligned",
            instr
        )));
    }
    Ok((word_bits / 8) as usize)
}

pub fn mask_for_bits(bits: u32) -> u64 {
    if bits >= 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    }
}

fn classify_operand_kind(register: Option<&RegisterBinding>, ops: &[SubFieldOp]) -> OperandKind {
    if register.is_some() {
        return OperandKind::Register;
    }
    if ops.iter().any(|op| {
        let kind = op.kind.to_ascii_lowercase();
        kind == "immediate" || kind.starts_with("imm")
    }) {
        return OperandKind::Immediate;
    }
    OperandKind::Other
}

#[derive(Debug)]
pub enum BitFieldSpecParseError {
    TooWide,
    SpecError(BitFieldError),
}

impl std::fmt::Display for BitFieldSpecParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BitFieldSpecParseError::TooWide => write!(f, "bit spec exceeds 64-bit container"),
            BitFieldSpecParseError::SpecError(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for BitFieldSpecParseError {}
