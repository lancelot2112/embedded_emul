use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::soc::isa::ast::{IsaSpecification, SpaceKind};
use crate::soc::isa::diagnostic::{DiagnosticLevel, DiagnosticPhase, IsaDiagnostic, SourceSpan};
use crate::soc::isa::error::IsaError;

use super::spans::span_from_token;
use super::{Lexer, Token, TokenKind};

pub struct Parser<'src> {
    lexer: Lexer<'src>,
    peeked: Option<Token>,
    last_token: Option<Token>,
    known_spaces: HashMap<String, SpaceKind>,
    path: PathBuf,
    diagnostics: Vec<IsaDiagnostic>,
}

impl<'src> Parser<'src> {
    pub fn new(source: &'src str, path: PathBuf) -> Self {
        Self {
            lexer: Lexer::new(source, path.clone()),
            peeked: None,
            last_token: None,
            known_spaces: HashMap::new(),
            path,
            diagnostics: Vec::new(),
        }
    }

    pub fn parse_document(&mut self) -> Result<IsaSpecification, IsaError> {
        let mut items = Vec::new();
        while !self.check(TokenKind::EOF)? {
            match self.parse_directive() {
                Ok(item) => items.push(item),
                Err(err) => self.handle_parse_error(err)?,
            }
        }

        if self.diagnostics.is_empty() {
            Ok(IsaSpecification::new(self.path.clone(), items))
        } else {
            Err(IsaError::Diagnostics {
                phase: DiagnosticPhase::Parser,
                diagnostics: std::mem::take(&mut self.diagnostics),
            })
        }
    }

    pub(super) fn expect_identifier_token(&mut self, context: &str) -> Result<Token, IsaError> {
        let token = self.consume()?;
        if token.kind == TokenKind::Identifier {
            Ok(token)
        } else {
            Err(IsaError::Parser(format!(
                "expected identifier for {context}"
            )))
        }
    }

    pub(super) fn expect_identifier(&mut self, context: &str) -> Result<String, IsaError> {
        Ok(self.expect_identifier_token(context)?.lexeme)
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
        let token = if let Some(token) = self.peeked.take() {
            token
        } else {
            self.lexer.next_token()?
        };
        self.last_token = Some(token.clone());
        Ok(token)
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

    pub(super) fn file_path(&self) -> &Path {
        &self.path
    }

    pub(super) fn last_consumed_token(&self) -> Option<&Token> {
        self.last_token.as_ref()
    }

    fn handle_parse_error(&mut self, err: IsaError) -> Result<(), IsaError> {
        match err {
            IsaError::Parser(msg) => {
                self.push_parser_diagnostic(msg);
                self.synchronize_directive();
                Ok(())
            }
            IsaError::Diagnostics {
                phase: DiagnosticPhase::Parser,
                diagnostics,
            } => {
                self.diagnostics.extend(diagnostics);
                self.synchronize_directive();
                Ok(())
            }
            other => Err(other),
        }
    }

    fn push_parser_diagnostic(&mut self, message: String) {
        let span = self.current_error_span();
        self.diagnostics.push(IsaDiagnostic::new(
            DiagnosticPhase::Parser,
            DiagnosticLevel::Error,
            "parser.syntax",
            message,
            span,
        ));
    }

    fn current_error_span(&mut self) -> Option<SourceSpan> {
        if let Some(token) = self.peeked.as_ref() {
            return Some(span_from_token(self.file_path(), token));
        }
        if let Some(token) = self.last_token.as_ref() {
            return Some(span_from_token(self.file_path(), token));
        }
        let path = self.path.clone();
        self.peek()
            .ok()
            .map(|token| span_from_token(path.as_path(), token))
    }

    fn synchronize_directive(&mut self) {
        loop {
            match self.peek() {
                Ok(token) if token.kind == TokenKind::Colon || token.kind == TokenKind::EOF => {
                    break;
                }
                Ok(_) => {
                    if self.consume().is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    }
}

/// Convenience helper used by the loader when parsing files without needing to hold onto the
/// parser instance.
pub fn parse_str(path: PathBuf, src: &str) -> Result<IsaSpecification, IsaError> {
    let mut parser = Parser::new(src, path);
    parser.parse_document()
}
