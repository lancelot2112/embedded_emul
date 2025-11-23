//! Intermediate representation for semantic blocks embedded in `.isa` files.

use std::sync::{Arc, OnceLock};

use crate::soc::isa::error::IsaError;

pub mod bindings;
pub mod context;
pub mod expression;
pub mod program;
pub mod register;
pub mod runtime;
pub mod value;

pub use bindings::{OperandBinder, ParameterBindings};
pub use program::SemanticProgram;

/// A semantic block captures the original source plus any parsed operations.
#[derive(Debug, Clone)]
pub struct SemanticBlock {
    /// Raw source extracted from the `.isa` file between `{` and `}`.
    pub source: String,
    compiled: OnceLock<Arc<SemanticProgram>>,
}

impl SemanticBlock {
    pub fn new(source: String) -> Self {
        Self {
            source,
            compiled: OnceLock::new(),
        }
    }

    pub fn from_source(source: String) -> Self {
        Self::new(source)
    }

    pub fn empty() -> Self {
        Self::from_source(String::new())
    }

    pub fn set_program(&mut self, program: SemanticProgram) {
        let _ = self.compiled.set(Arc::new(program));
    }

    pub fn program(&self) -> Option<&Arc<SemanticProgram>> {
        self.compiled.get()
    }

    pub fn ensure_program(&self) -> Result<&Arc<SemanticProgram>, IsaError> {
        if let Some(program) = self.compiled.get() {
            return Ok(program);
        }
        let program = SemanticProgram::parse(&self.source)?;
        let _ = self.compiled.set(Arc::new(program));
        self.compiled
            .get()
            .ok_or_else(|| IsaError::Machine("failed to store compiled program".into()))
    }
}

#[derive(Debug, Clone)]
pub enum SemanticExpr {
    Literal(u64),
    Identifier(String),
    BitExpr(String),
    BinaryOp {
        op: BinaryOperator,
        lhs: Box<SemanticExpr>,
        rhs: Box<SemanticExpr>,
    },
}

#[derive(Debug, Clone)]
pub enum BinaryOperator {
    Add,
    Sub,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Eq,
    Ne,
    LogicalAnd,
    LogicalOr,
}
