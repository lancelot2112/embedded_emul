//! ISA parsing, validation, and runtime interpretation pipeline.
//!
//! This module houses a staged architecture that turns `.isa` / `.isaext` sources into
//! a validated [`MachineDescription`](machine/struct.MachineDescription.html) capable of
//! disassembling binary streams and emitting IR semantics.

pub mod ast;
pub mod error;
pub mod handle;
pub mod machine;
pub mod semantics;
pub mod validator;

pub use handle::IsaHandle;
pub use machine::{Disassembly, MachineDescription};
