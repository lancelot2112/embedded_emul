use std::path::PathBuf;

use crate::common;
use nanemu::loader::isa::IsaLoader;
use nanemu::soc::core::ExecutionHarness;
use nanemu::soc::isa::machine::{MachineDescription, SoftwareHost};
use nanemu::soc::isa::semantics::trace::PipelinePrinter;

#[test]
fn disassembles_powerpc_vle_stream() {
    let _lock = common::serial();
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
fn executes_powerpc_add() {
    let _lock = common::serial();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("defs/powerpc");
    let coredef = root.join("e200.coredef");
    let mut harness = build_powerpc_harness(&coredef);
    harness
        .write("reg::r3", 0x7FFF_FFFF as u128)
        .expect("seed r3");
    harness.write("reg::r4", 1 as u128).expect("seed r4");

    let rom = assemble_block(harness.machine(), &["add r5, r3, r4"]);
    let executions = harness
        .execute_block(0x8000_1000, &rom)
        .expect("execute add");
    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].mnemonic, "add");

    let r5 = harness
        .state_mut()
        .read_register("reg::r5")
        .expect("read r5");
    assert_eq!(r5 as u32, 0x8000_0000);
}

#[test]
fn executes_powerpc_add_record_sets_cr0() {
    let _lock = common::serial();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("defs/powerpc");
    let coredef = root.join("e200.coredef");
    let mut harness = build_powerpc_harness(&coredef);
    harness
        .write("reg::r3", 0x7FFF_FFFF as u128)
        .expect("seed r3");
    harness.write("reg::r4", 1 as u128).expect("seed r4");

    let rom = assemble_block(harness.machine(), &["add r5, r3, r4", "add. r6, r5, r4"]);
    let executions = harness
        .execute_block(0x8000_1000, &rom)
        .expect("execute add.");
    assert_eq!(executions.len(), 2);
    assert_eq!(executions[1].mnemonic, "add.");

    let r6 = harness
        .state_mut()
        .read_register("reg::r6")
        .expect("read r6");
    assert_eq!(r6 as u32, 0x8000_0001);

    check_cr0(&mut harness, 0, 0, 0);
}

fn check_cr0(
    harness: &mut ExecutionHarness<SoftwareHost>,
    exp_neg: i64,
    exp_pos: i64,
    exp_zero: i64,
) {
    let raw = harness
        .state_mut()
        .read_register("reg::CR0")
        .expect("CR0 raw");
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
    assert_eq!(
        neg, exp_neg,
        "CR0::NEG should be {exp_neg}, got {neg} (raw=0x{raw:X})"
    );
    assert_eq!(
        pos, exp_pos,
        "CR0::POS should be {exp_pos}, got {pos} (raw=0x{raw:X})"
    );
    assert_eq!(
        zero, exp_zero,
        "CR0::ZERO should be {exp_zero}, got {zero} (raw=0x{raw:X})"
    );
}

#[test]
fn executes_powerpc_add_with_overflow() {
    let _lock = common::serial();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("defs/powerpc");
    let coredef = root.join("e200.coredef");
    let mut harness = build_powerpc_harness(&coredef);
    harness
        .write("reg::r3", 0xFFFF_FFFF as u128)
        .expect("seed r3");
    harness.write("reg::r4", 1 as u128).expect("seed r4");
    harness.write("reg::XER", 0).expect("clear XER");

    let rom = assemble_block(harness.machine(), &["addo r7, r3, r4"]);
    let executions = harness
        .execute_block(0x8000_1000, &rom)
        .expect("execute addo");
    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].mnemonic, "addo");

    let r7 = harness.read("reg::r7").expect("read r7");
    assert_eq!(r7 as u32, 0);

    check_summary_overflow(&mut harness, true, true, false);
}

#[test]
fn executes_powerpc_add_with_overflow_and_record() {
    let _lock = common::serial();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("defs/powerpc");
    let coredef = root.join("e200.coredef");
    let mut harness = build_powerpc_harness(&coredef);
    harness
        .write("reg::r3", 0xFFFF_FFFF as u128)
        .expect("seed r3");
    harness.write("reg::r4", 1 as u128).expect("seed r4");
    harness.write("reg::XER", 0).expect("clear XER");

    let rom = assemble_block(harness.machine(), &["addo. r8, r3, r4"]);
    let executions = harness
        .execute_block(0x8000_1000, &rom)
        .expect("execute addo.");
    assert_eq!(executions.len(), 1);
    assert_eq!(executions[0].mnemonic, "addo.");

    let r8 = harness.read("reg::r8").expect("read r8");
    assert_eq!(r8 as u32, 0);

    check_summary_overflow(&mut harness, true, true, true);
    check_cr0(&mut harness, 0, 0, 0);
}

fn check_summary_overflow(
    harness: &mut ExecutionHarness<SoftwareHost>,
    exp_xer_ov: bool,
    exp_xer_so: bool,
    exp_cr_so: bool,
) {
    let xer_ov = harness.read("reg::XER::OV").expect("read XER::OV") == 1;
    let cr_so = harness.read("reg::CR0::SO").expect("read CR0::SO") == 1;
    let xer_so = harness.read("reg::XER::SO").expect("read XER::SO") == 1;
    assert_eq!(
        xer_ov, exp_xer_ov,
        "overflow should be {exp_xer_ov}, (ov={xer_ov}, cr_so={cr_so},  xer_so={xer_so})"
    );
    assert_eq!(
        xer_so, exp_xer_so,
        "summary overflow should be {exp_xer_so}, (ov={xer_ov}, cr_so={cr_so}, xer_so={xer_so})"
    );
    assert_eq!(
        cr_so, exp_cr_so,
        "CR summary overflow should be {exp_cr_so}, (ov={xer_ov}, cr_so={cr_so}, xer_so={xer_so})"
    );
}

fn enable_trace_if_requested(harness: &mut ExecutionHarness<SoftwareHost>) {
    if std::env::var_os("TRACE_PIPELINE").is_some() {
        harness.enable_tracer(Box::new(PipelinePrinter::stdout()));
    }
}

fn build_powerpc_harness(coredef: &PathBuf) -> ExecutionHarness<SoftwareHost> {
    let mut harness =
        ExecutionHarness::from_coredef("ppc-e200", coredef, None).expect("construct harness");
    enable_trace_if_requested(&mut harness);
    harness
}

fn assemble_block(machine: &MachineDescription, lines: &[&str]) -> Vec<u8> {
    let mut rom = Vec::new();
    for line in lines {
        let bytes = machine.assemble(line).unwrap_or_else(|err| {
            panic!("failed to assemble '{line}': {err}");
        });
        rom.extend(bytes);
    }
    rom
}
