//! Register metadata helpers for the machine module. Defines register binding
//! declarations, formatting helpers, and register-space utilities.

use std::collections::BTreeMap;
use std::fmt;
use std::iter::Peekable;
use std::str::Chars;
use std::sync::Arc;

use crate::soc::device::Endianness;
use crate::soc::isa::ast::{
    ContextReference, FieldDecl, FieldIndexRange, SpaceKind, SubFieldDecl, SubFieldOp,
};
use crate::soc::isa::error::IsaError;
use crate::soc::prog::symbols::{StorageClass, SymbolHandle, SymbolKind, SymbolTable};
use crate::soc::prog::types::{BitFieldSpec, TypeArena, TypeBuilder, TypeId};

use super::space::SpaceInfo;
use crate::soc::isa::space::resolve_reference_path;

const DEFAULT_REGISTER_BITS: u32 = 64;
const MAX_REGISTER_BITS: u32 = 64;

#[derive(Debug, Clone)]
pub struct RegisterInfo {
    pub name: String,
    pub range: Option<FieldIndexRange>,
    pub size_bits: Option<u32>,
    pub offset: Option<u64>,
    pub description: Option<String>,
    pub redirect: Option<ContextReference>,
    pub subfields: Vec<SubFieldDecl>,
    display: Option<String>,
    type_handles: Option<RegisterTypeHandles>,
}

impl RegisterInfo {
    pub fn from_decl(decl: FieldDecl) -> Self {
        Self {
            name: decl.name,
            range: decl.range,
            size_bits: decl.size,
            offset: decl.offset,
            description: decl.description,
            redirect: decl.redirect,
            subfields: decl.subfields,
            display: decl.display,
            type_handles: None,
        }
    }

    pub fn with_size(name: impl Into<String>, size_bits: Option<u32>) -> Self {
        Self {
            name: name.into(),
            range: None,
            size_bits,
            offset: None,
            description: None,
            redirect: None,
            subfields: Vec::new(),
            display: None,
            type_handles: None,
        }
    }

    pub fn size_bits(&self) -> Option<u32> {
        self.size_bits
    }

    pub fn type_handles(&self) -> Option<RegisterTypeHandles> {
        self.type_handles
    }

    pub fn set_type_handles(&mut self, handles: RegisterTypeHandles) {
        self.type_handles = Some(handles);
    }

