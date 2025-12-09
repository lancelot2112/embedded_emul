use crate::soc::isa::error::IsaError;
use crate::soc::isa::semantics::{BinaryOperator, SemanticExpr};
use crate::soc::prog::types::parse_u64_literal;

use super::{Parser, TokenKind, spans::span_from_token};

pub(crate) fn parse_semantic_expr_block(
    parser: &mut Parser,
    context: &str,
) -> Result<SemanticExpr, IsaError> {
    parser.expect(TokenKind::LBrace, &format!("'{{' to start {context}"))?;
    let expr = parse_or_expr(parser)?;
    parser.expect(TokenKind::RBrace, &format!("'}}' to close {context}"))?;
    Ok(expr)
}

fn parse_or_expr(parser: &mut Parser) -> Result<SemanticExpr, IsaError> {
    let mut expr = parse_and_expr(parser)?;
    loop {
        if match_logical_or(parser)? {
            let rhs = parse_and_expr(parser)?;
            expr = SemanticExpr::BinaryOp {
                op: BinaryOperator::LogicalOr,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
            continue;
        }
        break;
    }
    Ok(expr)
}

fn parse_and_expr(parser: &mut Parser) -> Result<SemanticExpr, IsaError> {
    let mut expr = parse_equality_expr(parser)?;
    loop {
        if match_logical_and(parser)? {
            let rhs = parse_equality_expr(parser)?;
            expr = SemanticExpr::BinaryOp {
                op: BinaryOperator::LogicalAnd,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
            continue;
        }
        break;
    }
    Ok(expr)
}

fn parse_equality_expr(parser: &mut Parser) -> Result<SemanticExpr, IsaError> {
    let mut expr = parse_primary_expr(parser)?;
    loop {
        if parser.check(TokenKind::DoubleEquals)? {
            parser.consume()?;
            let rhs = parse_primary_expr(parser)?;
            expr = SemanticExpr::BinaryOp {
                op: BinaryOperator::Eq,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
            continue;
        }
        if parser.check(TokenKind::BangEquals)? {
            parser.consume()?;
            let rhs = parse_primary_expr(parser)?;
            expr = SemanticExpr::BinaryOp {
                op: BinaryOperator::Ne,
                lhs: Box::new(expr),
                rhs: Box::new(rhs),
            };
            continue;
        }
        if parser.check(TokenKind::Bang)? {
            parser.consume()?;
            return Err(IsaError::Parser(
                "logical operator '!=' requires '=' after '!'".into(),
            ));
        }
        break;
    }
    Ok(expr)
}

fn parse_primary_expr(parser: &mut Parser) -> Result<SemanticExpr, IsaError> {
    if parser.check(TokenKind::LParen)? {
        parser.consume()?;
        let expr = parse_or_expr(parser)?;
        parser.expect(TokenKind::RParen, "')' to close semantic expression")?;
        return Ok(expr);
    }
    if parser.check(TokenKind::BitExpr)? {
        let token = parser.consume()?;
        let span = span_from_token(parser.file_path(), &token);
        return Ok(SemanticExpr::BitExpr {
            spec: token.lexeme,
            span,
        });
    }
    if parser.check(TokenKind::Number)? {
        let token = parser.consume()?;
        let span = span_from_token(parser.file_path(), &token);
        let literal = token.lexeme.clone();
        let value = parse_u64_literal(&literal).map_err(|err| {
            IsaError::Parser(format!(
                "invalid numeric literal '{}' in semantic expression: {err}",
                token.lexeme
            ))
        })?;
        return Ok(SemanticExpr::Literal {
            value,
            text: literal,
            span,
        });
    }
    if parser.check(TokenKind::Identifier)? {
        let token = parser.consume()?;
        return Ok(SemanticExpr::Identifier(token.lexeme));
    }
    Err(IsaError::Parser(
        "unexpected token in semantic expression".into(),
    ))
}

fn match_logical_and(parser: &mut Parser) -> Result<bool, IsaError> {
    if parser.check(TokenKind::DoubleAmpersand)? {
        parser.consume()?;
        return Ok(true);
    }
    if parser.check(TokenKind::Ampersand)? {
        parser.consume()?;
        return Err(IsaError::Parser(
            "logical operator '&&' requires two '&' tokens".into(),
        ));
    }
    Ok(false)
}

fn match_logical_or(parser: &mut Parser) -> Result<bool, IsaError> {
    if parser.check(TokenKind::DoublePipe)? {
        parser.consume()?;
        return Ok(true);
    }
    if parser.check(TokenKind::Pipe)? {
        parser.consume()?;
        return Err(IsaError::Parser(
            "logical operator '||' requires two '|' tokens".into(),
        ));
    }
    Ok(false)
}
