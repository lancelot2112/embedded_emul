use crate::soc::isa::ast::{ParameterDecl, ParameterValue};
use crate::soc::isa::error::IsaError;

use super::{literals::parse_numeric_literal, Parser, TokenKind};

pub(super) fn parse_parameter_decl(
    parser: &mut Parser,
    name_context: &str,
) -> Result<ParameterDecl, IsaError> {
    let name = parser.expect_identifier(name_context)?;
    parser.expect(TokenKind::Equals, "'=' after parameter name")?;
    let value = parse_parameter_value(parser)?;
    Ok(ParameterDecl { name, value })
}

fn parse_parameter_value(parser: &mut Parser) -> Result<ParameterValue, IsaError> {
    let token = parser.consume()?;
    match token.kind {
        TokenKind::String | TokenKind::Identifier => Ok(ParameterValue::Word(token.lexeme)),
        TokenKind::Number => {
            let value = parse_numeric_literal(&token.lexeme).map_err(|err| {
                IsaError::Parser(format!("invalid numeric literal '{}': {err}", token.lexeme))
            })?;
            Ok(ParameterValue::Number(value))
        }
        other => Err(IsaError::Parser(format!(
            "unexpected token {:?} when parsing parameter value",
            other
        ))),
    }
}
