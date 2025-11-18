//! Intermediate representation for semantic blocks embedded in `.isa` files.

/// A semantic block captures structured actions (register transfers, flag updates, etc.).
#[derive(Debug, Clone)]
pub struct SemanticBlock {
    pub ops: Vec<SemanticOp>,
}

impl SemanticBlock {
    pub fn new(ops: Vec<SemanticOp>) -> Self {
        Self { ops }
    }
}

#[derive(Debug, Clone)]
pub enum SemanticOp {
    Assign { target: String, expr: SemanticExpr },
    Call { func: String, args: Vec<SemanticExpr> },
}

#[derive(Debug, Clone)]
pub enum SemanticExpr {
    Literal(u64),
    Identifier(String),
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
}
