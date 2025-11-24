//! Core-level runtime primitives including processor descriptors and mutable state
//! snapshots backed by the shared bus abstractions.

pub mod harness;
pub mod isa;
pub mod specification;
pub mod state;

pub use harness::{ExecutionHarness, HarnessError, InstructionExecution};
pub use isa::{InstructionSemantics, IsaSpec, IsaSpecError};
pub use specification::{
    CoreSpec, CoreSpecBuildError, CoreSpecBuilder, CoreSpecError, RegisterSpec,
};
pub use state::{CoreState, RegisterLayout, StateError, StateResult};
