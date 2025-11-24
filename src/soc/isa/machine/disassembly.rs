//! Disassembly pipeline plus decode-space bookkeeping and enable-expression
//! evaluation used to pick the right instruction for a stream of bytes.

use crate::soc::device::endianness::Endianness;
use crate::soc::isa::ast::{MaskSelector, SpaceKind};
use crate::soc::isa::error::IsaError;
use crate::soc::isa::semantics::{BinaryOperator, SemanticExpr};
use crate::soc::prog::types::BitFieldSpec;

use super::MachineDescription;
use super::format;
use super::instruction::{Instruction, InstructionPattern};
use super::space::{
    SpaceInfo, encode_constant, ensure_byte_aligned, mask_for_bits, parse_bit_spec,
};

#[derive(Debug, Clone)]
pub struct Disassembly {
    pub address: u64,
    pub opcode: u64,
    pub mnemonic: String,
    pub operands: Vec<String>,
    pub display: Option<String>,
}

/// Lightweight decoding entry that retains references to the matched instruction
/// and its operand metadata so downstream components (like the semantics harness)
/// can inspect the raw bits instead of stringified operands.
pub struct DecodedInstruction<'a> {
    address: u64,
    bits: u64,
    instruction: &'a Instruction,
    pattern: &'a InstructionPattern,
}

impl<'a> DecodedInstruction<'a> {
    pub fn address(&self) -> u64 {
        self.address
    }

    pub fn bits(&self) -> u64 {
        self.bits
    }

    pub fn instruction(&self) -> &'a Instruction {
        self.instruction
    }

    pub fn operand_names(&self) -> &'a [String] {
        &self.pattern.operand_names
    }

    pub fn space(&self) -> &'a str {
        &self.pattern.space
    }

    pub fn form_name(&self) -> Option<&'a str> {
        self.pattern.form.as_deref()
    }
}

impl MachineDescription {
    pub fn disassemble_from(&self, bytes: &[u8], base_address: u64) -> Vec<Disassembly> {
        if self.decode_spaces.is_empty() {
            return Vec::new();
        }
        let mut cursor = 0usize;
        let mut address = base_address;
        let mut listing = Vec::new();

        while cursor < bytes.len() {
            let remaining = &bytes[cursor..];
            let Some(space) = self.select_space(remaining) else {
                break;
            };
            if remaining.len() < space.word_bytes {
                break;
            }
            let chunk = &remaining[..space.word_bytes];
            let bits = decode_word(chunk, space.endianness) & space.mask;
            let entry = if let Some(pattern) = self.best_match(&space.name, bits) {
                let instr = &self.instructions[pattern.instruction_idx];
                let operands = self.decode_operands(pattern, bits);
                let display = format::render_display(self, pattern, bits, &operands);
                Disassembly {
                    address,
                    opcode: bits,
                    mnemonic: instr.name.clone(),
                    operands,
                    display,
                }
            } else {
                Disassembly {
                    address,
                    opcode: bits,
                    mnemonic: "unknown".into(),
                    operands: vec![format!("0x{bits:0width$X}", width = space.word_bytes * 2)],
                    display: None,
                }
            };
            listing.push(entry);
            cursor += space.word_bytes;
            address += space.word_bytes as u64;
        }

        listing
    }

