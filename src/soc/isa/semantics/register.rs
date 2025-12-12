use std::borrow::Cow;
use std::convert::TryFrom;

use crate::soc::core::state::{CoreState, StateError};
use crate::soc::isa::ast::ContextReference;
use crate::soc::isa::error::IsaError;
use crate::soc::isa::machine::{
    MachineDescription, RegisterElement, RegisterFieldMetadata, RegisterMetadata, RegisterSchema,
};
use crate::soc::isa::semantics::program::RegisterRef;
use crate::soc::prog::types::arena::TypeArena;
use crate::soc::prog::types::arena_record::TypeRecord;
use crate::soc::prog::types::bitfield::BitFieldSpec;

use crate::soc::isa::semantics::value::SemanticValue;

/// Resolves `$reg::` references into concrete metadata and offers read/write helpers.
pub struct RegisterAccess<'machine> {
    machine: &'machine MachineDescription,
    schema: &'machine RegisterSchema,
    arena: &'machine TypeArena,
}

impl<'machine> RegisterAccess<'machine> {
    pub(crate) fn new(machine: &'machine MachineDescription) -> Self {
        let schema = machine.register_schema();
        Self {
            machine,
            schema,
            arena: schema.type_arena().as_ref(),
        }
    }

    /// Resolves a parsed register reference using the provided (already evaluated) index value.
    pub fn resolve(
        &'machine self,
        reference: &RegisterRef,
        evaluated_index: Option<i64>,
    ) -> Result<ResolvedRegister<'machine>, IsaError> {
        let alias_fields = self.schema.alias_fields(&reference.space, &reference.name);
        if let Some(resolved) = self.try_resolve_direct(
            &reference.space,
            &reference.name,
            reference.subfield.as_deref(),
            evaluated_index,
            alias_fields,
            None,
        )? {
            return Ok(resolved);
        }

        if let Some((target_space, target_name)) =
            self.follow_redirect(&reference.space, &reference.name)?
        {
            let alias_display = format!("{}::{}", reference.space, reference.name);
            if let Some(resolved) = self.try_resolve_direct(
                &target_space,
                &target_name,
                reference.subfield.as_deref(),
                evaluated_index,
                alias_fields,
                Some(alias_display.as_str()),
            )? {
                return Ok(resolved);
            }
        }

