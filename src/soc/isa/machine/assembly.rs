//! Instruction assembly helpers that mirror the disassembler and remove
//! duplicated encoding logic from tests.

use super::MachineDescription;
use super::instruction::Instruction;
use super::space::{FieldEncoding, OperandKind, encode_constant, ensure_byte_aligned};
use crate::soc::device::endianness::Endianness;
use crate::soc::isa::ast::MaskSelector;
use crate::soc::isa::error::IsaError;

impl MachineDescription {
    /// Encodes a machine instruction using explicit operand/value pairs.
    pub fn encode_instruction(
        &self,
        mnemonic: &str,
        operands: &[(&str, i64)],
    ) -> Result<Vec<u8>, IsaError> {
        let instr = self.find_instruction(mnemonic)?;
        self.encode_with_operands(instr, operands)
    }

    /// Assembles a single textual instruction such as `add. r1, r2, r3`.
    pub fn assemble(&self, asm: &str) -> Result<Vec<u8>, IsaError> {
        let (mnemonic, raw_operands) = parse_assembly(asm)?;
        let instr = self.find_instruction(&mnemonic)?;
        let resolved = self.resolve_operand_values(instr, &raw_operands)?;
        let pairs: Vec<(&str, i64)> = resolved
            .iter()
            .map(|(name, value)| (name.as_str(), *value))
            .collect();
        self.encode_with_operands(instr, &pairs)
    }

    fn find_instruction(&self, mnemonic: &str) -> Result<&Instruction, IsaError> {
        self.instructions
            .iter()
            .find(|candidate| candidate.name == mnemonic)
            .ok_or_else(|| IsaError::Machine(format!("unknown instruction '{mnemonic}'")))
    }

