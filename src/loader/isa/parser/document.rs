use std::path::PathBuf;

use crate::soc::isa::ast::IsaDocument;
use crate::soc::isa::error::IsaError;

use super::{Lexer, Token, TokenKind};

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
        let mut items = Vec::new();
        while !self.check(TokenKind::EOF)? {
            items.push(self.parse_directive()?);
        }
        Ok(IsaDocument::new(path, items))
    }

    pub(super) fn expect_identifier(&mut self, context: &str) -> Result<String, IsaError> {
        let token = self.consume()?;
        if token.kind == TokenKind::Identifier {
            Ok(token.lexeme)
        } else {
            Err(IsaError::Parser(format!("expected identifier for {context}")))
        }
    }

    pub(super) fn expect(&mut self, kind: TokenKind, context: &str) -> Result<Token, IsaError> {
        let token = self.consume()?;
        if token.kind == kind {
            Ok(token)
        } else {
            Err(IsaError::Parser(format!("expected {context}")))
        }
    }

    fn check(&mut self, kind: TokenKind) -> Result<bool, IsaError> {
        Ok(self.peek()?.kind == kind)
    }

    fn peek(&mut self) -> Result<&Token, IsaError> {
        if self.peeked.is_none() {
            self.peeked = Some(self.lexer.next_token()?);
        }
        Ok(self.peeked.as_ref().expect("peeked token must exist"))
    }

    pub(super) fn consume(&mut self) -> Result<Token, IsaError> {
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
