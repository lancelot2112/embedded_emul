//! Recursive descent parser that turns lexer tokens into [`IsaDocument`](crate::soc::isa::ast::IsaDocument).

mod document;
mod directives;
mod literals;
mod parameters;

pub use document::{parse_str, Parser};

pub(super) use super::lexer::{Lexer, Token, TokenKind};