    pub fn decode_instructions(
        &self,
        bytes: &[u8],
        base_address: u64,
    ) -> Vec<DecodedInstruction<'_>> {
        if self.decode_spaces.is_empty() {
            return Vec::new();
        }
        let mut cursor = 0usize;
        let mut address = base_address;
        let mut entries = Vec::new();
        while cursor < bytes.len() {
            let remaining = &bytes[cursor..];
            let Some(space) = self.select_space(remaining) else {
                break;
            };
            if remaining.len() < space.word_bytes {
                break;
            }
            let chunk = &remaining[..space.word_bytes];
            let bits = decode_word(chunk, space.endianness) & space.mask;
            if let Some(pattern) = self.best_match(&space.name, bits) {
                let instr = &self.instructions[pattern.instruction_idx];
                entries.push(DecodedInstruction {
                    address,
                    bits,
                    instruction: instr,
                    pattern,
                });
            }
            cursor += space.word_bytes;
            address += space.word_bytes as u64;
        }
        entries
    }

    pub fn build_patterns(&mut self) -> Result<(), IsaError> {
        let mut patterns = Vec::new();
        for (idx, instr) in self.instructions.iter().enumerate() {
            if instr.mask.is_none() {
                continue;
            }
            if let Some(pattern) = self.build_pattern(idx, instr)? {
                patterns.push(pattern);
            }
        }
        self.patterns = patterns;
        Ok(())
    }

    pub fn build_decode_spaces(&mut self) -> Result<(), IsaError> {
        let mut spaces = Vec::new();
        for info in self.spaces.values() {
            if info.kind != SpaceKind::Logic {
                continue;
            }
            let word_bits = info.word_bits()?;
            let word_bytes = ensure_byte_aligned(word_bits, &info.name)?;
            let mask = mask_for_bits(word_bits);
            let enable = if let Some(expr) = &info.enable {
                Some(EnablePredicate::new(expr.clone(), word_bits, &info.name)?)
            } else {
                None
            };
            spaces.push(LogicDecodeSpace {
                name: info.name.clone(),
                word_bits,
                word_bytes,
                mask,
                endianness: info.endianness,
                enable,
            });
        }

        if spaces.is_empty() {
            self.decode_spaces.clear();
            return Ok(());
        }

        spaces.sort_by(|a, b| {
            a.word_bits
                .cmp(&b.word_bits)
                .then_with(|| a.name.cmp(&b.name))
        });
        self.decode_spaces = spaces;
        Ok(())
    }
}

impl MachineDescription {
    fn select_space(&self, bytes: &[u8]) -> Option<&LogicDecodeSpace> {
        self.decode_spaces.iter().find(|space| {
            if bytes.len() < space.word_bytes {
                return false;
            }
            let chunk = &bytes[..space.word_bytes];
            let bits = decode_word(chunk, space.endianness) & space.mask;
            match &space.enable {
                Some(predicate) => predicate.evaluate(bits),
                None => true,
            }
        })
    }

    fn best_match(&self, space: &str, bits: u64) -> Option<&InstructionPattern> {
        self.patterns
            .iter()
            .filter(|pattern| pattern.space == space && bits & pattern.mask == pattern.value)
            .max_by_key(|pattern| pattern.specificity)
    }

    fn decode_operands(&self, pattern: &InstructionPattern, bits: u64) -> Vec<String> {
        let form_name = match &pattern.form {
            Some(name) => name.clone(),
            None => return Vec::new(),
        };
        let Some(space) = self.spaces.get(&pattern.space) else {
            return Vec::new();
        };
        let Some(form) = space.forms.get(&form_name) else {
            return Vec::new();
        };

        pattern
            .operand_names
            .iter()
            .map(|name| {
                form.subfield(name)
                    .map(|field| {
                        let (value, _) = field.spec.read_bits(bits);
                        format::format_operand(self, field, value)
                    })
                    .unwrap_or_else(|| format!("?{name}"))
            })
            .collect()
    }

