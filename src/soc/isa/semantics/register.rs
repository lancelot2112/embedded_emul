use std::borrow::Cow;

use crate::soc::core::state::{CoreState, StateError};
use crate::soc::isa::ast::ContextReference;
use crate::soc::isa::error::IsaError;
use crate::soc::isa::machine::{
    MachineDescription, RegisterElement, RegisterFieldMetadata, RegisterMetadata, RegisterSchema,
};
use crate::soc::isa::semantics::program::RegisterRef;
use crate::soc::prog::types::arena::TypeArena;
use crate::soc::prog::types::bitfield::BitFieldSpec;
use crate::soc::prog::types::record::TypeRecord;

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
        if let Some(resolved) = self.try_resolve_direct(
            &reference.space,
            &reference.name,
            reference.subfield.as_deref(),
            evaluated_index,
        )? {
            return Ok(resolved);
        }

        if let Some((target_space, target_name)) =
            self.follow_redirect(&reference.space, &reference.name)?
        {
            if let Some(resolved) = self.try_resolve_direct(
                &target_space,
                &target_name,
                reference.subfield.as_deref(),
                evaluated_index,
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
    ) -> Result<Option<ResolvedRegister<'machine>>, IsaError> {
        if let Some(metadata) = self.schema.lookup(space, name) {
            let element = self.select_element(metadata, name, evaluated_index)?;
            let field = self.select_field(metadata, subfield, name)?;
            return Ok(Some(ResolvedRegister::new(
                metadata, element, field, self.arena,
            )));
        }

        if let Some((metadata, element)) = self.schema.find_by_label(space, name) {
            if let Some(index) = evaluated_index {
                if index != element.index as i64 {
                    return Err(IsaError::Machine(format!(
                        "register '{}' already selects element '{}' and cannot mix with index {}",
                        name, element.label, index
                    )));
                }
            }
            let field = self.select_field(metadata, subfield, name)?;
            return Ok(Some(ResolvedRegister::new(
                metadata, element, field, self.arena,
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
            if let Some(index) = evaluated_index {
                if index != 0 {
                    return Err(IsaError::Machine(format!(
                        "register '{}' has a single element and cannot use index {index}",
                        register_name
                    )));
                }
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
    ) -> Result<Option<&'schema RegisterFieldMetadata>, IsaError> {
        if let Some(name) = subfield {
            let field = metadata
                .fields
                .iter()
                .find(|field| field.name.eq_ignore_ascii_case(name))
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
    name: String,
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
    ) -> Self {
        let name = format!("{}::{}", metadata.space, element.label);
        Self {
            name,
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
            let value = spec.read_signed(raw);
            Ok(SemanticValue::int(value))
        } else {
            Ok(SemanticValue::int(raw as i64))
        }
    }

    pub fn write(&self, state: &mut CoreState, value: i64) -> Result<(), IsaError> {
        if let Some(field) = self.field {
            let spec = self.field_spec(field)?;
            let mut container = self.read_raw(state)?;
            let updated = spec.write_bits(container, value as u64).map_err(|err| {
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
                .write_register(&self.name, masked as u128)
                .map_err(core_state_error)
        }
    }

    fn read_raw(&self, state: &mut CoreState) -> Result<u64, IsaError> {
        let value = state.read_register(&self.name).map_err(core_state_error)?;
        if self.metadata.bit_width > 64 {
            return Err(IsaError::Machine(format!(
                "register '{}::{}' exceeds 64-bit access width",
                self.metadata.space, self.element.label
            )));
        }
        Ok(value as u64)
    }

    fn write_raw(&self, state: &mut CoreState, value: u64) -> Result<(), IsaError> {
        state
            .write_register(&self.name, value as u128)
            .map_err(core_state_error)
    }

    fn field_spec(&self, field: &RegisterFieldMetadata) -> Result<&BitFieldSpec, IsaError> {
        match self.arena.get(field.ty) {
            TypeRecord::BitField(spec) => Ok(spec),
            _ => Err(IsaError::Machine(format!(
                "subfield '{}' lacks bitfield metadata",
                field.name
            ))),
        }
    }
}

fn resolve_reference_path(
    current_space: &str,
    reference: &ContextReference,
) -> (String, Vec<String>) {
    if let Some(first) = reference.segments.first() {
        if first.starts_with('$') {
            let space = first.trim_start_matches('$').to_string();
            let rest = reference.segments[1..].to_vec();
            return (space, rest);
        }
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
        };
        let resolved = access.resolve(&alias, None).expect("resolve alias");
        let value = resolved.read(&mut state).expect("read alias");
        assert_eq!(value.as_int().unwrap(), 0xFFFF);
        resolved.write(&mut state, 0xAA).expect("write alias");
        let raw = state.read_register("reg::GPR0").expect("read gpr0") as u32;
        assert_eq!(raw, 0xAA);
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
        };
        let resolved = access.resolve(&reference, None).expect("resolve label");
        let value = resolved.read(&mut state).expect("read gpr0");
        assert_eq!(value.as_int().unwrap(), 0xFEED);
    }

    fn build_machine() -> MachineDescription {
        let span = SourceSpan::point(PathBuf::from("test.isa"), SourcePosition::new(1, 1));
        let mut items = Vec::new();
        items.push(IsaItem::Space(SpaceDecl {
            name: "reg".into(),
            kind: SpaceKind::Register,
            attributes: vec![
                SpaceAttribute::WordSize(32),
                SpaceAttribute::Endianness(Endianness::Little),
            ],
            span: span.clone(),
            enable: None,
        }));

        items.push(IsaItem::SpaceMember(SpaceMemberDecl {
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
        }));

        items.push(IsaItem::SpaceMember(SpaceMemberDecl {
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
        }));

        items.push(IsaItem::SpaceMember(SpaceMemberDecl {
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
                }],
                span: span.clone(),
                display: None,
            }),
        }));

        items.push(IsaItem::SpaceMember(SpaceMemberDecl {
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
                }),
                subfields: Vec::new(),
                span: span.clone(),
                display: None,
            }),
        }));

        let spec = IsaSpecification::new(PathBuf::from("test.isa"), items);
        MachineDescription::from_documents(vec![spec]).expect("machine description")
    }

    fn build_core_spec() -> Arc<CoreSpec> {
        Arc::new(
            CoreSpec::builder("demo", Endianness::Little)
                .register("reg::ACC", 16)
                .register("reg::GPR0", 32)
                .register("reg::GPR1", 32)
                .register("reg::FLAGS", 8)
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