        Err(IsaError::Machine(format!(
            "unknown register '{}::{}'",
            reference.space, reference.name
        )))
    }

    fn try_resolve_direct(
        &'machine self,
        space: &str,
        name: &str,
        subfield: Option<&str>,
        evaluated_index: Option<i64>,
        alias_fields: Option<&'machine Vec<RegisterFieldMetadata>>,
        display_name: Option<&str>,
    ) -> Result<Option<ResolvedRegister<'machine>>, IsaError> {
        if let Some(metadata) = self.schema.lookup(space, name) {
            let element = self.select_element(metadata, name, evaluated_index)?;
            let field = self.select_field(metadata, subfield, name, alias_fields)?;
            return Ok(Some(ResolvedRegister::new(
                metadata,
                element,
                field,
                self.arena,
                display_name.map(|value| value.to_string()),
            )));
        }

        if let Some((metadata, element)) = self.schema.find_by_label(space, name) {
            if let Some(index) = evaluated_index
                && index != element.index as i64
            {
                return Err(IsaError::Machine(format!(
                    "register '{}' already selects element '{}' and cannot mix with index {}",
                    name, element.label, index
                )));
            }
            let field = self.select_field(metadata, subfield, name, alias_fields)?;
            return Ok(Some(ResolvedRegister::new(
                metadata,
                element,
                field,
                self.arena,
                display_name.map(|value| value.to_string()),
            )));
        }

        Ok(None)
    }

    fn select_element<'schema>(
        &self,
        metadata: &'schema RegisterMetadata,
        register_name: &str,
        evaluated_index: Option<i64>,
    ) -> Result<&'schema RegisterElement, IsaError> {
        if metadata.count <= 1 {
            if let Some(index) = evaluated_index
                && index != 0
            {
                return Err(IsaError::Machine(format!(
                    "register '{}' has a single element and cannot use index {index}",
                    register_name
                )));
            }
            return metadata.elements.first().ok_or_else(|| {
                IsaError::Machine(format!(
                    "register '{}' is missing element metadata",
                    register_name
                ))
            });
        }

        let index = evaluated_index.ok_or_else(|| {
            IsaError::Machine(format!(
                "register '{}' requires an index expression",
                register_name
            ))
        })?;
        if index < 0 {
            return Err(IsaError::Machine(format!(
                "register '{}' index {index} must be non-negative",
                register_name
            )));
        }
        let index = u32::try_from(index).map_err(|_| {
            IsaError::Machine(format!(
                "register '{}' index exceeds supported range",
                register_name
            ))
        })?;
        metadata
            .elements
            .iter()
            .find(|element| element.index == index)
            .ok_or_else(|| {
                IsaError::Machine(format!(
                    "register '{}' index {index} out of range",
                    register_name
                ))
            })
    }

    fn select_field<'schema>(
        &self,
        metadata: &'schema RegisterMetadata,
        subfield: Option<&str>,
        register_name: &str,
        alias_fields: Option<&'schema Vec<RegisterFieldMetadata>>,
    ) -> Result<Option<&'schema RegisterFieldMetadata>, IsaError> {
        if let Some(name) = subfield {
            let field = metadata
                .fields
                .iter()
                .find(|field| field.name.eq_ignore_ascii_case(name))
                .or_else(|| {
                    alias_fields.and_then(|fields| {
                        fields
                            .iter()
                            .find(|field| field.name.eq_ignore_ascii_case(name))
                    })
                })
                .ok_or_else(|| {
                    IsaError::Machine(format!(
                        "register '{}::{}' has no subfield '{}'",
                        metadata.space, register_name, name
                    ))
                })?;
            Ok(Some(field))
        } else {
            Ok(None)
        }
    }

    fn follow_redirect(
        &self,
        start_space: &str,
        start_name: &str,
    ) -> Result<Option<(String, String)>, IsaError> {
        let mut current_space = Cow::Borrowed(start_space);
        let mut current_name = Cow::Borrowed(start_name);
        let mut visited = 0;

        loop {
            let Some(space) = self.machine.spaces.get(current_space.as_ref()) else {
                return Ok(None);
            };
            let Some(register) = space.registers.get(current_name.as_ref()) else {
                if visited == 0 {
                    return Ok(None);
                }
                return Ok(Some((
                    current_space.into_owned(),
                    current_name.into_owned(),
                )));
            };
            let Some(reference) = &register.redirect else {
                if visited == 0 {
                    return Ok(None);
                }
                return Ok(Some((
                    current_space.into_owned(),
                    current_name.into_owned(),
                )));
            };
            visited += 1;
            if visited > 8 {
                return Err(IsaError::Machine(format!(
                    "redirect chain for '{}::{}' exceeds supported depth",
                    start_space, start_name
                )));
            }
            let (next_space, path) = resolve_reference_path(current_space.as_ref(), reference);
            let next_name = path.first().cloned().ok_or_else(|| {
                IsaError::Machine(format!(
                    "redirect for '{}::{}' is missing a target register",
                    start_space, start_name
                ))
            })?;
            if path.len() > 1 {
                return Err(IsaError::Machine(format!(
                    "redirect for '{}::{}' cannot reference subfields",
                    start_space, start_name
                )));
            }
            current_space = Cow::Owned(next_space);
            current_name = Cow::Owned(next_name);
        }
    }
}

