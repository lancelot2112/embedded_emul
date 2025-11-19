//! Recursive descent parser that turns lexer tokens into [`IsaDocument`](crate::soc::isa::ast::IsaDocument).

mod directives;
mod parameters;
mod space;
mod space_context;
mod spans;
mod specification;

pub use specification::{Parser, parse_str};

pub(super) use super::lexer::{Lexer, Token, TokenKind};
