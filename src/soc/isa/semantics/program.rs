use std::path::PathBuf;

use crate::loader::isa::lexer::{Lexer, Token, TokenKind};
use crate::soc::isa::diagnostic::{SourcePosition, SourceSpan};
use crate::soc::isa::error::IsaError;
use crate::soc::prog::types::parse_u64_literal;

#[derive(Debug, Clone)]
pub struct SemanticProgram {
    pub statements: Vec<SemanticStmt>,
}

impl SemanticProgram {
    pub fn parse(source: &str) -> Result<Self, IsaError> {
        Self::parse_with_span(source, None)
    }

    pub fn parse_with_span(source: &str, span: Option<&SourceSpan>) -> Result<Self, IsaError> {
        let mut parser = Parser::with_span(source, span);
        parser.parse_program()
    }
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)] // Semantic AST mirrors source layout; boxing would complicate ownership
pub enum SemanticStmt {
    Assign { target: AssignTarget, expr: Expr },
    Expr(Expr),
    Return(Expr),
}

#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)] // Variants intentionally inline to keep parsing simple
pub enum AssignTarget {
    Variable(String),
    Tuple(Vec<String>),
    Register(RegisterRef),
}

#[derive(Debug, Clone)]
pub struct RegisterRef {
    pub space: String,
    pub name: String,
    pub subfield: Option<String>,
    pub index: Option<Expr>,
    pub span: Option<SourceSpan>,
}

