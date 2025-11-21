//! Runtime representation of a validated ISA along with helpers for disassembly and semantics.

use std::collections::BTreeMap;
use std::iter::Peekable;
use std::str::Chars;

use crate::soc::device::endianness::Endianness;
use crate::soc::prog::types::{BitFieldSegment, BitFieldSpec, TypeId};

use super::ast::{
    FieldDecl, FieldIndexRange, FormDecl, InstructionDecl, IsaItem, IsaSpecification, MaskSelector,
    SpaceAttribute, SpaceDecl, SpaceKind, SpaceMember, SubFieldOp,
};
use super::error::IsaError;
use super::semantics::{BinaryOperator, SemanticBlock, SemanticExpr};

#[derive(Debug, Clone)]
pub struct MachineDescription {
    pub instructions: Vec<Instruction>,
    pub spaces: BTreeMap<String, SpaceInfo>,
    patterns: Vec<InstructionPattern>,
    decode_spaces: Vec<LogicDecodeSpace>,
}

impl Default for MachineDescription {
    fn default() -> Self {
        Self {
            instructions: Vec::new(),
            spaces: BTreeMap::new(),
            patterns: Vec::new(),
            decode_spaces: Vec::new(),
        }
    }
}

impl MachineDescription {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_documents(docs: Vec<IsaSpecification>) -> Result<Self, IsaError> {
        let mut spaces = Vec::new();
        let mut forms = Vec::new();
        let mut fields = Vec::new();
        let mut instructions = Vec::new();
        for doc in docs {
            for item in doc.items {
                match item {
                    IsaItem::Space(space) => spaces.push(space),
                    IsaItem::SpaceMember(member) => match member.member {
                        SpaceMember::Form(form) => forms.push(form),
                        SpaceMember::Instruction(instr) => instructions.push(instr),
                        SpaceMember::Field(field) => fields.push(field),
                    },
                    IsaItem::Instruction(instr) => instructions.push(instr),
                    _ => {}
                }
            }
        }

        let mut machine = MachineDescription::new();
        for space in spaces {
            machine.register_space(space);
        }
        for form in forms {
            machine.register_form(form)?;
        }
        for instr in instructions {
            machine.instructions.push(Instruction::from_decl(instr));
        }
        for field in fields {
            machine.register_field(field)?;
        }
        machine.build_patterns()?;
        machine.build_decode_spaces()?;

        Ok(machine)
    }

    /// Disassembles machine words assuming an implicit base address of zero.
    pub fn disassemble(&self, bytes: &[u8]) -> Vec<Disassembly> {
        self.disassemble_from(bytes, 0)
    }

    /// Disassembles machine words and annotates them with `base_address` offsets.
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
                let display = self.render_display(pattern, bits, &operands);
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
        let Some(form_name) = pattern.form.as_ref() else {
            return Vec::new();
        };
        let Some(space) = self.spaces.get(&pattern.space) else {
            return Vec::new();
        };
        let Some(form) = space.forms.get(form_name) else {
            return Vec::new();
        };

