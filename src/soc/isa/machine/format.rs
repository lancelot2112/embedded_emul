//! Operand and display formatting helpers shared by the machine runtime.

use std::iter::Peekable;
use std::str::Chars;

use super::MachineDescription;
use super::instruction::InstructionPattern;
use super::space::{FieldEncoding, FormInfo, OperandKind};

pub(super) fn format_operand(
    machine: &MachineDescription,
    field: &FieldEncoding,
    value: u64,
) -> String {
    if let Some(binding) = &field.register {
        if let Some(space) = machine.spaces.get(&binding.space)
            && let Some(register) = space.registers.get(&binding.field)
        {
            return register.format(value);
        }
        return format!("{}{}", binding.field, value);
    }

    if field.kind == OperandKind::Immediate {
        return format_immediate(field, value);
    }

    if field
        .operations
        .iter()
        .any(|op| op.kind.eq_ignore_ascii_case("reg"))
    {
        return format!("r{value}");
    }

    format!("{value}")
}

pub(super) fn render_display(
    machine: &MachineDescription,
    pattern: &InstructionPattern,
    bits: u64,
    operands: &[String],
) -> Option<String> {
    let template = pattern.display.as_ref()?;
    let form_name = pattern.form.as_ref()?;
    let space = machine.spaces.get(&pattern.space)?;
    let form = space.forms.get(form_name)?;

    Some(DisplayRenderer::new(template, machine, form, pattern, bits, operands).render())
}

pub(super) fn default_display_template(
    form: Option<&String>,
    operands: &[String],
) -> Option<String> {
    if form.is_none() || operands.is_empty() {
        return None;
    }
    let parts: Vec<String> = operands.iter().map(|name| format!("#{name}")).collect();
    Some(parts.join(", "))
}

fn format_immediate(field: &FieldEncoding, value: u64) -> String {
    let mut bits = u32::from(field.spec.data_width());
    if bits == 0 {
        bits = 1;
    }
    let digits = (bits as usize).div_ceil(4);
    let truncated = if bits >= 64 {
        value
    } else {
        let mask = (1u64 << bits) - 1;
        value & mask
    };
    format!("0x{truncated:0digits$X}")
}

struct DisplayRenderer<'a> {
    machine: &'a MachineDescription,
    form: &'a FormInfo,
    pattern: &'a InstructionPattern,
    bits: u64,
    operands: &'a [String],
    template: &'a str,
}

impl<'a> DisplayRenderer<'a> {
    fn new(
        template: &'a str,
        machine: &'a MachineDescription,
        form: &'a FormInfo,
        pattern: &'a InstructionPattern,
        bits: u64,
        operands: &'a [String],
    ) -> Self {
        Self {
            machine,
            form,
            pattern,
            bits,
            operands,
            template,
        }
    }

    fn render(&self) -> String {
        let mut result = String::with_capacity(self.template.len());
        let mut chars = self.template.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch != '#' {
                result.push(ch);
                continue;
            }
            if matches!(chars.peek(), Some('#')) {
                chars.next();
                result.push('#');
                continue;
            }
            let token = Self::next_identifier(&mut chars);
            if token.is_empty() {
                result.push('#');
                continue;
            }
            if let Some(value) = self.resolve_token(&token) {
                result.push_str(&value);
            } else {
                result.push('#');
                result.push_str(&token);
            }
        }
        result
    }

    fn next_identifier(iter: &mut Peekable<Chars<'_>>) -> String {
        let mut ident = String::new();
        while let Some(&ch) = iter.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ident.push(ch);
                iter.next();
            } else {
                break;
            }
        }
        ident
    }

    fn resolve_token(&self, token: &str) -> Option<String> {
        if token.eq_ignore_ascii_case("op") {
            return self.pattern.operator.as_ref().cloned();
        }
        if let Some(value) = self.operand_value(token) {
            return Some(value.to_string());
        }
        let field = self.form.subfield(token)?;
        let (value, _) = field.spec.read_bits(self.bits);
        Some(format_operand(self.machine, field, value))
    }

    fn operand_value(&self, token: &str) -> Option<&str> {
        self.pattern
            .operand_names
            .iter()
            .zip(self.operands.iter())
            .find(|(name, _)| name.as_str() == token)
            .map(|(_, value)| value.as_str())
    }
}