    fn encode_with_operands(
        &self,
        instr: &Instruction,
        operands: &[(&str, i64)],
    ) -> Result<Vec<u8>, IsaError> {
        let space = self.spaces.get(&instr.space).ok_or_else(|| {
            IsaError::Machine(format!("instruction space '{}' missing", instr.space))
        })?;
        let word_bits = space.word_bits()?;
        let word_bytes = ensure_byte_aligned(word_bits, &instr.name)?;
        let mut bits = 0u64;

        if let Some(mask) = &instr.mask {
            for field in &mask.fields {
                let spec = match &field.selector {
                    MaskSelector::Field(name) => resolve_form_field(instr, space, name)?,
                    MaskSelector::BitExpr(expr) => super::space::parse_bit_spec(word_bits, expr)
                        .map_err(|err| {
                            IsaError::Machine(format!(
                                "invalid bit selector '{expr}' on instruction '{}': {err}",
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
                bits = (bits & !field_mask) | (encoded & field_mask);
            }
        }

        if let Some(form_name) = &instr.form {
            let form = space.forms.get(form_name).ok_or_else(|| {
                IsaError::Machine(format!(
                    "instruction '{}' references missing form '{}'",
                    instr.name, form_name
                ))
            })?;
            for (name, value) in operands {
                let field = form.subfield(name).ok_or_else(|| {
                    IsaError::Machine(format!("unknown operand '{name}' for '{}'", instr.name))
                })?;
                bits = field.spec.write_to(bits, *value as u64).map_err(|err| {
                    IsaError::Machine(format!(
                        "failed to encode operand '{name}' on '{}': {err}",
                        instr.name
                    ))
                })?;
            }
        } else if !operands.is_empty() {
            return Err(IsaError::Machine(format!(
                "instruction '{}' does not take operands",
                instr.name
            )));
        }

        let mut buffer = vec![0u8; word_bytes];
        write_word(bits, &mut buffer, space.endianness);
        Ok(buffer)
    }

    fn resolve_operand_values(
        &self,
        instr: &Instruction,
        raw_operands: &[String],
    ) -> Result<Vec<(String, i64)>, IsaError> {
        let space = self.spaces.get(&instr.space).ok_or_else(|| {
            IsaError::Machine(format!("instruction space '{}' missing", instr.space))
        })?;
        let Some(form_name) = &instr.form else {
            if raw_operands.is_empty() {
                return Ok(Vec::new());
            }
            return Err(IsaError::Machine(format!(
                "instruction '{}' has no form; unable to resolve operands",
                instr.name
            )));
        };
        let form = space.forms.get(form_name).ok_or_else(|| {
            IsaError::Machine(format!(
                "instruction '{}' references missing form '{}'",
                instr.name, form_name
            ))
        })?;

        let operand_names = if !instr.operands.is_empty() {
            instr.operands.clone()
        } else {
            form.operand_order.clone()
        };

        if operand_names.len() != raw_operands.len() {
            return Err(IsaError::Machine(format!(
                "instruction '{}' expects {} operand(s) but got {}",
                instr.name,
                operand_names.len(),
                raw_operands.len()
            )));
        }

        let mut resolved = Vec::with_capacity(raw_operands.len());
        for (name, raw) in operand_names.iter().zip(raw_operands.iter()) {
            let field = form.subfield(name).ok_or_else(|| {
                IsaError::Machine(format!(
                    "instruction '{}' references unknown operand '{}'",
                    instr.name, name
                ))
            })?;
            let value = parse_operand_value(self, field, raw)?;
            resolved.push((name.clone(), value));
        }

        Ok(resolved)
    }
}

fn parse_assembly(input: &str) -> Result<(String, Vec<String>), IsaError> {
    let mut parts = input
        .splitn(2, char::is_whitespace)
        .filter(|part| !part.is_empty());
    let mnemonic = parts
        .next()
        .ok_or_else(|| IsaError::Machine("assembly line missing mnemonic".into()))?;
    let operands = parts
        .next()
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect();
    Ok((mnemonic.to_string(), operands))
}

fn parse_operand_value(
    machine: &MachineDescription,
    field: &FieldEncoding,
    raw: &str,
) -> Result<i64, IsaError> {
    if let Some(binding) = &field.register {
        if let Some((_, element)) = machine.register_schema().find_by_label(&binding.space, raw) {
            return Ok(i64::from(element.index));
        }
        return Err(IsaError::Machine(format!(
            "unknown register '{raw}' for operand '{}'",
            field.name
        )));
    }

    if let Some(stripped) = raw
        .strip_prefix('r')
        .or_else(|| raw.strip_prefix('R'))
        .filter(|_| field.kind == OperandKind::Register)
    {
        return parse_numeric(stripped);
    }

    parse_numeric(raw)
}

fn parse_numeric(raw: &str) -> Result<i64, IsaError> {
    let trimmed = raw.trim();
    let sign = if trimmed.starts_with('-') { -1 } else { 1 };
    let number = trimmed.trim_start_matches(['-', '+']);
    let token = number.replace('_', "");
    let (base, digits) = if let Some(hex) = token
        .strip_prefix("0x")
        .or_else(|| token.strip_prefix("0X"))
    {
        (16, hex)
    } else if let Some(bin) = token
        .strip_prefix("0b")
        .or_else(|| token.strip_prefix("0B"))
    {
        (2, bin)
    } else if let Some(oct) = token
        .strip_prefix("0o")
        .or_else(|| token.strip_prefix("0O"))
    {
        (8, oct)
    } else {
        (10, token.as_str())
    };
    i64::from_str_radix(digits, base)
        .map(|value| value * sign)
        .map_err(|err| IsaError::Machine(format!("unable to parse numeric literal '{raw}': {err}")))
}

fn resolve_form_field(
    instr: &Instruction,
    space: &super::space::SpaceInfo,
    name: &str,
) -> Result<crate::soc::prog::types::BitFieldSpec, IsaError> {
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

fn write_word(bits: u64, buffer: &mut [u8], endianness: Endianness) {
    match endianness {
        Endianness::Little => {
            for (idx, byte) in buffer.iter_mut().enumerate() {
                *byte = ((bits >> (8 * idx)) & 0xFF) as u8;
            }
        }
        Endianness::Big => {
            let width = buffer.len();
            for (idx, byte) in buffer.iter_mut().enumerate() {
                let shift = 8 * (width - 1 - idx);
                *byte = ((bits >> shift) & 0xFF) as u8;
            }
        }
    }
}