        pattern
            .operand_names
            .iter()
            .map(|name| {
                form.subfield(name)
                    .map(|field| {
                        let (value, _) = field.spec.read_bits(bits);
                        self.format_operand(field, value)
                    })
                    .unwrap_or_else(|| format!("?{name}"))
            })
            .collect()
    }

    fn render_display(
        &self,
        pattern: &InstructionPattern,
        bits: u64,
        operands: &[String],
    ) -> Option<String> {
        let template = pattern.display.as_ref()?;
        let Some(space) = self.spaces.get(&pattern.space) else {
            return Some(template.clone());
        };
        let Some(form_name) = pattern.form.as_ref() else {
            return Some(template.clone());
        };
        let Some(form) = space.forms.get(form_name) else {
            return Some(template.clone());
        };

        Some(DisplayRenderer::new(
            template,
            self,
            form,
            pattern,
            bits,
            operands,
        )
        .render())
    }

    fn register_space(&mut self, space: SpaceDecl) {
        let info = SpaceInfo::from_decl(space);
        self.spaces.insert(info.name.clone(), info);
    }

    fn register_form(&mut self, form: FormDecl) -> Result<(), IsaError> {
        let space = self.spaces.get_mut(&form.space).ok_or_else(|| {
            IsaError::Machine(format!(
                "form '{}' declared for unknown space '{}'",
                form.name, form.space
            ))
        })?;
        if space.kind != SpaceKind::Logic {
            return Ok(());
        }
        space.add_form(form)
    }

    fn register_field(&mut self, field: FieldDecl) -> Result<(), IsaError> {
        let space = self.spaces.get_mut(&field.space).ok_or_else(|| {
            IsaError::Machine(format!(
                "field '{}' declared for unknown space '{}'",
                field.name, field.space
            ))
        })?;
        space.add_register_field(field);
        Ok(())
    }

    fn build_patterns(&mut self) -> Result<(), IsaError> {
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

    fn build_decode_spaces(&mut self) -> Result<(), IsaError> {
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
            return Err(IsaError::Machine("no logic spaces defined".into()));
        }

        spaces.sort_by(|a, b| {
            a.word_bits
                .cmp(&b.word_bits)
                .then_with(|| a.name.cmp(&b.name))
        });
        self.decode_spaces = spaces;
        Ok(())
    }

    fn format_operand(&self, field: &FieldEncoding, value: u64) -> String {
        if let Some(binding) = &field.register {
            if let Some(space) = self.spaces.get(&binding.space)
                && let Some(register) = space.registers.get(&binding.field)
            {
                return register.format(value);
            }
            return format!("{}{}", binding.field, value);
        }

        if field.kind == OperandKind::Immediate {
            return self.format_immediate(field, value);
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

    fn format_immediate(&self, field: &FieldEncoding, value: u64) -> String {
        let mut bits = u32::from(field.spec.data_width());
        if bits == 0 {
            bits = 1;
        }
        let digits = ((bits as usize) + 3) / 4;
        let truncated = if bits >= 64 {
            value
        } else {
            let mask = (1u64 << bits) - 1;
            value & mask
        };
        format!("0x{truncated:0digits$X}")
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
            let spec =
                match &field.selector {
                    MaskSelector::Field(name) => {
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
                        form.subfield(name).ok_or_else(|| IsaError::Machine(format!(
                        "instruction '{}' references unknown field '{}' on form '{}::{}'",
                        instr.name, name, space.name, form_name
                    )))?.spec.clone()
                    }
                    MaskSelector::BitExpr(expr) => {
                        parse_bit_spec(word_bits, expr).map_err(|err| {
                            IsaError::Machine(format!(
                                "invalid bit expression '{expr}' in instruction '{}': {err}",
                                instr.name
                            ))
                        })?
                    }
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

        let display = instr
            .display
            .clone()
            .or(form_display)
            .or_else(|| default_display_template(instr.form.as_ref(), &operand_names));

        Ok(Some(InstructionPattern {
            instruction_idx: idx,
            space: instr.space.clone(),
            form: instr.form.clone(),
            mask,
            value: value_bits,
            operand_names,
            display,
            operator: instr.operator.clone(),
            specificity: mask.count_ones(),
        }))
    }
}

#[derive(Debug, Clone)]
pub struct SpaceInfo {
    pub name: String,
    pub kind: SpaceKind,
    pub size_bits: Option<u32>,
    pub endianness: Endianness,
    pub forms: BTreeMap<String, FormInfo>,
    pub registers: BTreeMap<String, RegisterInfo>,
    pub enable: Option<SemanticExpr>,
}

impl SpaceInfo {
    fn from_decl(space: SpaceDecl) -> Self {
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

    fn word_bits(&self) -> Result<u32, IsaError> {
        self.size_bits.ok_or_else(|| {
            IsaError::Machine(format!(
                "logic space '{}' missing required word size attribute",
                self.name
            ))
        })
    }

    fn add_form(&mut self, form: FormDecl) -> Result<(), IsaError> {
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

    fn add_register_field(&mut self, field: FieldDecl) {
        if self.kind != SpaceKind::Register {
            return;
        }
        let info = RegisterInfo::from_decl(field);
        self.registers.insert(info.name.clone(), info);
    }
}

#[derive(Debug, Clone)]
pub struct RegisterInfo {
    pub name: String,
    pub range: Option<FieldIndexRange>,
    display: Option<String>,
}

impl RegisterInfo {
    fn from_decl(decl: FieldDecl) -> Self {
        Self {
            name: decl.name,
            range: decl.range,
            display: decl.display,
        }
    }

    fn format(&self, value: u64) -> String {
        if let Some(pattern) = &self.display {
            return format_register_display(pattern, value);
        }
        if self.range.is_some() {
            format!("{}{}", self.name, value)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub space: String,
    pub name: String,
    pub form: Option<String>,
    pub description: Option<String>,
    pub operands: Vec<String>,
    pub display: Option<String>,
    pub operator: Option<String>,
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
            display: decl.display,
            operator: decl.operator,
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
    pub opcode: u64,
    pub mnemonic: String,
    pub operands: Vec<String>,
    pub display: Option<String>,
}

#[derive(Debug, Clone)]
struct LogicDecodeSpace {
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

#[derive(Debug, Clone)]
struct InstructionPattern {
    instruction_idx: usize,
    space: String,
    form: Option<String>,
    mask: u64,
    value: u64,
    operand_names: Vec<String>,
    display: Option<String>,
    operator: Option<String>,
    specificity: u32,
}

#[derive(Debug, Clone)]
pub struct FormInfo {
    fields: Vec<FieldEncoding>,
    field_index: BTreeMap<String, usize>,
    operand_order: Vec<String>,
    display: Option<String>,
}

impl FormInfo {
    fn new(_name: String) -> Self {
        Self {
            fields: Vec::new(),
            field_index: BTreeMap::new(),
            operand_order: Vec::new(),
            display: None,
        }
    }

    fn contains(&self, name: &str) -> bool {
        self.field_index.contains_key(name)
    }

    fn push_field(&mut self, field: FieldEncoding) {
        if !field.is_function_only() {
            self.operand_order.push(field.name.clone());
        }
        self.field_index
            .insert(field.name.clone(), self.fields.len());
        self.fields.push(field);
    }

    fn subfield(&self, name: &str) -> Option<&FieldEncoding> {
        self.field_index
            .get(name)
            .and_then(|index| self.fields.get(*index))
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
    fn is_function_only(&self) -> bool {
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

fn derive_register_binding(ops: &[SubFieldOp]) -> Option<RegisterBinding> {
    ops.iter().find_map(parse_register_op)
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

fn default_display_template(form: Option<&String>, operands: &[String]) -> Option<String> {
    if form.is_none() || operands.is_empty() {
        return None;
    }
    let parts: Vec<String> = operands.iter().map(|name| format!("#{name}")).collect();
    Some(parts.join(", "))
}

fn parse_register_op(op: &SubFieldOp) -> Option<RegisterBinding> {
    if let Some(binding) = parse_context_style_register(op) {
        return Some(binding);
    }
    if op.kind.eq_ignore_ascii_case("reg") {
        if let Some(field) = &op.subtype {
            return Some(RegisterBinding {
                space: "reg".into(),
                field: field.clone(),
            });
        }
    }
    None
}

fn parse_context_style_register(op: &SubFieldOp) -> Option<RegisterBinding> {
    if !op.kind.starts_with('$') {
        return None;
    }
    let mut segments: Vec<&str> = op.kind.split("::").collect();
    if segments.len() < 2 {
        return None;
    }
    let space = segments.remove(0).trim_start_matches('$');
    let field = segments.remove(0);
    if space.is_empty() || field.is_empty() {
        return None;
    }
    Some(RegisterBinding {
        space: space.to_string(),
        field: field.to_string(),
    })
}

#[derive(Debug, Clone)]
pub struct RegisterBinding {
    pub space: String,
    pub field: String,
}

impl Instruction {}

struct DisplayRenderer<'a> {
    machine: &'a MachineDescription,
    template: &'a str,
    form: &'a FormInfo,
    pattern: &'a InstructionPattern,
    bits: u64,
    operands: &'a [String],
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
            template,
            form,
            pattern,
            bits,
            operands,
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
        Some(self.machine.format_operand(field, value))
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

fn ensure_byte_aligned(word_bits: u32, instr: &str) -> Result<usize, IsaError> {
    if word_bits % 8 != 0 {
        return Err(IsaError::Machine(format!(
            "instruction '{}' width ({word_bits} bits) is not byte-aligned",
            instr
        )));
    }
    Ok((word_bits / 8) as usize)
}

fn mask_for_bits(bits: u32) -> u64 {
    if bits >= 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    }
}

fn format_register_display(pattern: &str, value: u64) -> String {
    let mut result = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            result.push(ch);
            continue;
        }

        if matches!(chars.peek(), Some('%')) {
            chars.next();
            result.push('%');
            continue;
        }

        if let Some(fragment) = next_display_fragment(&mut chars, value) {
            result.push_str(&fragment);
        } else {
            result.push('%');
        }
    }
    result
}

fn next_display_fragment(iter: &mut Peekable<Chars<'_>>, value: u64) -> Option<String> {
    let mut zero_pad = false;
    let mut width_digits = String::new();

    while let Some(&ch) = iter.peek() {
        if ch == '0' && width_digits.is_empty() {
            zero_pad = true;
            iter.next();
            continue;
        }
        if ch.is_ascii_digit() {
            width_digits.push(ch);
            iter.next();
        } else {
            break;
        }
    }

    let width = if width_digits.is_empty() {
        None
    } else {
        width_digits.parse().ok()
    };

    let spec = iter.next()?;
    Some(match spec {
        'd' | 'u' => format_number(value, width, zero_pad, NumberFormat::Decimal),
        'x' => format_number(value, width, zero_pad, NumberFormat::HexLower),
        'X' => format_number(value, width, zero_pad, NumberFormat::HexUpper),
        '%' => "%".into(),
        other => {
            let mut literal = String::from("%");
            if zero_pad {
                literal.push('0');
            }
            if let Some(w) = width {
                literal.push_str(&w.to_string());
            }
            literal.push(other);
            literal
        }
    })
}

#[derive(Clone, Copy)]
enum NumberFormat {
    Decimal,
    HexLower,
    HexUpper,
}

fn format_number(value: u64, width: Option<usize>, zero_pad: bool, format: NumberFormat) -> String {
    match format {
        NumberFormat::Decimal => match (width, zero_pad) {
            (Some(w), true) => format!("{value:0width$}", width = w),
            (Some(w), false) => format!("{value:width$}", width = w),
            (None, _) => format!("{value}"),
        },
        NumberFormat::HexLower => match (width, zero_pad) {
            (Some(w), true) => format!("{value:0width$x}", width = w),
            (Some(w), false) => format!("{value:width$x}", width = w),
            (None, _) => format!("{value:x}"),
        },
        NumberFormat::HexUpper => match (width, zero_pad) {
            (Some(w), true) => format!("{value:0width$X}", width = w),
            (Some(w), false) => format!("{value:width$X}", width = w),
            (None, _) => format!("{value:X}"),
        },
    }
}

fn parse_bit_spec(word_bits: u32, spec: &str) -> Result<BitFieldSpec, BitFieldSpecParseError> {
    let container = u16::try_from(word_bits).map_err(|_| BitFieldSpecParseError::TooWide)?;
    BitFieldSpec::from_spec_str(TypeId::from_index(0), container, spec)
        .map_err(BitFieldSpecParseError::SpecError)
}

fn encode_constant(spec: &BitFieldSpec, value: u64) -> Result<(u64, u64), BitFieldSpecParseError> {
    let mask = spec
        .segments
        .iter()
        .fold(0u64, |acc, segment| match segment {
            BitFieldSegment::Slice(slice) => acc | slice.mask,
            BitFieldSegment::Literal { .. } => acc,
        });
    let encoded = spec
        .write_bits(0, value)
        .map_err(BitFieldSpecParseError::SpecError)?;
    Ok((mask, encoded & mask))
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

#[derive(Debug)]
enum BitFieldSpecParseError {
    TooWide,
    SpecError(crate::soc::prog::types::bitfield::BitFieldError),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::isa::ast::{SpaceAttribute, SpaceKind, SubFieldDecl};
    use crate::soc::isa::builder::{IsaBuilder, mask_field_selector, subfield_op};

    #[test]
    fn lifter_decodes_simple_logic_space() {
        let mut builder = IsaBuilder::new("lift.isa");
        builder.add_space(
            "test",
            SpaceKind::Logic,
            vec![
                SpaceAttribute::WordSize(8),
                SpaceAttribute::Endianness(Endianness::Big),
            ],
        );
        builder.add_form(
            "test",
            "BASE",
            None,
            vec![
                SubFieldDecl {
                    name: "OPC".into(),
                    bit_spec: "@(0..3)".into(),
                    operations: vec![subfield_op("func", None::<&str>)],
                    description: None,
                },
                SubFieldDecl {
                    name: "DST".into(),
                    bit_spec: "@(4..7)".into(),
                    operations: vec![
                        subfield_op("target", None::<&str>),
                        subfield_op("reg", Some("GPR")),
                    ],
                    description: None,
                },
            ],
        );
        builder
            .instruction("test", "mov")
            .form("BASE")
            .mask_field(mask_field_selector("OPC"), 0xA)
            .finish();
        let doc = builder.build();
        let machine = MachineDescription::from_documents(vec![doc]).expect("machine");
        let bytes = [0xA5u8];
        let listing = machine.disassemble_from(&bytes, 0x1000);
        assert_eq!(listing.len(), 1);
        let entry = &listing[0];
        assert_eq!(entry.address, 0x1000);
        assert_eq!(entry.mnemonic, "mov");
        assert_eq!(entry.operands, vec!["GPR5".to_string()]);
        assert_eq!(entry.opcode, 0xA5);
    }

    #[test]
    fn display_templates_apply_form_defaults_and_overrides() {
        let mut builder = IsaBuilder::new("display.isa");
        builder.add_space(
            "logic",
            SpaceKind::Logic,
            vec![
                SpaceAttribute::WordSize(8),
                SpaceAttribute::Endianness(Endianness::Big),
            ],
        );
        builder.add_form_with_display(
            "logic",
            "BIN",
            None,
            Some("#RT <- #RA #op #RB".into()),
            vec![
                SubFieldDecl {
                    name: "OPC".into(),
                    bit_spec: "@(0..1)".into(),
                    operations: vec![subfield_op("func", None::<&str>)],
                    description: None,
                },
                SubFieldDecl {
                    name: "RT".into(),
                    bit_spec: "@(2..3)".into(),
                    operations: vec![
                        subfield_op("target", None::<&str>),
                        subfield_op("reg", Some("GPR")),
                    ],
                    description: None,
                },
                SubFieldDecl {
                    name: "RA".into(),
                    bit_spec: "@(4..5)".into(),
                    operations: vec![
                        subfield_op("source", None::<&str>),
                        subfield_op("reg", Some("GPR")),
                    ],
                    description: None,
                },
                SubFieldDecl {
                    name: "RB".into(),
                    bit_spec: "@(6..7)".into(),
                    operations: vec![
                        subfield_op("source", None::<&str>),
                        subfield_op("reg", Some("GPR")),
                    ],
                    description: None,
                },
            ],
        );
        builder
            .instruction("logic", "add")
            .form("BIN")
            .operator("+")
            .mask_field(mask_field_selector("OPC"), 0)
            .finish();
        builder
            .instruction("logic", "swap")
            .form("BIN")
            .display("#RT <-> #RA")
            .mask_field(mask_field_selector("OPC"), 1)
            .finish();

        let machine = MachineDescription::from_documents(vec![builder.build()]).expect("machine");
        let bytes = [0x1Bu8, 0x4Eu8];
        let listing = machine.disassemble(&bytes);
        assert_eq!(listing.len(), 2);
        assert_eq!(listing[0].mnemonic, "add");
        assert_eq!(
            listing[0].operands,
            vec!["GPR1".to_string(), "GPR2".to_string(), "GPR3".to_string()]
        );
        assert_eq!(
            listing[0].display.as_deref(),
            Some("GPR1 <- GPR2 + GPR3")
        );

        assert_eq!(listing[1].mnemonic, "swap");
        assert_eq!(
            listing[1].operands,
            vec!["GPR0".to_string(), "GPR3".to_string(), "GPR2".to_string()]
        );
        assert_eq!(listing[1].display.as_deref(), Some("GPR0 <-> GPR3"));
    }

    #[test]
    fn immediate_operands_render_in_hex() {
        let mut builder = IsaBuilder::new("imm.isa");
        builder.add_space(
            "logic",
            SpaceKind::Logic,
            vec![
                SpaceAttribute::WordSize(16),
                SpaceAttribute::Endianness(Endianness::Big),
            ],
        );
        builder.add_form(
            "logic",
            "IMM",
            None,
            vec![
                SubFieldDecl {
                    name: "OPC".into(),
                    bit_spec: "@(0..3)".into(),
                    operations: vec![subfield_op("func", None::<&str>)],
                    description: None,
                },
                SubFieldDecl {
                    name: "SIMM".into(),
                    bit_spec: "@(4..15)".into(),
                    operations: vec![subfield_op("immediate", None::<&str>)],
                    description: None,
                },
            ],
        );
        builder
            .instruction("logic", "addi")
            .form("IMM")
            .mask_field(mask_field_selector("OPC"), 0xA)
            .finish();

        let machine = MachineDescription::from_documents(vec![builder.build()]).expect("machine");
        let bytes = [0xA1u8, 0x23u8];
        let listing = machine.disassemble(&bytes);
        assert_eq!(listing.len(), 1);
        assert_eq!(listing[0].mnemonic, "addi");
        assert_eq!(listing[0].operands, vec!["0x123".to_string()]);
    }

    #[test]
    fn default_display_lists_non_func_operands() {
        let mut builder = IsaBuilder::new("default_disp.isa");
        builder.add_space(
            "logic",
            SpaceKind::Logic,
            vec![
                SpaceAttribute::WordSize(8),
                SpaceAttribute::Endianness(Endianness::Big),
            ],
        );
        builder.add_form(
            "logic",
            "RAW",
            None,
            vec![
                SubFieldDecl {
                    name: "OPC".into(),
                    bit_spec: "@(0..1)".into(),
                    operations: vec![subfield_op("func", None::<&str>)],
                    description: None,
                },
                SubFieldDecl {
                    name: "RT".into(),
                    bit_spec: "@(2..3)".into(),
                    operations: vec![
                        subfield_op("target", None::<&str>),
                        subfield_op("reg", Some("GPR")),
                    ],
                    description: None,
                },
                SubFieldDecl {
                    name: "RA".into(),
                    bit_spec: "@(4..5)".into(),
                    operations: vec![
                        subfield_op("source", None::<&str>),
                        subfield_op("reg", Some("GPR")),
                    ],
                    description: None,
                },
                SubFieldDecl {
                    name: "RB".into(),
                    bit_spec: "@(6..7)".into(),
                    operations: vec![
                        subfield_op("source", None::<&str>),
                        subfield_op("reg", Some("GPR")),
                    ],
                    description: None,
                },
            ],
        );
        builder
            .instruction("logic", "copy")
            .form("RAW")
            .mask_field(mask_field_selector("OPC"), 0)
            .finish();

        let machine = MachineDescription::from_documents(vec![builder.build()]).expect("machine");
        let bytes = [0x1Bu8];
        let listing = machine.disassemble(&bytes);
        assert_eq!(listing.len(), 1);
        let entry = &listing[0];
        assert_eq!(entry.mnemonic, "copy");
        assert_eq!(
            entry.operands,
            vec!["GPR1".to_string(), "GPR2".to_string(), "GPR3".to_string()]
        );
        assert_eq!(entry.display.as_deref(), Some("GPR1, GPR2, GPR3"));
    }

    #[test]
    fn xo_masks_overlap() {
        let xo = parse_bit_spec(32, "@(21..30)").expect("xo spec");
        let oe = parse_bit_spec(32, "@(21)").expect("oe spec");
        let (xo_mask, xo_bits) = encode_constant(&xo, 266).expect("xo encode");
        let (oe_mask, oe_bits) = encode_constant(&oe, 1).expect("oe encode");
        // PowerPC addo encodings set OE separately even though it's part of XO.
        // This asserts that our BitField encoding indeed produces conflicting bits,
        // justifying the override behavior in `build_pattern`.
        assert_eq!(xo_mask & oe_mask, oe_mask);
        assert_eq!(oe_bits, oe_mask);
        assert_eq!(xo_bits & oe_mask, 0);
    }
}
