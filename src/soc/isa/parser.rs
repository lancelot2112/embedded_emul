//! Recursive descent parser that turns lexer tokens into [`IsaDocument`](crate::soc::isa::ast::IsaDocument).

use std::path::PathBuf;

use super::ast::IsaDocument;
use super::error::IsaError;
use super::lexer::{Lexer, Token, TokenKind};

pub struct Parser<'src> {
    lexer: Lexer<'src>,
    peeked: Option<Token>,
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            lexer: Lexer::new(source),
            peeked: None,
        }
    }

    pub fn parse_document(&mut self, path: PathBuf) -> Result<IsaDocument, IsaError> {
        let items = Vec::new();
        while self.peek()?.kind != TokenKind::EOF {
            // Placeholder: once the grammar is implemented each directive will be parsed here.
            self.consume()?;
        }
        Ok(IsaDocument::new(path, items))
    }

    fn peek(&mut self) -> Result<&Token, IsaError> {
        if self.peeked.is_none() {
            self.peeked = Some(self.lexer.next_token()?);
        }
        Ok(self.peeked.as_ref().expect("peeked token must exist"))
    }

    fn consume(&mut self) -> Result<Token, IsaError> {
        if let Some(token) = self.peeked.take() {
            return Ok(token);
        }
        self.lexer.next_token()
    }
}

/// Convenience helper used by the loader when parsing files without needing to hold onto the
/// parser instance.
pub fn parse_str(path: PathBuf, src: &str) -> Result<IsaDocument, IsaError> {
    let mut parser = Parser::new(src);
    parser.parse_document(path)
}
