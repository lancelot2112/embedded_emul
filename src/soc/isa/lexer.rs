//! Streaming tokenizer for `.isa`-family source files.

use super::error::IsaError;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub lexeme: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Colon,
    Identifier,
    Number,
    String,
    LBrace,
    RBrace,
    LParen,
    RParen,
    Pipe,
    Equals,
    Comma,
    At,
    Question,
    EOF,
}

pub struct Lexer<'src> {
    src: &'src str,
    line: usize,
    column: usize,
}

impl<'src> Lexer<'src> {
    pub fn new(src: &'src str) -> Self {
        Self {
            src,
            line: 1,
            column: 0,
        }
    }

    /// Produces the next token. This is a placeholder implementation until the full tokenizer is
    /// ported from the existing linter.
    pub fn next_token(&mut self) -> Result<Token, IsaError> {
        let _ = self.src;
        Ok(Token {
            kind: TokenKind::EOF,
            lexeme: String::new(),
            line: self.line,
            column: self.column,
        })
    }
}
