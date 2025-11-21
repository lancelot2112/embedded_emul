//! Intermediate representation for semantic blocks embedded in `.isa` files.

/// A semantic block captures the original source plus any parsed operations.
#[derive(Debug, Clone)]
pub struct SemanticBlock {
    /// Raw source extracted from the `.isa` file between `{` and `}`.
    pub source: String,
    /// Structured operations (unused for now, but reserved for future lowering passes).
    pub ops: Vec<SemanticOp>,
}

impl SemanticBlock {
    pub fn new(source: String, ops: Vec<SemanticOp>) -> Self {
        Self { source, ops }
    }

    pub fn from_source(source: String) -> Self {
        Self::new(source, Vec::new())
    }

    pub fn empty() -> Self {
        Self::from_source(String::new())
    }
}

#[derive(Debug, Clone)]
pub enum SemanticOp {
    Assign {
        target: String,
        expr: SemanticExpr,
    },
    Call {
        func: String,
        args: Vec<SemanticExpr>,
    },
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
