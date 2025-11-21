//! Recursive descent parser that turns lexer tokens into [`IsaDocument`](crate::soc::isa::ast::IsaDocument).

mod directives;
mod parameters;
mod semantics;
mod space;
mod space_context;
mod spans;
mod specification;

pub use specification::{Parser, parse_str, parse_str_with_spaces};

pub(super) use super::lexer::{Lexer, Token, TokenKind};
pub(super) use semantics::parse_semantic_expr_block;
