use std::path::PathBuf;

use nanemu::loader::isa::IsaLoader;
use nanemu::soc::core::ExecutionHarness;
use nanemu::soc::device::Endianness;
use nanemu::soc::isa::ast::MaskSelector;
use nanemu::soc::isa::machine::MachineDescription;

#[test]
fn disassembles_powerpc_vle_stream() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("defs/powerpc");
    let coredef = root.join("e200.coredef");
    let mut loader = IsaLoader::new();
    let machine = loader
        .load_machine(coredef)
        .expect("load powerpc + vle includes");

    let addi = 0x3800_0000u32.to_be_bytes();
    let se_b = 0xE800u16.to_be_bytes();

    let mut stream = Vec::new();
    stream.extend_from_slice(&addi);
    stream.extend_from_slice(&se_b);

    let listing = machine.disassemble_from(&stream, 0x1000);
    assert_eq!(listing.len(), 2, "expected 32-bit + 16-bit instructions");

    if std::env::var_os("SHOW_DISASM").is_some() {
        eprintln!("PowerPC VLE listing:");
        for entry in &listing {
            if let Some(display) = &entry.display {
                eprintln!(
                    "  0x{addr:08X}: {mnemonic:<6} {display}",
                    addr = entry.address,
                    mnemonic = entry.mnemonic,
                    display = display,
                );
            } else {
                eprintln!(
                    "  0x{addr:08X}: {mnemonic:<6} {operands:?}",
                    addr = entry.address,
                    mnemonic = entry.mnemonic,
                    operands = entry.operands,
                );
            }
        }
    }

    assert_eq!(listing[0].mnemonic, "addi");
    assert_eq!(listing[0].address, 0x1000);
    assert_eq!(
        listing[0].operands,
        vec!["r0", "r0", "0x0000"],
        "disp formatting should rename registers"
    );
    assert_eq!(listing[0].display.as_deref(), Some("r0, r0, 0x0000"));

    assert_eq!(listing[1].mnemonic, "se_b");
    assert_eq!(listing[1].address, 0x1004);
    assert_eq!(listing[1].display.as_deref(), Some("0x000"));
}

#[test]
fn executes_powerpc_add_family() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("defs/powerpc");
    let coredef = root.join("e200.coredef");
    let mut harness =
        ExecutionHarness::from_coredef("ppc-e200", &coredef, None).expect("construct harness");

    {
        let state = harness.state_mut();
        state
            .write_register("reg::r3", 0x7FFF_FFFF)
            .expect("seed r3");
        state.write_register("reg::r4", 1).expect("seed r4");
    }

    let mut rom = Vec::new();
    rom.extend(encode_instruction(
        harness.machine(),
        "add",
        &[("RT", 5), ("RA", 3), ("RB", 4)],
    ));
    rom.extend(encode_instruction(
        harness.machine(),
        "add.",
        &[("RT", 6), ("RA", 5), ("RB", 4)],
    ));
    rom.extend(encode_instruction(
        harness.machine(),
        "addo",
        &[("RT", 7), ("RA", 3), ("RB", 3)],
    ));
    rom.extend(encode_instruction(
        harness.machine(),
        "addo.",
        &[("RT", 8), ("RA", 7), ("RB", 4)],
    ));

    let executions = harness
        .execute_block(0x8000_1000, &rom)
        .expect("execute rom");
    assert_eq!(executions.len(), 4);
    let mnemonics: Vec<_> = executions
        .iter()
        .map(|exec| exec.mnemonic.as_str())
        .collect();
    assert_eq!(mnemonics, vec!["add", "add.", "addo", "addo."]);

    let r5 = harness
        .state_mut()
        .read_register("reg::r5")
        .expect("read r5");
    assert_eq!(r5 as u32, 0x8000_0000);

    let r6 = harness
        .state_mut()
        .read_register("reg::r6")
        .expect("read r6");
    assert_eq!(r6 as u32, 0x8000_0001);

    let r7 = harness
        .state_mut()
        .read_register("reg::r7")
        .expect("read r7");
    assert_eq!(r7 as u32, 0xFFFF_FFFE);

    let r8 = harness
        .state_mut()
        .read_register("reg::r8")
        .expect("read r8");
    assert_eq!(r8 as u32, 0xFFFF_FFFF);

    let neg = harness
        .read_register_value("reg", "CR0", Some("NEG"), None)
        .expect("CR0::NEG")
        .as_int()
        .expect("neg int");
    let pos = harness
        .read_register_value("reg", "CR0", Some("POS"), None)
        .expect("CR0::POS")
        .as_int()
        .expect("pos int");
    let zero = harness
        .read_register_value("reg", "CR0", Some("ZERO"), None)
        .expect("CR0::ZERO")
        .as_int()
        .expect("zero int");
    let cr_so = harness
        .read_register_value("reg", "CR0", Some("SO"), None)
        .expect("CR0::SO")
        .as_int()
        .expect("so int");
    assert_eq!(neg, 1, "add. with negative result should set NEG");
    assert_eq!(pos, 0);
    assert_eq!(zero, 0);
    assert_eq!(cr_so, 1, "addo. should mirror XER::SO into CR0");

    let xer_ov = harness
        .read_register_value("reg", "XER", Some("OV"), None)
        .expect("XER::OV")
        .as_int()
        .expect("ov int");
    let xer_so = harness
        .read_register_value("reg", "XER", Some("SO"), None)
        .expect("XER::SO")
        .as_int()
        .expect("so int");
    assert_eq!(xer_ov, 1, "addo should raise overflow flag");
    assert_eq!(xer_so, 1, "addo should latch summary overflow");
}