/// Fully resolved register reference ready for read/write operations.
pub struct ResolvedRegister<'schema> {
    original_name: String,
    resolved_name: String,
    display_override: Option<String>,
    metadata: &'schema RegisterMetadata,
    element: &'schema RegisterElement,
    field: Option<&'schema RegisterFieldMetadata>,
    arena: &'schema TypeArena,
}

impl<'schema> ResolvedRegister<'schema> {
    fn new(
        metadata: &'schema RegisterMetadata,
        element: &'schema RegisterElement,
        field: Option<&'schema RegisterFieldMetadata>,
        arena: &'schema TypeArena,
        display_name: Option<String>,
    ) -> Self {
        let resolved_name = format!("{}::{}", metadata.space, element.label);
        let original_name = metadata.name.clone();
        Self {
            original_name,
            resolved_name,
            display_override: display_name,
            metadata,
            element,
            field,
            arena,
        }
    }

    pub fn read(&self, state: &mut CoreState) -> Result<SemanticValue, IsaError> {
        let raw = self.read_raw(state)?;
        if let Some(field) = self.field {
            let spec = self.field_spec(field)?;
            let value = spec.read_from(raw);
            Ok(SemanticValue::int(value as i64))
        } else {
            Ok(SemanticValue::int(raw as i64))
        }
    }

    /// Reads the raw bits backing this register or subfield without applying sign extension.
    pub fn read_bits(&self, state: &mut CoreState) -> Result<u64, IsaError> {
        if let Some(field) = self.field {
            let spec = self.field_spec(field)?;
            let container = self.read_raw(state)?;
            let (value, _) = spec.read_bits(container);
            Ok(value)
        } else {
            state
                .read_register(&self.resolved_name)
                .map_err(core_state_error)
        }
    }

    /// Writes raw bits into the register or subfield, masking to the declared width.
    pub fn write_bits(&self, state: &mut CoreState, value: u64) -> Result<(), IsaError> {
        if let Some(field) = self.field {
            let spec = self.field_spec(field)?;
            let container = self.read_raw(state)?;
            let updated = spec.write_to(container, value).map_err(|err| {
                IsaError::Machine(format!(
                    "failed to write subfield '{}::{}': {err}",
                    self.metadata.space, field.name
                ))
            })?;
            self.write_raw(state, updated)
        } else {
            if self.metadata.bit_width > 64 {
                return Err(IsaError::Machine(format!(
                    "register '{}::{}' width {} exceeds harness support",
                    self.metadata.space, self.element.label, self.metadata.bit_width
                )));
            }
            let masked = mask_to_width_u64(value, self.metadata.bit_width);
            state
                .write_register(&self.resolved_name, masked)
                .map_err(core_state_error)
        }
    }

    pub fn write(&self, state: &mut CoreState, value: i64) -> Result<(), IsaError> {
        if let Some(field) = self.field {
            let spec = self.field_spec(field)?;
            let mut container = self.read_raw(state)?;
            let updated = spec.write_to(container, value as u64).map_err(|err| {
                IsaError::Machine(format!(
                    "failed to write subfield '{}::{}': {err}",
                    self.metadata.space, field.name
                ))
            })?;
            container = updated;
            self.write_raw(state, container)
        } else {
            let masked = mask_to_width(value, self.metadata.bit_width);
            state
                .write_register(&self.resolved_name, masked)
                .map_err(core_state_error)
        }
    }

    fn read_raw(&self, state: &mut CoreState) -> Result<u64, IsaError> {
        let value = state
            .read_register(&self.resolved_name)
            .map_err(core_state_error)?;
        if self.metadata.bit_width > 64 {
            return Err(IsaError::Machine(format!(
                "register '{}::{}' exceeds 64-bit access width",
                self.metadata.space, self.element.label
            )));
        }
        Ok(value)
    }

    fn write_raw(&self, state: &mut CoreState, value: u64) -> Result<(), IsaError> {
        state
            .write_register(&self.resolved_name, value)
            .map_err(core_state_error)
    }