#[derive(Debug, Clone)]
pub struct ContextCall {
    pub kind: ContextKind,
    pub space: String,
    pub name: String,
    pub subpath: Vec<String>,
    pub args: Vec<Expr>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextKind {
    Register,
    Macro,
    Instruction,
    Host,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(u64),
    Variable {
        name: String,
        span: SourceSpan,
    },
    Parameter {
        name: String,
        span: SourceSpan,
    },
    Call(ContextCall),
    Tuple(Vec<Expr>),
    BinaryOp {
        op: ExprBinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    UnaryOp {
        op: ExprUnaryOp,
        expr: Box<Expr>,
    },
    BitSlice {
        expr: Box<Expr>,
        slice: BitSlice,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum ExprBinaryOp {
    LogicalOr,
    LogicalAnd,
    BitOr,
    BitXor,
    BitAnd,
    Eq,
    Ne,
    Lt,
    Gt,
    Add,
    Sub,
}

#[derive(Debug, Clone, Copy)]
pub enum ExprUnaryOp {
    LogicalNot,
}

#[derive(Debug, Clone)]
pub struct BitSlice {
    pub start: u32,
    pub end: u32,
}

struct Parser<'src> {
    lexer: Lexer<'src>,
    peeked: Option<Token>,
}

impl<'src> Parser<'src> {
    fn with_span(source: &'src str, span: Option<&SourceSpan>) -> Self {
        let (path, line, column) = if let Some(span) = span {
            (span.path.clone(), span.start.line, span.start.column)
        } else {
            (PathBuf::from("<semantics>"), 1, 0)
        };
        Self {
            lexer: Lexer::with_origin(source, path, line, column),
            peeked: None,
        }
    }

    fn parse_program(&mut self) -> Result<SemanticProgram, IsaError> {
        let mut statements = Vec::new();
        while !self.check(TokenKind::EOF)? {
            if self.check(TokenKind::RBrace)? {
                break;
            }
            if self.match_token(TokenKind::Semicolon)? {
                continue;
            }
            let stmt = self.parse_statement()?;
            statements.push(stmt);
        }
        Ok(SemanticProgram { statements })
    }

    fn parse_statement(&mut self) -> Result<SemanticStmt, IsaError> {
        let expr = self.parse_expression()?;
        if self.is_assignment_target(&expr)? {
            self.expect(TokenKind::Equals, "'=' in assignment")?;
            let rhs = self.parse_expression()?;
            let target = AssignTarget::try_from_expr(expr)?;
            return Ok(SemanticStmt::Assign { target, expr: rhs });
        }
        if matches!(expr, Expr::Tuple(_)) {
            return Ok(SemanticStmt::Return(expr));
        }
        Ok(SemanticStmt::Expr(expr))
    }

    fn parse_expression(&mut self) -> Result<Expr, IsaError> {
        self.parse_logical_or()
    }

    fn parse_logical_or(&mut self) -> Result<Expr, IsaError> {
        let mut expr = self.parse_logical_and()?;
        while self.match_token(TokenKind::DoublePipe)? {
            let rhs = self.parse_logical_and()?;
            expr = Expr::BinaryOp {
                op: ExprBinaryOp::LogicalOr,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, IsaError> {
        let mut expr = self.parse_bit_or()?;
        while self.match_token(TokenKind::DoubleAmpersand)? {
            let rhs = self.parse_bit_or()?;
            expr = Expr::BinaryOp {
                op: ExprBinaryOp::LogicalAnd,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_bit_or(&mut self) -> Result<Expr, IsaError> {
        let mut expr = self.parse_bit_xor()?;
        while self.match_token(TokenKind::Pipe)? {
            let rhs = self.parse_bit_xor()?;
            expr = Expr::BinaryOp {
                op: ExprBinaryOp::BitOr,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_bit_xor(&mut self) -> Result<Expr, IsaError> {
        let mut expr = self.parse_bit_and()?;
        while self.match_token(TokenKind::Caret)? {
            let rhs = self.parse_bit_and()?;
            expr = Expr::BinaryOp {
                op: ExprBinaryOp::BitXor,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_bit_and(&mut self) -> Result<Expr, IsaError> {
        let mut expr = self.parse_equality()?;
        while self.match_token(TokenKind::Ampersand)? {
            let rhs = self.parse_equality()?;
            expr = Expr::BinaryOp {
                op: ExprBinaryOp::BitAnd,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
        }
        Ok(expr)
    }

    fn parse_equality(&mut self) -> Result<Expr, IsaError> {
        let mut expr = self.parse_relational()?;
        loop {
            if self.match_token(TokenKind::DoubleEquals)? {
                let rhs = self.parse_relational()?;
                expr = Expr::BinaryOp {
                    op: ExprBinaryOp::Eq,
                    lhs: Box::new(expr),
                    rhs: Box::new(rhs),
                };
                continue;
            }
            if self.match_token(TokenKind::BangEquals)? {
                let rhs = self.parse_relational()?;
                expr = Expr::BinaryOp {
                    op: ExprBinaryOp::Ne,
                    lhs: Box::new(expr),
                    rhs: Box::new(rhs),
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_relational(&mut self) -> Result<Expr, IsaError> {
        let mut expr = self.parse_term()?;
        loop {
            if self.match_token(TokenKind::LessThan)? {
                let rhs = self.parse_term()?;
                expr = Expr::BinaryOp {
                    op: ExprBinaryOp::Lt,
                    lhs: Box::new(expr),
                    rhs: Box::new(rhs),
                };
                continue;
            }
            if self.match_token(TokenKind::GreaterThan)? {
                let rhs = self.parse_term()?;
                expr = Expr::BinaryOp {
                    op: ExprBinaryOp::Gt,
                    lhs: Box::new(expr),
                    rhs: Box::new(rhs),
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_term(&mut self) -> Result<Expr, IsaError> {
        let mut expr = self.parse_factor()?;
        loop {
            if self.match_token(TokenKind::Plus)? {
                let rhs = self.parse_factor()?;
                expr = Expr::BinaryOp {
                    op: ExprBinaryOp::Add,
                    lhs: Box::new(expr),
                    rhs: Box::new(rhs),
                };
                continue;
            }
            if self.match_token(TokenKind::Dash)? {
                let rhs = self.parse_factor()?;
                expr = Expr::BinaryOp {
                    op: ExprBinaryOp::Sub,
                    lhs: Box::new(expr),
                    rhs: Box::new(rhs),
                };
                continue;
            }
            break;
        }
        Ok(expr)
    }

    fn parse_factor(&mut self) -> Result<Expr, IsaError> {
        if self.match_token(TokenKind::Bang)? {
            let expr = self.parse_factor()?;
            return Ok(Expr::UnaryOp {
                op: ExprUnaryOp::LogicalNot,
                expr: Box::new(expr),
            });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, IsaError> {
        let mut expr = self.parse_primary()?;
        while self.check(TokenKind::BitExpr)? {
            let token = self.consume()?;
            let slice = parse_bit_slice(&token.lexeme)?;
            expr = Expr::BitSlice {
                expr: Box::new(expr),
                slice,
            };
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, IsaError> {
        if self.match_token(TokenKind::LParen)? {
            let first = self.parse_expression()?;
            if self.match_token(TokenKind::Comma)? {
                let mut items = vec![first];
                items.push(self.parse_expression()?);
                while self.match_token(TokenKind::Comma)? {
                    items.push(self.parse_expression()?);
                }
                self.expect(TokenKind::RParen, "')' to close tuple")?;
                return Ok(Expr::Tuple(items));
            }
            self.expect(TokenKind::RParen, "')' to close expression")?;
            return Ok(first);
        }

        if self.check(TokenKind::Number)? {
            let token = self.consume()?;
            let value = parse_u64_literal(&token.lexeme).map_err(|err| {
                IsaError::Parser(format!("invalid numeric literal '{}': {err}", token.lexeme))
            })?;
            return Ok(Expr::Number(value));
        }

        if self.check(TokenKind::Identifier)? {
            let token = self.consume()?;
            let span = self.point_span(&token);
            let lexeme = token.lexeme;
            if let Some(stripped) = lexeme.strip_prefix('#') {
                return Ok(Expr::Parameter {
                    name: stripped.to_string(),
                    span,
                });
            }
            if let Some(stripped) = lexeme.strip_prefix('$') {
                let mut segments = vec![stripped.to_string()];
                while self.match_token(TokenKind::DoubleColon)? {
                    let seg = self.expect_identifier("context segment")?;
                    segments.push(seg);
                }
                let args = if self.match_token(TokenKind::LParen)? {
                    self.parse_argument_list()?
                } else {
                    Vec::new()
                };
                let kind = ContextKind::from_prefix(&segments[0])?;
                let space = segments[0].clone();
                let name = segments
                    .get(1)
                    .cloned()
                    .ok_or_else(|| IsaError::Parser("context reference missing name".into()))?;
                let subpath = segments.into_iter().skip(2).collect();
                return Ok(Expr::Call(ContextCall {
                    kind,
                    space,
                    name,
                    subpath,
                    args,
                    span,
                }));
            }
            return Ok(Expr::Variable { name: lexeme, span });
        }

        Err(IsaError::Parser(
            "unexpected token in semantic expression".into(),
        ))
    }

    fn parse_argument_list(&mut self) -> Result<Vec<Expr>, IsaError> {
        let mut args = Vec::new();
        if self.check(TokenKind::RParen)? {
            self.consume()?;
            return Ok(args);
        }
        loop {
            args.push(self.parse_expression()?);
            if self.match_token(TokenKind::Comma)? {
                continue;
            }
            self.expect(TokenKind::RParen, "')' to close argument list")?;
            break;
        }
        Ok(args)
    }

    fn is_assignment_target(&mut self, expr: &Expr) -> Result<bool, IsaError> {
        if !self.check(TokenKind::Equals)? {
            return Ok(false);
        }
        if matches!(expr, Expr::Variable { .. } | Expr::Tuple(_)) {
            return Ok(true);
        }
        if let Expr::Call(call) = expr {
            return Ok(call.kind == ContextKind::Register);
        }
        Ok(false)
    }

    fn expect(&mut self, kind: TokenKind, context: &str) -> Result<Token, IsaError> {
        let token = self.consume()?;
        if token.kind == kind {
            Ok(token)
        } else {
            Err(IsaError::Parser(format!("expected {context}")))
        }
    }

    fn expect_identifier(&mut self, context: &str) -> Result<String, IsaError> {
        let token = self.expect(TokenKind::Identifier, context)?;
        Ok(token.lexeme)
    }

    fn point_span(&self, token: &Token) -> SourceSpan {
        SourceSpan::point(
            self.lexer.path().clone(),
            SourcePosition::new(token.line, token.column),
        )
    }

    fn match_token(&mut self, kind: TokenKind) -> Result<bool, IsaError> {
        if self.check(kind.clone())? {
            self.consume()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn check(&mut self, kind: TokenKind) -> Result<bool, IsaError> {
        Ok(self.peek()?.kind == kind)
    }

    fn peek(&mut self) -> Result<&Token, IsaError> {
        if self.peeked.is_none() {
            self.peeked = Some(self.lexer.next_token()?);
        }
        Ok(self.peeked.as_ref().expect("peeked token"))
    }

    fn consume(&mut self) -> Result<Token, IsaError> {
        if let Some(token) = self.peeked.take() {
            return Ok(token);
        }
        self.lexer.next_token()
    }
}

impl AssignTarget {
    fn try_from_expr(expr: Expr) -> Result<Self, IsaError> {
        match expr {
            Expr::Variable { name, .. } => Ok(AssignTarget::Variable(name)),
            Expr::Tuple(items) => {
                let mut names = Vec::new();
                for item in items {
                    if let Expr::Variable { name, .. } = item {
                        names.push(name);
                    } else {
                        return Err(IsaError::Parser(
                            "tuple assignment only supports identifier members".into(),
                        ));
                    }
                }
                Ok(AssignTarget::Tuple(names))
            }
            Expr::Call(call) if call.kind == ContextKind::Register => {
                let index = call.args.into_iter().next();
                let reference = RegisterRef {
                    space: call.space,
                    name: call.name,
                    subfield: call.subpath.into_iter().next(),
                    index,
                    span: Some(call.span),
                };
                Ok(AssignTarget::Register(reference))
            }
            _ => Err(IsaError::Parser(
                "unsupported assignment target in semantics block".into(),
            )),
        }
    }
}

impl ContextKind {
    fn from_prefix(prefix: &str) -> Result<Self, IsaError> {
        match prefix.to_ascii_lowercase().as_str() {
            "reg" => Ok(ContextKind::Register),
            "macro" => Ok(ContextKind::Macro),
            "insn" => Ok(ContextKind::Instruction),
            "host" => Ok(ContextKind::Host),
            other => Err(IsaError::Parser(format!(
                "unknown context prefix '${other}'"
            ))),
        }
    }
}

fn parse_bit_slice(spec: &str) -> Result<BitSlice, IsaError> {
    let inner = spec
        .strip_prefix("@(")
        .and_then(|rest| rest.strip_suffix(')'))
        .ok_or_else(|| IsaError::Parser(format!("invalid bit slice '{spec}'")))?;
    let mut parts = inner.split("..");
    let start = parts
        .next()
        .ok_or_else(|| IsaError::Parser("bit slice missing start".into()))?;
    let end = parts.next();
    let start_val = parse_u64_literal(start)
        .map_err(|err| IsaError::Parser(format!("invalid bit slice start '{}': {err}", start)))?;
    let end_val = if let Some(end_str) = end {
        parse_u64_literal(end_str).map_err(|err| {
            IsaError::Parser(format!("invalid bit slice end '{}': {err}", end_str))
        })?
    } else {
        start_val
    };
    if end_val < start_val {
        return Err(IsaError::Parser(format!(
            "bit slice end {} before start {}",
            end_val, start_val
        )));
    }
    Ok(BitSlice {
        start: start_val as u32,
        end: end_val as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::SemanticProgram;

    #[test]
    fn parses_postfix_bit_slices() {
        let source = "\n    neg = #res@(0) == 1\n";
        SemanticProgram::parse(source).expect("bit slice after parameter should parse");
    }

    #[test]
    fn parses_logical_not_prefix() {
        let source = "\n    $reg::CR0::POS = !neg\n";
        SemanticProgram::parse(source).expect("logical not operator should parse");
    }
}
