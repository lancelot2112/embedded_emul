//! Intermediate representation for semantic blocks embedded in `.isa` files.

use std::sync::Arc;

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
    compiled: Option<Arc<SemanticProgram>>,
}

impl SemanticBlock {
    pub fn new(source: String) -> Self {
        Self {
            source,
            compiled: None,
        }
    }

    pub fn from_source(source: String) -> Self {
        Self::new(source)
    }

    pub fn empty() -> Self {
        Self::from_source(String::new())
    }

    pub fn set_program(&mut self, program: SemanticProgram) {
        self.compiled = Some(Arc::new(program));
    }

    pub fn program(&self) -> Option<&Arc<SemanticProgram>> {
        self.compiled.as_ref()
    }

    pub fn ensure_program(&mut self) -> Result<&Arc<SemanticProgram>, IsaError> {
        if self.compiled.is_none() {
            let program = SemanticProgram::parse(&self.source)?;
            self.compiled = Some(Arc::new(program));
        }
        Ok(self.compiled.as_ref().expect("program must exist"))
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