    fn field_spec(&self, field: &RegisterFieldMetadata) -> Result<&BitFieldSpec, IsaError> {
        match self.arena.get(field.handle.owner) {
            TypeRecord::ScalarWithFields(record) => {
                let fields = self.arena.fields(record.fields);
                let field_type = fields
                    .get(field.handle.field_index as usize)
                    .map(|entry| self.arena.get(entry.ty))
                    .ok_or_else(|| {
                        IsaError::Machine(format!(
                            "subfield '{}' exceeds declared field count",
                            field.name
                        ))
                    })?;
                match field_type {
                    TypeRecord::BitField(spec) => Ok(spec),
                    other => Err(IsaError::Machine(format!(
                        "subfield '{}' has invalid type {:?}",
                        field.name, other
                    ))),
                }
            }
            _ => Err(IsaError::Machine(format!(
                "subfield '{}' lacks scalar-with-fields metadata",
                field.name
            ))),
        }
    }

    pub fn name(&self) -> &str {
        &self.original_name
    }

    pub fn display_name(&self) -> &str {
        self.display_override
            .as_deref()
            .unwrap_or(&self.resolved_name)
    }

    pub fn resolved(&self) -> &str {
        &self.resolved_name
    }

    pub fn bit_width(&self) -> u32 {
        if let Some(field) = self.field {
            if let Ok(spec) = self.field_spec(field) {
                spec.data_width() as u32
            } else {
                self.metadata.bit_width
            }
        } else {
            self.metadata.bit_width
        }
    }
}

fn resolve_reference_path(
    current_space: &str,
    reference: &ContextReference,
) -> (String, Vec<String>) {
    if let Some(first) = reference.segments.first()
        && first.starts_with('$')
    {
        let space = first.trim_start_matches('$').to_string();
        let rest = reference.segments[1..].to_vec();
        return (space, rest);
    }
    (current_space.to_string(), reference.segments.clone())
}

fn mask_to_width(value: i64, width: u32) -> u64 {
    if width >= 64 {
        value as u64
    } else if width == 0 {
        0
    } else {
        let mask = if width == 64 {
            u64::MAX
        } else {
            (1u64 << width) - 1
        };
        (value as u64) & mask
    }
}

fn mask_to_width_u64(value: u64, width: u32) -> u64 {
    if width == 0 {
        0
    } else if width >= 64 {
        value
    } else {
        let mask = (1u64 << width) - 1;
        value & mask
    }
}

fn core_state_error(err: StateError) -> IsaError {
    IsaError::Machine(format!("core state error: {err}"))
}

#[cfg(test)]
mod tests {
    use super::RegisterAccess;
    use crate::soc::core::specification::CoreSpec;
    use crate::soc::core::state::CoreState;
    use crate::soc::device::Endianness;
    use crate::soc::isa::ast::{
        ContextReference, FieldDecl, FieldIndexRange, IsaItem, IsaSpecification, SpaceAttribute,
        SpaceDecl, SpaceKind, SpaceMember, SpaceMemberDecl, SubFieldDecl,
    };
    use crate::soc::isa::diagnostic::{SourcePosition, SourceSpan};
    use crate::soc::isa::error::IsaError;
    use crate::soc::isa::machine::MachineDescription;
    use crate::soc::isa::semantics::program::RegisterRef;
    use crate::soc::prog::types::bitfield::BitOrder;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn register_access_reads_scalar_registers() {
        let (machine, mut state) = test_machine_state();
        let access = RegisterAccess::new(&machine);
        state.write_register("reg::ACC", 0x1A2B).expect("write acc");
        let reference = RegisterRef {
            space: "reg".into(),
            name: "ACC".into(),
            subfield: None,
            index: None,
            span: None,
        };
        let resolved = access.resolve(&reference, None).expect("resolve acc");
        let value = resolved.read(&mut state).expect("read acc");
        assert_eq!(value.as_int().unwrap(), 0x1A2B);
    }