    pub fn format(&self, value: u64) -> String {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterTypeHandles {
    pub structure: TypeId,
    pub array: TypeId,
}

#[derive(Debug, Clone)]
pub struct RegisterBinding {
    pub space: String,
    pub field: String,
}

pub(super) fn derive_register_binding(ops: &[SubFieldOp]) -> Option<RegisterBinding> {
    ops.iter().find_map(parse_register_op)
}

fn parse_register_op(op: &SubFieldOp) -> Option<RegisterBinding> {
    if let Some(binding) = parse_context_style_register(op) {
        return Some(binding);
    }
    if let Some(field) = op
        .subtype
        .as_ref()
        .filter(|_| op.kind.eq_ignore_ascii_case("reg"))
    {
        return Some(RegisterBinding {
            space: "reg".into(),
            field: field.clone(),
        });
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

pub struct RegisterSchema {
    types: Arc<TypeArena>,
    table: SymbolTable,
    registers: BTreeMap<RegisterKey, RegisterMetadata>,
    alias_fields: BTreeMap<RegisterKey, Vec<RegisterFieldMetadata>>,
}

impl fmt::Debug for RegisterSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisterSchema")
            .field("register_count", &self.registers.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RegisterKey {
    space: String,
    name: String,
}

#[derive(Debug, Clone)]
pub struct RegisterMetadata {
    pub space: String,
    pub name: String,
    pub description: Option<String>,
    pub bit_width: u32,
    pub count: u32,
    pub byte_order: Endianness,
    pub structure: TypeId,
    pub array: TypeId,
    pub symbol: SymbolHandle,
    pub elements: Vec<RegisterElement>,
    pub fields: Vec<RegisterFieldMetadata>,
}

#[derive(Debug, Clone)]
pub struct RegisterElement {
    pub index: u32,
    pub label: String,
    pub symbol: SymbolHandle,
}

#[derive(Debug, Clone)]
pub struct RegisterFieldMetadata {
    pub name: String,
    pub handle: ScalarFieldHandle,
}

#[derive(Debug, Clone, Copy)]
pub struct ScalarFieldHandle {
    pub owner: TypeId,
    pub field_index: u16,
}

impl RegisterSchema {
    pub fn empty() -> Self {
        let types = Arc::new(TypeArena::new());
        let table = SymbolTable::new(Arc::clone(&types));
        Self {
            types,
            table,
            registers: BTreeMap::new(),
            alias_fields: BTreeMap::new(),
        }
    }

    pub fn build(spaces: &mut BTreeMap<String, SpaceInfo>) -> Result<Self, IsaError> {
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let mut pending: Vec<PendingRegister> = Vec::new();
        let mut alias_fields: BTreeMap<RegisterKey, Vec<RegisterFieldMetadata>> = BTreeMap::new();

        for (space_name, space) in spaces.iter_mut() {
            if space.kind != SpaceKind::Register {
                continue;
            }
            let default_bits = space.size_bits.unwrap_or(DEFAULT_REGISTER_BITS);
            for info in space.registers.values_mut() {
                if info.redirect.is_some() {
                    continue;
                }
                let entry = build_register_entry(
                    &mut builder,
                    space_name,
                    space.endianness,
                    default_bits,
                    info,
                )?;
                info.set_type_handles(RegisterTypeHandles {
                    structure: entry.structure,
                    array: entry.array,
                });
                pending.push(entry);
            }
        }

        for (space_name, space) in spaces.iter() {
            if space.kind != SpaceKind::Register {
                continue;
            }
            for info in space.registers.values() {
                if let Some(reference) = &info.redirect {
                    if info.subfields.is_empty() {
                        continue;
                    }
                    let fields =
                        build_alias_fields(&mut builder, spaces, space_name, reference, info)?;
                    alias_fields.insert(RegisterKey::new(space_name, &info.name), fields);
                }
            }
        }

        let types = Arc::new(arena);
        let mut table = SymbolTable::new(Arc::clone(&types));
        let mut registers = BTreeMap::new();

        for entry in pending {
            let metadata = entry.commit(&mut table)?;
            registers.insert(RegisterKey::new(&metadata.space, &metadata.name), metadata);
        }

        Ok(Self {
            types,
            table,
            registers,
            alias_fields,
        })
    }

    pub fn symbol_table(&self) -> &SymbolTable {
        &self.table
    }

    pub fn type_arena(&self) -> &Arc<TypeArena> {
        &self.types
    }

    pub fn lookup(&self, space: &str, name: &str) -> Option<&RegisterMetadata> {
        self.registers.get(&RegisterKey::new(space, name))
    }

    pub fn alias_fields(&self, space: &str, name: &str) -> Option<&Vec<RegisterFieldMetadata>> {
        self.alias_fields.get(&RegisterKey::new(space, name))
    }

    /// Finds the register metadata/element pair that owns the provided label inside a space.
    pub fn find_by_label(
        &self,
        space: &str,
        label: &str,
    ) -> Option<(&RegisterMetadata, &RegisterElement)> {
        for metadata in self.registers.values() {
            if metadata.space != space {
                continue;
            }
            if let Some(element) = metadata
                .elements
                .iter()
                .find(|element| element.label == label)
            {
                return Some((metadata, element));
            }
        }
        None
    }
}

impl RegisterKey {
    fn new(space: &str, name: &str) -> Self {
        Self {
            space: space.to_string(),
            name: name.to_string(),
        }
    }
}

struct PendingRegister {
    key: RegisterKey,
    description: Option<String>,
    bit_width: u32,
    count: u32,
    byte_order: Endianness,
    structure: TypeId,
    array: TypeId,
    fields: Vec<RegisterFieldMetadata>,
    elements: Vec<PendingElement>,
}

struct PendingElement {
    index: u32,
    label: String,
}

impl PendingRegister {
    fn commit(self, table: &mut SymbolTable) -> Result<RegisterMetadata, IsaError> {
        let mut base_builder = table.builder();
        base_builder = base_builder
            .label(&self.key.name)
            .type_id(self.array)
            .kind(SymbolKind::Metadata)
            .storage(StorageClass::METADATA)
            .byte_order(self.byte_order);
        if let Some(desc) = &self.description {
            base_builder = base_builder.description(desc);
        }
        let bytes_per_element = element_bytes(self.bit_width as usize);
        let total_bytes = bytes_per_element
            .checked_mul(self.count as usize)
            .ok_or_else(|| IsaError::Machine("register byte size overflow".into()))?;
        base_builder = base_builder.size(total_bytes);
        let base_symbol = base_builder.finish();

        let mut elements = Vec::with_capacity(self.elements.len());
        for element in self.elements {
            let mut elem_builder = table.builder();
            elem_builder = elem_builder
                .label(&element.label)
                .type_id(self.structure)
                .kind(SymbolKind::Metadata)
                .storage(StorageClass::METADATA)
                .byte_order(self.byte_order)
                .size(bytes_per_element);
            let symbol = elem_builder.finish();
            elements.push(RegisterElement {
                index: element.index,
                label: element.label,
                symbol,
            });
        }

        Ok(RegisterMetadata {
            space: self.key.space,
            name: self.key.name,
            description: self.description,
            bit_width: self.bit_width,
            count: self.count,
            byte_order: self.byte_order,
            structure: self.structure,
            array: self.array,
            symbol: base_symbol,
            elements,
            fields: self.fields,
        })
    }
}

fn build_register_entry(
    builder: &mut TypeBuilder<'_>,
    space_name: &str,
    byte_order: Endianness,
    default_bits: u32,
    info: &RegisterInfo,
) -> Result<PendingRegister, IsaError> {
    let store_bitw = info.size_bits.unwrap_or(default_bits);
    if store_bitw == 0 {
        return Err(IsaError::Machine(format!(
            "register '{}::{}' must declare a positive bit width",
            space_name, info.name
        )));
    }
    if store_bitw > MAX_REGISTER_BITS {
        return Err(IsaError::Machine(format!(
            "register '{}::{}' width {} exceeds supported limit of {} bits",
            space_name, info.name, store_bitw, MAX_REGISTER_BITS
        )));
    }
    let (fields, structure) = build_register_fields(builder, store_bitw, space_name, info)?;
    let count = register_count(info);
    let stride_bytes = element_bytes(store_bitw as usize);
    let array = builder.sequence_static(structure, stride_bytes, count as usize);
    let elements = materialize_elements(info);

    Ok(PendingRegister {
        key: RegisterKey::new(space_name, &info.name),
        description: info.description.clone(),
        bit_width: store_bitw,
        count,
        byte_order,
        structure,
        array,
        fields,
        elements,
    })
}

fn build_alias_fields(
    builder: &mut TypeBuilder<'_>,
    spaces: &BTreeMap<String, SpaceInfo>,
    alias_space: &str,
    reference: &ContextReference,
    alias_info: &RegisterInfo,
) -> Result<Vec<RegisterFieldMetadata>, IsaError> {
    if alias_info.subfields.is_empty() {
        return Ok(Vec::new());
    }
    let (target_space, path) = resolve_reference_path(alias_space, reference);
    let Some(target_name) = path.first() else {
        return Err(IsaError::Machine(format!(
            "redirect for '{}::{}' is missing a target register",
            alias_space, alias_info.name
        )));
    };
    if path.len() > 1 {
        return Err(IsaError::Machine(format!(
            "redirect for '{}::{}' cannot reference subfields",
            alias_space, alias_info.name
        )));
    }
    let store_bitw = resolve_register_bit_width(spaces, &target_space, target_name)?;
    let entries = collect_subfield_specs(
        store_bitw,
        alias_space,
        &alias_info.name,
        &alias_info.subfields,
    )?;
    let (fields, _) = finish_scalar_fields(builder, store_bitw, entries);
    Ok(fields)
}

fn resolve_register_bit_width(
    spaces: &BTreeMap<String, SpaceInfo>,
    space_name: &str,
    target: &str,
) -> Result<u32, IsaError> {
    let space = spaces.get(space_name).ok_or_else(|| {
        IsaError::Machine(format!(
            "redirect references undefined space '{}'",
            space_name
        ))
    })?;
    let default_bits = space.size_bits.unwrap_or(DEFAULT_REGISTER_BITS);
    if let Some(info) = space.registers.get(target) {
        return Ok(info.size_bits.unwrap_or(default_bits));
    }
    for info in space.registers.values() {
        if info
            .range
            .as_ref()
            .is_some_and(|range| register_label_matches(info, range, target))
        {
            return Ok(info.size_bits.unwrap_or(default_bits));
        }
    }
    Err(IsaError::Machine(format!(
        "redirect references undefined register '{}::{}'",
        space_name, target
    )))
}

fn register_label_matches(info: &RegisterInfo, range: &FieldIndexRange, label: &str) -> bool {
    for index in range.start..=range.end {
        if info.format(index as u64) == label {
            return true;
        }
    }
    false
}

fn build_register_fields(
    builder: &mut TypeBuilder<'_>,
    store_bitw: u32,
    space: &str,
    info: &RegisterInfo,
) -> Result<(Vec<RegisterFieldMetadata>, TypeId), IsaError> {
    let entries = if info.subfields.is_empty() {
        vec![(
            "VALUE".into(),
            BitFieldSpec::builder(store_bitw as u16).finish(),
        )]
    } else {
        collect_subfield_specs(store_bitw, space, &info.name, &info.subfields)?
    };
    Ok(finish_scalar_fields(builder, store_bitw, entries))
}

fn register_count(info: &RegisterInfo) -> u32 {
    info.range
        .as_ref()
        .map(|range| (range.end - range.start) + 1)
        .unwrap_or(1)
}

fn materialize_elements(info: &RegisterInfo) -> Vec<PendingElement> {
    match info.range.as_ref() {
        Some(range) => {
            let mut elements = Vec::with_capacity((range.end - range.start + 1) as usize);
            for index in range.start..=range.end {
                elements.push(PendingElement {
                    index,
                    label: info.format(index as u64),
                });
            }
            elements
        }
        None => vec![PendingElement {
            index: 0,
            label: info.format(0),
        }],
    }
}

fn element_bytes(bit_width: usize) -> usize {
    bit_width.div_ceil(8).max(1)
}

fn collect_subfield_specs(
    store_bitw: u32,
    space: &str,
    register: &str,
    subfields: &[SubFieldDecl],
) -> Result<Vec<(String, BitFieldSpec)>, IsaError> {
    let mut entries = Vec::with_capacity(subfields.len());
    for sub in subfields {
        let spec =
            BitFieldSpec::from_spec_str(store_bitw as u16, &sub.bit_spec).map_err(|err| {
                IsaError::Machine(format!(
                    "invalid bit spec '{}' on register '{}::{}::{}': {err}",
                    sub.bit_spec, space, register, sub.name
                ))
            })?;
        entries.push((sub.name.clone(), spec));
    }
    Ok(entries)
}

fn finish_scalar_fields(
    builder: &mut TypeBuilder<'_>,
    store_bitw: u32,
    entries: Vec<(String, BitFieldSpec)>,
) -> (Vec<RegisterFieldMetadata>, TypeId) {
    let storage_bytes = element_bytes(store_bitw as usize);
    let mut scalar = builder.scalar_with_fields(storage_bytes);
    let mut names = Vec::with_capacity(entries.len());
    for (name, spec) in entries {
        let name_id = builder.intern(&name);
        scalar.push_spec(name_id, spec);
        names.push(name);
    }
    let owner = scalar.finish_with(builder);
    let fields = names
        .into_iter()
        .enumerate()
        .map(|(idx, name)| RegisterFieldMetadata {
            name,
            handle: ScalarFieldHandle {
                owner,
                field_index: idx as u16,
            },
        })
        .collect();
    (fields, owner)
}
