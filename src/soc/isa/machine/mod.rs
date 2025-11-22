//! Root coordination layer for the ISA machine runtime. This module owns the
//! [`MachineDescription`] structure and wires together the specialized
//! submodules that handle spaces, instructions, disassembly, register metadata,
//! and display formatting.

mod disassembly;
mod format;
mod host;

mod instruction;
mod macros;
mod register;
mod space;

pub use disassembly::Disassembly;
pub use host::{HostArithResult, HostMulResult, HostServices, SoftwareHost};
pub use instruction::{Instruction, InstructionMask};
pub use macros::MacroInfo;
pub use register::{RegisterBinding, RegisterInfo};
pub use space::{FieldEncoding, FormInfo, OperandKind, SpaceInfo, encode_constant, parse_bit_spec};

use std::collections::BTreeMap;

use crate::soc::isa::ast::{
    FieldDecl, FormDecl, IsaItem, IsaSpecification, MacroDecl, ParameterDecl, ParameterValue,
    SpaceDecl, SpaceKind, SpaceMember,
};
use crate::soc::isa::error::IsaError;

use disassembly::LogicDecodeSpace;
use instruction::InstructionPattern;

#[derive(Debug, Clone)]
pub struct MachineDescription {
    pub instructions: Vec<Instruction>,
    pub spaces: BTreeMap<String, SpaceInfo>,
    pub macros: Vec<MacroInfo>,
    pub parameters: BTreeMap<String, ParameterValue>,
    patterns: Vec<InstructionPattern>,
    decode_spaces: Vec<LogicDecodeSpace>,
}

impl Default for MachineDescription {
    fn default() -> Self {
        Self {
            instructions: Vec::new(),
            spaces: BTreeMap::new(),
            macros: Vec::new(),
            parameters: BTreeMap::new(),
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
        let mut macros = Vec::new();
        let mut parameters: BTreeMap<String, ParameterValue> = BTreeMap::new();

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
                    IsaItem::Macro(mac) => macros.push(mac),
                    IsaItem::Parameter(ParameterDecl { name, value }) => {
                        parameters.insert(name, value);
                    }
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
        for mac in macros {
            machine.register_macro(mac);
        }
        machine.parameters = parameters;
        machine.build_patterns()?;
        machine.build_decode_spaces()?;

        Ok(machine)
    }

    pub fn disassemble(&self, bytes: &[u8]) -> Vec<Disassembly> {
        self.disassemble_from(bytes, 0)
    }

    pub fn finalize_machine(
        &self,
        docs: Vec<IsaSpecification>,
    ) -> Result<MachineDescription, IsaError> {
        MachineDescription::from_documents(docs)
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

    fn register_macro(&mut self, mac: MacroDecl) {
        self.macros.push(MacroInfo::from_decl(mac));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::endianness::Endianness;
    use crate::soc::isa::ast::{
        IsaItem, IsaSpecification, MacroDecl, ParameterValue, SpaceAttribute, SpaceKind,
        SubFieldDecl,
    };
    use crate::soc::isa::builder::{IsaBuilder, mask_field_selector, subfield_op};
    use crate::soc::isa::diagnostic::{SourcePosition, SourceSpan};
    use crate::soc::isa::machine::{encode_constant, parse_bit_spec};
    use crate::soc::isa::semantics::SemanticBlock;
    use std::path::PathBuf;

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
        assert_eq!(listing[0].display.as_deref(), Some("GPR1 <- GPR2 + GPR3"));

        assert_eq!(listing[1].mnemonic, "swap");
        assert_eq!(
            listing[1].operands,
            vec!["GPR0".to_string(), "GPR3".to_string(), "GPR2".to_string()]
        );
        assert_eq!(listing[1].display.as_deref(), Some("GPR0 <-> GPR3"));
    }

    #[test]
    fn machine_description_preserves_parameters() {
        let mut builder = IsaBuilder::new("params.isa");
        builder.add_parameter("SIZE_MODE", ParameterValue::Number(32));
        builder.add_parameter("ENDIAN", ParameterValue::Word("big".into()));
        let doc = builder.build();
        let machine = MachineDescription::from_documents(vec![doc]).expect("machine");
        assert_eq!(machine.parameters.len(), 2);
        assert!(matches!(
            machine.parameters.get("SIZE_MODE"),
            Some(ParameterValue::Number(32))
        ));
        assert!(matches!(
            machine.parameters.get("ENDIAN"),
            Some(ParameterValue::Word(value)) if value == "big"
        ));
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
    fn collects_macros_from_documents() {
        let macro_decl = MacroDecl {
            name: "upd".into(),
            parameters: vec!["res".into()],
            semantics: SemanticBlock::from_source("$reg::CR0 = #res".into()),
            span: SourceSpan::point(PathBuf::from("test.isa"), SourcePosition::new(1, 1)),
        };
        let doc =
            IsaSpecification::new(PathBuf::from("test.isa"), vec![IsaItem::Macro(macro_decl)]);
        let machine = MachineDescription::from_documents(vec![doc]).expect("machine");
        assert_eq!(machine.macros.len(), 1);
        let mac = &machine.macros[0];
        assert_eq!(mac.name, "upd");
        assert_eq!(mac.parameters, vec!["res".to_string()]);
        assert!(mac.semantics.source.contains("#res"));
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