    #[test]
    fn register_access_requires_index_for_arrays() {
        let (machine, _state) = test_machine_state();
        let access = RegisterAccess::new(&machine);
        let reference = RegisterRef {
            space: "reg".into(),
            name: "GPR".into(),
            subfield: None,
            index: None,
            span: None,
        };
        let result = access.resolve(&reference, None);
        assert!(matches!(result, Err(IsaError::Machine(msg)) if msg.contains("requires an index")));
    }

    #[test]
    fn register_access_reads_array_elements() {
        let (machine, mut state) = test_machine_state();
        let access = RegisterAccess::new(&machine);
        state
            .write_register("reg::GPR1", 0xDEADBEEF)
            .expect("write gpr1");
        let reference = RegisterRef {
            space: "reg".into(),
            name: "GPR".into(),
            subfield: None,
            index: None,
            span: None,
        };
        let resolved = access
            .resolve(&reference, Some(1))
            .expect("resolve indexed register");
        let value = resolved.read(&mut state).expect("read gpr1");
        assert_eq!(value.as_int().unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn register_access_handles_subfields() {
        let (machine, mut state) = test_machine_state();
        let access = RegisterAccess::new(&machine);
        let reference = RegisterRef {
            space: "reg".into(),
            name: "FLAGS".into(),
            subfield: Some("ZERO".into()),
            index: None,
            span: None,
        };
        let resolved = access.resolve(&reference, None).expect("resolve subfield");
        resolved.write(&mut state, 1).expect("set zero flag");
        let asserted = resolved.read(&mut state).expect("read zero flag");
        assert_eq!(asserted.as_int().unwrap(), 1);
        resolved.write(&mut state, 0).expect("clear zero flag");
        let cleared = resolved.read(&mut state).expect("read zero flag");
        assert_eq!(cleared.as_int().unwrap(), 0);
    }

    #[test]
    fn register_access_resolves_alias_redirects() {
        let (machine, mut state) = test_machine_state();
        let access = RegisterAccess::new(&machine);
        state
            .write_register("reg::GPR0", 0xFFFF)
            .expect("seed gpr0");
        let alias = RegisterRef {
            space: "reg".into(),
            name: "ALIAS".into(),
            subfield: None,
            index: None,
            span: None,
        };
        let resolved = access.resolve(&alias, None).expect("resolve alias");
        let value = resolved.read(&mut state).expect("read alias");
        assert_eq!(value.as_int().unwrap(), 0xFFFF);
        resolved.write(&mut state, 0xAA).expect("write alias");
        let raw = state.read_register("reg::GPR0").expect("read gpr0") as u32;
        assert_eq!(raw, 0xAA);
    }

    #[test]
    fn register_access_reads_alias_subfields() {
        let (machine, mut state) = test_machine_state();
        let access = RegisterAccess::new(&machine);
        state
            .write_register("reg::GPR0", 0x1234_5678)
            .expect("seed gpr0");
        let reference = RegisterRef {
            space: "reg".into(),
            name: "ALIAS".into(),
            subfield: Some("LOW".into()),
            index: None,
            span: None,
        };
        let resolved = access.resolve(&reference, None).expect("resolve alias");
        let value = resolved.read(&mut state).expect("read alias subfield");
        assert_eq!(value.as_int().unwrap(), 0x1234);
    }

    #[test]
    fn register_access_accepts_explicit_labels() {
        let (machine, mut state) = test_machine_state();
        let access = RegisterAccess::new(&machine);
        state
            .write_register("reg::GPR0", 0xFEED)
            .expect("seed gpr0");
        let reference = RegisterRef {
            space: "reg".into(),
            name: "GPR0".into(),
            subfield: None,
            index: None,
            span: None,
        };
        let resolved = access.resolve(&reference, None).expect("resolve label");
        let value = resolved.read(&mut state).expect("read gpr0");
        assert_eq!(value.as_int().unwrap(), 0xFEED);
    }

    #[test]
    fn register_access_handles_shared_bit_subfields() {
        let (machine, mut state) = test_machine_state();
        let access = RegisterAccess::new(&machine);

        let lt_ref = RegisterRef {
            space: "reg".into(),
            name: "CR0".into(),
            subfield: Some("LT".into()),
            index: None,
            span: None,
        };
        let neg_ref = RegisterRef {
            space: "reg".into(),
            name: "CR0".into(),
            subfield: Some("NEG".into()),
            index: None,
            span: None,
        };
        let so_ref = RegisterRef {
            space: "reg".into(),
            name: "CR0".into(),
            subfield: Some("SO".into()),
            index: None,
            span: None,
        };

        let lt = access.resolve(&lt_ref, None).expect("lt subfield");
        let neg = access.resolve(&neg_ref, None).expect("neg subfield");
        let so = access.resolve(&so_ref, None).expect("so subfield");

        lt.write(&mut state, 1).expect("set lt");
        assert_eq!(
            neg.read(&mut state).unwrap().as_int().unwrap(),
            1,
            "shared bit reflects across aliases",
        );

        lt.write(&mut state, 0).expect("clear lt");
        assert_eq!(
            neg.read(&mut state).unwrap().as_int().unwrap(),
            0,
            "clearing shared bit updates all names",
        );

        so.write(&mut state, 1).expect("set so");
        let raw = state.read_register("reg::CR0").expect("read cr0");
        assert_eq!(raw, 0b0001, "setting SO updates the correct bit of CR0");
    }

    fn build_machine() -> MachineDescription {
        let span = SourceSpan::point(PathBuf::from("test.isa"), SourcePosition::new(1, 1));
        let items = vec![
            IsaItem::Space(SpaceDecl {
                name: "reg".into(),
                kind: SpaceKind::Register,
                attributes: vec![
                    SpaceAttribute::WordSize(32),
                    SpaceAttribute::Endianness(Endianness::Little),
                ],
                span: span.clone(),
                enable: None,
            }),
            IsaItem::SpaceMember(SpaceMemberDecl {
                space: "reg".into(),
                member: SpaceMember::Field(FieldDecl {
                    space: "reg".into(),
                    name: "ACC".into(),
                    range: None,
                    offset: None,
                    size: Some(16),
                    reset: None,
                    description: None,
                    redirect: None,
                    subfields: Vec::new(),
                    span: span.clone(),
                    display: None,
                }),
            }),
            IsaItem::SpaceMember(SpaceMemberDecl {
                space: "reg".into(),
                member: SpaceMember::Field(FieldDecl {
                    space: "reg".into(),
                    name: "GPR".into(),
                    range: Some(FieldIndexRange { start: 0, end: 1 }),
                    offset: None,
                    size: Some(32),
                    reset: None,
                    description: None,
                    redirect: None,
                    subfields: Vec::new(),
                    span: span.clone(),
                    display: None,
                }),
            }),
            IsaItem::SpaceMember(SpaceMemberDecl {
                space: "reg".into(),
                member: SpaceMember::Field(FieldDecl {
                    space: "reg".into(),
                    name: "FLAGS".into(),
                    range: None,
                    offset: None,
                    size: Some(8),
                    reset: None,
                    description: None,
                    redirect: None,
                    subfields: vec![SubFieldDecl {
                        name: "ZERO".into(),
                        bit_spec: "@(0..1)".into(),
                        operations: Vec::new(),
                        description: None,
                        bit_spec_span: None,
                        bit_order: BitOrder::Msb0,
                    }],
                    span: span.clone(),
                    display: None,
                }),
            }),
            IsaItem::SpaceMember(SpaceMemberDecl {
                space: "reg".into(),
                member: SpaceMember::Field(FieldDecl {
                    space: "reg".into(),
                    name: "CR".into(),
                    range: Some(FieldIndexRange { start: 0, end: 1 }),
                    offset: None,
                    size: Some(4),
                    reset: None,
                    description: None,
                    redirect: None,
                    subfields: vec![
                        SubFieldDecl {
                            name: "LT".into(),
                            bit_spec: "@(0)".into(),
                            operations: Vec::new(),
                            description: Some("Less Than".into()),
                            bit_spec_span: None,
                            bit_order: BitOrder::Msb0,
                        },
                        SubFieldDecl {
                            name: "NEG".into(),
                            bit_spec: "@(0)".into(),
                            operations: Vec::new(),
                            description: Some("Negative".into()),
                            bit_spec_span: None,
                            bit_order: BitOrder::Msb0,
                        },
                        SubFieldDecl {
                            name: "GT".into(),
                            bit_spec: "@(1)".into(),
                            operations: Vec::new(),
                            description: Some("Greater Than".into()),
                            bit_spec_span: None,
                            bit_order: BitOrder::Msb0,
                        },
                        SubFieldDecl {
                            name: "POS".into(),
                            bit_spec: "@(1)".into(),
                            operations: Vec::new(),
                            description: Some("Positive".into()),
                            bit_spec_span: None,
                            bit_order: BitOrder::Msb0,
                        },
                        SubFieldDecl {
                            name: "EQ".into(),
                            bit_spec: "@(2)".into(),
                            operations: Vec::new(),
                            description: Some("Equal".into()),
                            bit_spec_span: None,
                            bit_order: BitOrder::Msb0,
                        },
                        SubFieldDecl {
                            name: "ZERO".into(),
                            bit_spec: "@(2)".into(),
                            operations: Vec::new(),
                            description: Some("Zero".into()),
                            bit_spec_span: None,
                            bit_order: BitOrder::Msb0,
                        },
                        SubFieldDecl {
                            name: "SO".into(),
                            bit_spec: "@(3)".into(),
                            operations: Vec::new(),
                            description: Some("Summary Overflow".into()),
                            bit_spec_span: None,
                            bit_order: BitOrder::Msb0,
                        },
                    ],
                    span: span.clone(),
                    display: None,
                }),
            }),
            IsaItem::SpaceMember(SpaceMemberDecl {
                space: "reg".into(),
                member: SpaceMember::Field(FieldDecl {
                    space: "reg".into(),
                    name: "ALIAS".into(),
                    range: None,
                    offset: None,
                    size: Some(32),
                    reset: None,
                    description: None,
                    redirect: Some(ContextReference {
                        segments: vec!["GPR0".into()],
                        segment_spans: vec![span.clone()],
                        span: span.clone(),
                    }),
                    subfields: vec![SubFieldDecl {
                        name: "LOW".into(),
                        bit_spec: "@(0..15)".into(),
                        operations: Vec::new(),
                        description: None,
                        bit_spec_span: None,
                        bit_order: BitOrder::Msb0,
                    }],
                    span: span.clone(),
                    display: None,
                }),
            }),
        ];
        let spec = IsaSpecification::new(PathBuf::from("test.isa"), items, Vec::new());
        MachineDescription::from_documents(vec![spec]).expect("machine description")
    }

    fn build_core_spec() -> Arc<CoreSpec> {
        Arc::new(
            CoreSpec::builder("demo", Endianness::Little)
                .register("reg::ACC", 16)
                .register("reg::GPR0", 32)
                .register("reg::GPR1", 32)
                .register("reg::FLAGS", 8)
                .register("reg::CR0", 4)
                .register("reg::CR1", 4)
                .build()
                .expect("core spec"),
        )
    }

    fn test_machine_state() -> (MachineDescription, CoreState) {
        let machine = build_machine();
        let core_spec = build_core_spec();
        let state = CoreState::new(core_spec).expect("core state");
        (machine, state)
    }
}
