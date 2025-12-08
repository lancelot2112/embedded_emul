//! Operand and parameter binding helpers used by the semantics runtime.
//!
//! These utilities decode instruction operands using `BitFieldSpec` metadata and
//! gather the resulting values (plus global parameters) into a convenient map
//! that seeds the `ExecutionContext` when evaluating semantic programs.

use std::collections::HashMap;
use std::convert::TryFrom;

use crate::soc::isa::ast::ParameterValue;
use crate::soc::isa::error::IsaError;
use crate::soc::isa::machine::FormInfo;
use crate::soc::isa::semantics::value::SemanticValue;
use crate::soc::prog::types::BitFieldSpec;

#[derive(Debug, Clone)]
pub struct OperandBinder {
    bindings: Vec<FieldBinding>,
}

#[derive(Debug, Clone)]
struct FieldBinding {
    name: String,
    spec: BitFieldSpec,
}

#[derive(Debug, Default, Clone)]
pub struct ParameterBindings {
    values: HashMap<String, SemanticValue>,
}

impl OperandBinder {
    /// Builds a binder from the provided form metadata plus the instruction's operand list.
    ///
    /// If an instruction does not declare operands explicitly, the form's operand order
    /// drives the binding list. An error is returned when an operand references an
    /// undefined subfield.
    pub fn from_form(form: &FormInfo, instruction_operands: &[String]) -> Result<Self, IsaError> {
        let operand_names: Vec<&str> = if instruction_operands.is_empty() {
            form.operand_order
                .iter()
                .map(|name| name.as_str())
                .collect()
        } else {
            instruction_operands
                .iter()
                .map(|name| name.as_str())
                .collect()
        };

        let mut bindings = Vec::with_capacity(operand_names.len());
        for name in operand_names {
            let field = form.subfield(name).ok_or_else(|| {
                IsaError::Machine(format!(
                    "instruction references operand '{name}' missing from form"
                ))
            })?;
            bindings.push(FieldBinding {
                name: name.to_string(),
                spec: field.spec.clone(),
            });
        }
        Ok(Self { bindings })
    }

    /// Decodes operands from the provided instruction bits using the stored bindings.
    pub fn decode(&self, bits: u64) -> ParameterBindings {
        let mut bindings = ParameterBindings::new();
        self.decode_into(bits, &mut bindings);
        bindings
    }

    /// Decodes operands and writes them into an existing bindings map.
    pub fn decode_into(&self, bits: u64, bindings: &mut ParameterBindings) {
        for field in &self.bindings {
            let value = field.spec.read_from(bits) as i64;
            bindings.insert_int(field.name.clone(), value);
        }
    }

