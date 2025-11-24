//! Intermediate representation for semantic blocks embedded in `.isa` files.

use std::sync::{Arc, OnceLock};

use crate::soc::isa::diagnostic::{DiagnosticLevel, DiagnosticPhase, IsaDiagnostic, SourceSpan};
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
    span: Option<SourceSpan>,
    compiled: OnceLock<Arc<SemanticProgram>>,
}

impl SemanticBlock {
    pub fn new(source: String) -> Self {
        Self::with_span(source, None)
    }

    pub fn with_span(source: String, span: Option<SourceSpan>) -> Self {
        Self {
            source,
            span,
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

    pub fn span(&self) -> Option<&SourceSpan> {
        self.span.as_ref()
    }

    pub fn program(&self) -> Option<&Arc<SemanticProgram>> {
        self.compiled.get()
    }

    pub fn ensure_program(&self) -> Result<&Arc<SemanticProgram>, IsaError> {
        if let Some(program) = self.compiled.get() {
            return Ok(program);
        }
        let program = SemanticProgram::parse_with_span(&self.source, self.span())
            .map_err(|err| self.decorate_error(err))?;
        let _ = self.compiled.set(Arc::new(program));
        self.compiled
            .get()
            .ok_or_else(|| IsaError::Machine("failed to store compiled program".into()))
    }

    fn decorate_error(&self, err: IsaError) -> IsaError {
        match (err, self.span.clone()) {
            (IsaError::Parser(message), Some(span)) => IsaError::Diagnostics {
                phase: DiagnosticPhase::Parser,
                diagnostics: vec![IsaDiagnostic::new(
                    DiagnosticPhase::Parser,
                    DiagnosticLevel::Error,
                    "semantics.syntax",
                    message,
                    Some(span),
                )],
            },
            (other, _) => other,
        }
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