    fn build_pattern(
        &self,
        idx: usize,
        instr: &Instruction,
    ) -> Result<Option<InstructionPattern>, IsaError> {
        let Some(mask_spec) = instr.mask.as_ref() else {
            return Ok(None);
        };
        let space = self.spaces.get(&instr.space).ok_or_else(|| {
            IsaError::Machine(format!(
                "instruction '{}' references unknown space '{}'",
                instr.name, instr.space
            ))
        })?;
        if space.kind != SpaceKind::Logic {
            return Ok(None);
        }
        let word_bits = space.word_bits()?;
        ensure_byte_aligned(word_bits, &instr.name)?;

        let mut mask = 0u64;
        let mut value_bits = 0u64;
        for field in &mask_spec.fields {
            let spec = match &field.selector {
                MaskSelector::Field(name) => self.resolve_form_field(instr, space, name)?,
                MaskSelector::BitExpr(expr) => parse_bit_spec(word_bits, expr).map_err(|err| {
                    IsaError::Machine(format!(
                        "invalid bit expression '{expr}' in instruction '{}': {err}",
                        instr.name
                    ))
                })?,
            };
            let (field_mask, encoded) = encode_constant(&spec, field.value).map_err(|err| {
                IsaError::Machine(format!(
                    "mask literal for instruction '{}' does not fit: {err}",
                    instr.name
                ))
            })?;
            let overlap = mask & field_mask;
            if overlap != 0 {
                let previous = value_bits & field_mask;
                if previous != (encoded & field_mask) {
                    eprintln!(
                        "warning: instruction '{}' mask selector '{:?}' overrides previously set bits; treating as alias",
                        instr.name, field.selector
                    );
                }
            }
            mask |= field_mask;
            value_bits = (value_bits & !field_mask) | encoded;
        }

        let operand_names = if !instr.operands.is_empty() {
            instr.operands.clone()
        } else {
            instr
                .form
                .as_ref()
                .and_then(|form_name| space.forms.get(form_name))
                .map(|form| form.operand_order.clone())
                .unwrap_or_default()
        };

        let form_display = instr
            .form
            .as_ref()
            .and_then(|form_name| space.forms.get(form_name))
            .and_then(|form| form.display.clone());

        let mut pattern = InstructionPattern {
            instruction_idx: idx,
            space: instr.space.clone(),
            form: instr.form.clone(),
            mask,
            value: value_bits,
            operand_names: operand_names.clone(),
            display: None,
            operator: instr.operator.clone(),
            specificity: mask.count_ones(),
        };

        pattern.with_display_override(instr.display.clone(), form_display, &operand_names);

        Ok(Some(pattern))
    }

    fn resolve_form_field(
        &self,
        instr: &Instruction,
        space: &SpaceInfo,
        name: &str,
    ) -> Result<BitFieldSpec, IsaError> {
        let form_name = instr.form.as_ref().ok_or_else(|| {
            IsaError::Machine(format!(
                "instruction '{}' uses mask field '{}' without a form",
                instr.name, name
            ))
        })?;
        let form = space.forms.get(form_name).ok_or_else(|| {
            IsaError::Machine(format!(
                "instruction '{}' references undefined form '{}::{}'",
                instr.name, space.name, form_name
            ))
        })?;
        form.subfield(name)
            .map(|field| field.spec.clone())
            .ok_or_else(|| {
                IsaError::Machine(format!(
                    "instruction '{}' references unknown field '{}' on form '{}::{}'",
                    instr.name, name, space.name, form_name
                ))
            })
    }
}

fn decode_word(bytes: &[u8], endianness: Endianness) -> u64 {
    match endianness {
        Endianness::Little => bytes
            .iter()
            .enumerate()
            .fold(0u64, |acc, (idx, byte)| acc | ((*byte as u64) << (idx * 8))),
        Endianness::Big => bytes
            .iter()
            .fold(0u64, |acc, byte| (acc << 8) | (*byte as u64)),
    }
}

#[derive(Debug, Clone)]
pub(super) struct LogicDecodeSpace {
    name: String,
    word_bits: u32,
    word_bytes: usize,
    mask: u64,
    endianness: Endianness,
    enable: Option<EnablePredicate>,
}

#[derive(Debug, Clone)]
struct EnablePredicate {
    expr: EnableExpr,
}

impl EnablePredicate {
    fn new(expr: SemanticExpr, word_bits: u32, space: &str) -> Result<Self, IsaError> {
        Ok(Self {
            expr: EnableExpr::compile(expr, word_bits, space)?,
        })
    }