    /// Reports how many operands will be produced.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl ParameterBindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_value(
        &mut self,
        name: impl Into<String>,
        value: SemanticValue,
    ) -> Option<SemanticValue> {
        self.values.insert(name.into(), value)
    }

    pub fn insert_int(&mut self, name: impl Into<String>, value: i64) -> Option<SemanticValue> {
        self.insert_value(name, SemanticValue::int(value))
    }

    pub fn insert_bool(&mut self, name: impl Into<String>, value: bool) -> Option<SemanticValue> {
        self.insert_value(name, SemanticValue::bool(value))
    }

    pub fn insert_word(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Option<SemanticValue> {
        self.insert_value(name, SemanticValue::word(value))
    }

    pub fn insert_parameter(
        &mut self,
        name: impl Into<String>,
        value: &ParameterValue,
    ) -> Result<(), IsaError> {
        let name = name.into();
        match value {
            ParameterValue::Number(raw) => {
                let signed = i64::try_from(*raw).map_err(|_| {
                    IsaError::Machine(format!("parameter '{name}' exceeds 64-bit signed range"))
                })?;
                self.insert_int(name, signed);
            }
            ParameterValue::Word(word) => {
                self.insert_word(name, word.clone());
            }
        }
        Ok(())
    }

    pub fn extend_from_parameters<'a, I>(&mut self, params: I) -> Result<(), IsaError>
    where
        I: IntoIterator<Item = (&'a str, &'a ParameterValue)>,
    {
        for (name, value) in params {
            self.insert_parameter(name, value)?;
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&SemanticValue> {
        self.values.get(name)
    }

    pub fn as_map(&self) -> &HashMap<String, SemanticValue> {
        &self.values
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn into_inner(self) -> HashMap<String, SemanticValue> {
        self.values
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::isa::ast::{ParameterValue, SubFieldOp};
    use crate::soc::isa::machine::{FieldEncoding, FormInfo, OperandKind};
    use crate::soc::prog::types::BitFieldSpec;

    fn subfield(
        name: &str,
        store_bitw: u16,
        offset: u16,
        width: u16,
        signed: bool,
        kind: OperandKind,
    ) -> FieldEncoding {
        let spec = BitFieldSpec::builder(store_bitw)
            .range(offset, width)
            .signed(signed)
            .finish();
        FieldEncoding {
            name: name.to_string(),
            spec,
            operations: vec![SubFieldOp {
                kind: match kind {
                    OperandKind::Register => "target".into(),
                    OperandKind::Immediate => "immediate".into(),
                    OperandKind::Other => "operand".into(),
                },
                subtype: None,
            }],
            register: None,
            kind,
        }
    }

    #[test]
    fn binder_decodes_operands_using_form_defaults() {
        let mut form = FormInfo::new("X_FORM".into());
        form.push_field(subfield("RT", 32, 0, 5, false, OperandKind::Register));
        form.push_field(subfield("RA", 32, 5, 5, false, OperandKind::Register));
        form.push_field(subfield("IMM", 32, 10, 16, true, OperandKind::Immediate));

        let binder = OperandBinder::from_form(&form, &[]).expect("binder");
        assert_eq!(binder.len(), 3);

        let rt = 3u64;
        let ra = 17u64;
        let imm = -4i16;
        let imm_bits = (imm as i32 as u32 & 0xFFFF) as u64;
        let bits = (rt & 0x1F) | ((ra & 0x1F) << 5) | (imm_bits << 10);

        let bindings = binder.decode(bits);
        assert_eq!(bindings.len(), 3);
        assert_eq!(bindings.get("RT").unwrap().as_int().unwrap(), 3);
        assert_eq!(bindings.get("RA").unwrap().as_int().unwrap(), 17);
        assert_eq!(bindings.get("IMM").unwrap().as_int().unwrap(), -4);
    }

    #[test]
    fn binder_respects_instruction_operand_override() {
        let mut form = FormInfo::new("X_FORM".into());
        form.push_field(subfield("RT", 32, 0, 5, false, OperandKind::Register));
        form.push_field(subfield("RB", 32, 5, 5, false, OperandKind::Register));

        let binder = OperandBinder::from_form(&form, &["RB".into()]).expect("binder");
        assert_eq!(binder.len(), 1);

        let bits = (12u64 << 5) | 1;
        let bindings = binder.decode(bits);
        assert!(bindings.get("RT").is_none());
        assert_eq!(bindings.get("RB").unwrap().as_int().unwrap(), 12);
    }

    #[test]
    fn parameter_bindings_store_scalars() {
        let mut params = ParameterBindings::new();
        assert!(params.is_empty());
        params.insert_int("SIZE_MODE", 32);
        params.insert_bool("flag", true);
        assert_eq!(params.len(), 2);
        assert_eq!(params.get("SIZE_MODE").unwrap().as_int().unwrap(), 32);
        assert!(params.get("flag").unwrap().as_bool().unwrap());
    }

    #[test]
    fn parameter_bindings_load_parameter_values() {
        let mut params = ParameterBindings::new();
        let entries = [
            ("SIZE_MODE", ParameterValue::Number(64)),
            ("ENDIAN", ParameterValue::Word("big".into())),
        ];
        params
            .extend_from_parameters(entries.iter().map(|(k, v)| (*k, v)))
            .expect("extend parameters");
        assert_eq!(params.get("SIZE_MODE").unwrap().as_int().unwrap(), 64);
        assert_eq!(params.get("ENDIAN").unwrap().as_word(), Some("big"));
    }

    #[test]
    fn parameter_binding_rejects_out_of_range_numbers() {
        let mut params = ParameterBindings::new();
        let result = params.insert_parameter("HUGE", &ParameterValue::Number(u64::MAX));
        assert!(matches!(result, Err(IsaError::Machine(msg)) if msg.contains("HUGE")));
        assert!(params.is_empty());
    }
}
