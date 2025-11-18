use std::collections::HashMap;
use std::path::PathBuf;

use crate::soc::isa::ast::{IsaDocument, SpaceKind};
use crate::soc::isa::error::IsaError;

use super::{Lexer, Token, TokenKind};

pub struct Parser<'src> {
    lexer: Lexer<'src>,
    peeked: Option<Token>,
    known_spaces: HashMap<String, SpaceKind>,
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            lexer: Lexer::new(source),
            peeked: None,
            known_spaces: HashMap::new(),
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

    pub(super) fn check(&mut self, kind: TokenKind) -> Result<bool, IsaError> {
        Ok(self.peek()?.kind == kind)
    }

    pub(super) fn peek(&mut self) -> Result<&Token, IsaError> {
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

impl<'src> Parser<'src> {
    pub(super) fn register_space(&mut self, name: &str, kind: SpaceKind) {
        self.known_spaces.insert(name.to_string(), kind);
    }

    pub(super) fn is_known_space(&self, name: &str) -> bool {
        self.known_spaces.contains_key(name)
    }

    pub(super) fn space_kind(&self, name: &str) -> Option<SpaceKind> {
        self.known_spaces.get(name).cloned()
    }
}

/// Convenience helper used by the loader when parsing files without needing to hold onto the
/// parser instance.
pub fn parse_str(path: PathBuf, src: &str) -> Result<IsaDocument, IsaError> {
    let mut parser = Parser::new(src);
    parser.parse_document(path)
}