    fn evaluate(&self, bits: u64) -> bool {
        self.expr.evaluate(bits).as_bool()
    }
}

#[derive(Debug, Clone)]
enum EnableExpr {
    Literal(u64),
    Bool(bool),
    BitField(BitFieldSpec),
    Binary {
        op: BinaryOperator,
        lhs: Box<EnableExpr>,
        rhs: Box<EnableExpr>,
    },
}

impl EnableExpr {
    fn compile(expr: SemanticExpr, word_bits: u32, space: &str) -> Result<Self, IsaError> {
        match expr {
            SemanticExpr::Literal(value) => Ok(Self::Literal(value)),
            SemanticExpr::Identifier(name) => match name.to_ascii_lowercase().as_str() {
                "true" => Ok(Self::Bool(true)),
                "false" => Ok(Self::Bool(false)),
                other => Err(IsaError::Machine(format!(
                    "identifier '{other}' is not supported in enbl expression for space '{space}'",
                ))),
            },
            SemanticExpr::BitExpr(spec) => {
                let parsed = parse_bit_spec(word_bits, &spec).map_err(|err| {
                    IsaError::Machine(format!(
                        "invalid bit selector '{spec}' in enbl expression for space '{space}': {err}",
                    ))
                })?;
                Ok(Self::BitField(parsed))
            }
            SemanticExpr::BinaryOp { op, lhs, rhs } => {
                if !matches!(
                    op,
                    BinaryOperator::Eq
                        | BinaryOperator::Ne
                        | BinaryOperator::LogicalAnd
                        | BinaryOperator::LogicalOr
                ) {
                    return Err(IsaError::Machine(format!(
                        "operator '{op:?}' is not supported in enbl expression for space '{space}'",
                    )));
                }
                let left = Self::compile(*lhs, word_bits, space)?;
                let right = Self::compile(*rhs, word_bits, space)?;
                Ok(Self::Binary {
                    op,
                    lhs: Box::new(left),
                    rhs: Box::new(right),
                })
            }
        }
    }

    fn evaluate(&self, bits: u64) -> EnableValue {
        match self {
            EnableExpr::Literal(value) => EnableValue::Number(*value),
            EnableExpr::Bool(value) => EnableValue::Bool(*value),
            EnableExpr::BitField(spec) => {
                let (value, _) = spec.read_bits(bits);
                EnableValue::Number(value)
            }
            EnableExpr::Binary { op, lhs, rhs } => match op {
                BinaryOperator::Eq => {
                    let l = lhs.evaluate(bits).as_number();
                    let r = rhs.evaluate(bits).as_number();
                    EnableValue::Bool(l == r)
                }
                BinaryOperator::Ne => {
                    let l = lhs.evaluate(bits).as_number();
                    let r = rhs.evaluate(bits).as_number();
                    EnableValue::Bool(l != r)
                }
                BinaryOperator::LogicalAnd => {
                    let l = lhs.evaluate(bits).as_bool();
                    if !l {
                        return EnableValue::Bool(false);
                    }
                    let r = rhs.evaluate(bits).as_bool();
                    EnableValue::Bool(l && r)
                }
                BinaryOperator::LogicalOr => {
                    let l = lhs.evaluate(bits).as_bool();
                    if l {
                        return EnableValue::Bool(true);
                    }
                    let r = rhs.evaluate(bits).as_bool();
                    EnableValue::Bool(l || r)
                }
                _ => unreachable!("unsupported operator filtered during compilation"),
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum EnableValue {
    Number(u64),
    Bool(bool),
}

impl EnableValue {
    fn as_bool(self) -> bool {
        match self {
            EnableValue::Bool(value) => value,
            EnableValue::Number(value) => value != 0,
        }
    }

    fn as_number(self) -> u64 {
        match self {
            EnableValue::Number(value) => value,
            EnableValue::Bool(value) => value as u64,
        }
    }
}
