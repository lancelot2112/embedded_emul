use std::cell::RefCell;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::rc::Rc;

use crate::common;
use nanemu::soc::core::ExecutionHarness;
use nanemu::soc::isa::machine::MachineDescription;
use nanemu::soc::isa::semantics::trace::{ExecutionTracer, HostOpKind, TraceEvent};

struct RecordingTracer {
    events: Rc<RefCell<Vec<TraceEvent>>>,
}

impl RecordingTracer {
    fn new(events: Rc<RefCell<Vec<TraceEvent>>>) -> Self {
        Self { events }
    }
}

impl ExecutionTracer for RecordingTracer {
    fn on_event(&mut self, event: TraceEvent) {
        self.events.borrow_mut().push(event);
    }
}

#[test]
fn tracer_captures_fetch_aliases_and_bit_widths() {
    let _lock = common::serial();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/tracer");
    let coredef = root.join("tracer.coredef");
    let mut harness =
        ExecutionHarness::from_coredef("ppc-trace", &coredef, None).expect("construct harness");

    {
        let state = harness.state_mut();
        state
            .write_register("reg::r3", 0x7FFF_FFFF)
            .expect("seed r3");
        state.write_register("reg::r4", 1).expect("seed r4");
    }

    let rom = build_add_rom(harness.machine());
    let events = Rc::new(RefCell::new(Vec::new()));
    harness.enable_tracer(Box::new(RecordingTracer::new(events.clone())));

    harness
        .execute_block(0x8000_1000, &rom)
        .expect("execute rom for trace");

    let events = events.borrow();
    let trace_dump = format_trace(&events);
    assert!(!events.is_empty(), "expected tracer events\n{trace_dump}");
    let fetch = events
        .iter()
        .find_map(|event| {
            if let TraceEvent::Fetch {
                mnemonic, detail, ..
            } = event
            {
                Some((mnemonic.clone(), detail.clone()))
            } else {
                None
            }
        })
        .unwrap_or_else(|| panic!("fetch event missing\n{trace_dump}"));
    assert_eq!(fetch.0, "add", "trace:\n{trace_dump}");
    assert!(
        fetch.1.contains("r5"),
        "fetch detail missing r5\n{trace_dump}"
    );

    assert_has_write(&events, "reg::CR0::NEG", 1, &trace_dump);
    assert_has_write(&events, "reg::XER::SO", 1, &trace_dump);
    assert_has_read(&events, "reg::XER::SO", 1, &trace_dump);

    assert!(
        events.iter().any(|event| matches!(
            event,
            TraceEvent::HostOp {
                op: HostOpKind::Add,
                ..
            }
        )),
        "missing host op\n{trace_dump}"
    );
}

fn assert_has_write(events: &[TraceEvent], name: &str, width: u32, trace: &str) {
    assert!(
        events.iter().any(|event| matches!(
            event,
            TraceEvent::RegisterWrite { name: n, width: w, .. } if n == name && *w == width
        )),
        "missing write for {name}\n{trace}"
    );
}

fn assert_has_read(events: &[TraceEvent], name: &str, width: u32, trace: &str) {
    assert!(
        events.iter().any(|event| matches!(
            event,
            TraceEvent::RegisterRead { name: n, width: w, .. } if n == name && *w == width
        )),
        "missing read for {name}\n{trace}"
    );
}

fn build_add_rom(machine: &MachineDescription) -> Vec<u8> {
    let mut rom = Vec::new();
    for line in [
        "add r5, r3, r4",
        "add. r6, r5, r4",
        "addo r7, r3, r3",
        "addo. r8, r7, r4",
    ] {
        let bytes = machine
            .assemble(line)
            .unwrap_or_else(|err| panic!("failed to assemble '{line}': {err}"));
        rom.extend(bytes);
    }
    rom
}

fn format_trace(events: &[TraceEvent]) -> String {
    let mut out = String::new();
    for event in events {
        let line = match event {
            TraceEvent::Fetch {
                address,
                opcode,
                mnemonic,
                detail,
            } => format!(
                "[Fetch] 0x{address:08X} 0x{opcode:08X} {mnemonic} {detail}",
                detail = detail.trim()
            ),
            TraceEvent::RegisterRead { name, value, width } => {
                format!("[ Read]   {name} -> {}", format_trace_value(*value, *width))
            }
            TraceEvent::RegisterWrite { name, value, width } => {
                format!("[Write]   {name} <- {}", format_trace_value(*value, *width))
            }
            TraceEvent::HostOp {
                op,
                args,
                result,
                carry,
            } => {
                if args.len() == 2 {
                    format!(
                        "[IntOp]   0x{lhs:016X} {op} 0x{rhs:016X} = 0x{result:016X} (carry={carry})",
                        lhs = args[0],
                        rhs = args[1]
                    )
                } else {
                    format!(
                        "[IntOp]   {op:?} {:?} -> 0x{result:016X} (carry={carry})",
                        args
                    )
                }
            }
        };
        let _ = writeln!(out, "{line}");
    }
    out
}

fn format_trace_value(value: i64, bits: u32) -> String {
    let width = std::cmp::max(1, (bits as usize).div_ceil(4));
    let masked = if bits == 0 {
        0
    } else if bits >= 64 {
        value as u64
    } else {
        let mask = (1u64 << bits) - 1;
        (value as u64) & mask
    };
    format!("0x{masked:0width$X}")
}
