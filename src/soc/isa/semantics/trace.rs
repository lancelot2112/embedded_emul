use std::fmt;
use std::io::Write;

/// High-level events emitted while executing semantics so tooling can build
/// pipeline-style traces.
#[derive(Debug, Clone)]
pub enum TraceEvent {
    Fetch {
        address: u64,
        opcode: u64,
        mnemonic: String,
        detail: String,
    },
    RegisterRead {
        name: String,
        value: i64,
        width: u32,
    },
    RegisterWrite {
        name: String,
        value: i64,
        width: u32,
    },
    HostOp {
        op: HostOpKind,
        args: Vec<i64>,
        result: i64,
        carry: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum HostOpKind {
    Add,
    Sub,
    Mul,
}

impl fmt::Display for HostOpKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HostOpKind::Add => write!(f, "+"),
            HostOpKind::Sub => write!(f, "-"),
            HostOpKind::Mul => write!(f, "*"),
        }
    }
}

/// Consumers implement this trait to receive execution trace events.
pub trait ExecutionTracer {
    fn on_event(&mut self, event: TraceEvent);
}

/// Simple tracer that prints events using a pipeline-like layout.
pub struct PipelinePrinter<W: Write> {
    writer: W,
}

impl<W: Write> PipelinePrinter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    fn writeln(&mut self, line: &str) {
        let _ = writeln!(self.writer, "{line}");
    }
}

impl PipelinePrinter<std::io::Stdout> {
    pub fn stdout() -> Self {
        Self::new(std::io::stdout())
    }
}

impl<W: Write> ExecutionTracer for PipelinePrinter<W> {
    fn on_event(&mut self, event: TraceEvent) {
        match event {
            TraceEvent::Fetch {
                address,
                opcode,
                mnemonic,
                detail,
            } => self.writeln(&format!(
                "[Fetch] 0x{address:08X} 0x{opcode:08X} {mnemonic} {detail}",
                detail = detail.trim()
            )),
            TraceEvent::RegisterRead { name, value, width } => self.writeln(&format!(
                "[ Read]   {name} -> {}",
                format_value(value, width)
            )),
            TraceEvent::RegisterWrite { name, value, width } => self.writeln(&format!(
                "[Write]   {name} <- {}",
                format_value(value, width)
            )),
            TraceEvent::HostOp {
                op,
                args,
                result,
                carry,
            } => {
                if args.len() == 2 {
                    self.writeln(&format!(
                        "[IntOp]   0x{lhs:016X} {op} 0x{rhs:016X} = 0x{result:016X} (carry={carry})",
                        lhs = args[0],
                        rhs = args[1]
                    ));
                } else {
                    self.writeln(&format!(
                        "[IntOp]   {op} [{args}] -> 0x{result:016X} (carry={carry})",
                        args = format_arg_list(&args)
                    ));
                }
            }
        }
    }
}

fn format_value(value: i64, bits: u32) -> String {
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

fn format_arg_list(args: &[i64]) -> String {
    args.iter()
        .map(|value| format!("0x{value:X}"))
        .collect::<Vec<_>>()
        .join(", ")
}