fn encode_instruction(
    machine: &MachineDescription,
    mnemonic: &str,
    operands: &[(&str, i64)],
) -> Vec<u8> {
    let instr = machine
        .instructions
        .iter()
        .find(|candidate| candidate.name == mnemonic)
        .unwrap_or_else(|| panic!("unknown instruction '{mnemonic}'"));
    let space = machine
        .spaces
        .get(&instr.space)
        .unwrap_or_else(|| panic!("instruction space '{}' missing", instr.space));
    let word_bits = space.word_bits().expect("logic space word size");
    assert_eq!(word_bits % 8, 0, "expected byte-aligned instruction");
    let word_bytes = (word_bits / 8) as usize;
    let mut bits = 0u64;

    if let Some(mask) = &instr.mask {
        for field in &mask.fields {
            let spec = match &field.selector {
                MaskSelector::Field(name) => {
                    let form_name = instr
                        .form
                        .as_ref()
                        .unwrap_or_else(|| panic!("instruction '{mnemonic}' missing form"));
                    let form = space
                        .forms
                        .get(form_name)
                        .unwrap_or_else(|| panic!("form '{form_name}' missing"));
                    form.subfield(name)
                        .unwrap_or_else(|| panic!("unknown field '{name}'"))
                        .spec
                        .clone()
                }
                MaskSelector::BitExpr(expr) => {
                    panic!("bit expression selector '{expr}' unsupported in test encoder")
                }
            };
            bits = spec
                .write_bits(bits, field.value)
                .expect("apply mask constant");
        }
    }

    if let Some(form_name) = &instr.form {
        let form = space
            .forms
            .get(form_name)
            .unwrap_or_else(|| panic!("form '{form_name}' missing"));
        for (name, value) in operands {
            let field = form
                .subfield(name)
                .unwrap_or_else(|| panic!("unknown operand '{name}'"));
            bits = field
                .spec
                .write_bits(bits, (*value as i64) as u64)
                .expect("encode operand");
        }
    }

    let mut buffer = vec![0u8; word_bytes];
    match space.endianness {
        Endianness::Little => {
            for (idx, byte) in buffer.iter_mut().enumerate() {
                *byte = ((bits >> (8 * idx)) & 0xFF) as u8;
            }
        }
        Endianness::Big => {
            for (idx, byte) in buffer.iter_mut().enumerate() {
                let shift = 8 * (word_bytes - 1 - idx);
                *byte = ((bits >> shift) & 0xFF) as u8;
            }
        }
    }
    buffer
}
