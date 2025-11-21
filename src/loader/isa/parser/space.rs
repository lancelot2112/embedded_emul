use crate::soc::device::endianness::Endianness;
use crate::soc::isa::ast::{IsaItem, SpaceAttribute, SpaceDecl, SpaceKind};
use crate::soc::isa::error::IsaError;
use crate::soc::prog::types::parse_u32_literal;

use super::spans::span_from_tokens;
use super::{Parser, TokenKind, parse_semantic_expr_block};

pub(super) fn parse_space_directive(parser: &mut Parser) -> Result<IsaItem, IsaError> {
    let name_token = parser.expect_identifier_token("space name")?;
    let name = name_token.lexeme.clone();
    let mut kind: Option<SpaceKind> = None;
    let mut attributes = Vec::new();
    let mut has_addr = false;
    let mut has_word = false;
    let mut enable_expr = None;

    while !parser.check(TokenKind::EOF)? && !parser.check(TokenKind::Colon)? {
        let attr_name = parser.expect_identifier("space attribute name")?;
        parser.expect(TokenKind::Equals, "'=' after space attribute name")?;
        match attr_name.to_ascii_lowercase().as_str() {
            "addr" => {
                let value = parser.expect(TokenKind::Number, "numeric value for addr")?;
                let bits = parse_u32_attr_literal(&value.lexeme, "addr")?;
                attributes.push(SpaceAttribute::AddressBits(bits));
                has_addr = true;
            }
            "word" => {
                let value = parser.expect(TokenKind::Number, "numeric value for word")?;
                let bits = parse_u32_attr_literal(&value.lexeme, "word")?;
                attributes.push(SpaceAttribute::WordSize(bits));
                has_word = true;
            }
            "align" => {
                let value = parser.expect(TokenKind::Number, "numeric value for align")?;
                let bytes = parse_u32_attr_literal(&value.lexeme, "align")?;
                attributes.push(SpaceAttribute::Alignment(bytes));
            }
            "type" => {
                let value = parser.expect(TokenKind::Identifier, "space type value")?;
                kind = Some(parse_space_kind(&value.lexeme)?);
            }
            "endian" => {
                let value = parser.expect(TokenKind::Identifier, "endianness value")?;
                let endianness = parse_endianness(&value.lexeme)?;
                attributes.push(SpaceAttribute::Endianness(endianness));
            }
            "enbl" => {
                if enable_expr.is_some() {
                    return Err(IsaError::Parser(
                        "enbl attribute can only be specified once per :space".into(),
                    ));
                }
                let expr = parse_semantic_expr_block(parser, "enbl expression")?;
                enable_expr = Some(expr);
            }
            other => {
                return Err(IsaError::Parser(format!(
                    "unknown :space attribute '{other}'"
                )));
            }
        }
    }

    let kind = kind.ok_or_else(|| IsaError::Parser(":space requires a type attribute".into()))?;
    if !has_addr {
        return Err(IsaError::Parser(":space requires an addr attribute".into()));
    }
    if !has_word {
        return Err(IsaError::Parser(":space requires a word attribute".into()));
    }

    parser.register_space(&name, kind.clone());
    let end_token = parser
        .last_consumed_token()
        .cloned()
        .unwrap_or_else(|| name_token.clone());
    let span = span_from_tokens(parser.file_path(), &name_token, &end_token);
    Ok(IsaItem::Space(SpaceDecl {
        name,
        kind,
        attributes,
        span,
        enable: enable_expr,
    }))
}

fn parse_u32_attr_literal(text: &str, context: &str) -> Result<u32, IsaError> {
    parse_u32_literal(text).map_err(|err| {
        IsaError::Parser(format!(
            "invalid numeric literal '{text}' for {context}: {err}"
        ))
    })
}

fn parse_space_kind(raw: &str) -> Result<SpaceKind, IsaError> {
    match raw.to_ascii_lowercase().as_str() {
        "rw" => Ok(SpaceKind::ReadWrite),
        "ro" => Ok(SpaceKind::ReadOnly),
        "memio" => Ok(SpaceKind::MemoryMappedIo),
        "register" => Ok(SpaceKind::Register),
        "logic" => Ok(SpaceKind::Logic),
        other => Err(IsaError::Parser(format!("unknown space type '{other}'"))),
    }
}

fn parse_endianness(raw: &str) -> Result<Endianness, IsaError> {
    match raw.to_ascii_lowercase().as_str() {
        "big" => Ok(Endianness::Big),
        "little" => Ok(Endianness::Little),
        other => Err(IsaError::Parser(format!("unknown endianness '{other}'"))),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::soc::isa::ast::{IsaItem, SpaceAttribute, SpaceKind};
    use crate::soc::isa::diagnostic::DiagnosticPhase;
    use crate::soc::isa::error::IsaError;

    use super::super::parse_str;

    fn parse(source: &str) -> crate::soc::isa::ast::IsaSpecification {
        parse_str(PathBuf::from("test.isa"), source).expect("parse")
    }

    fn expect_parser_diag(err: IsaError, needle: &str) {
        match err {
            IsaError::Diagnostics {
                phase: DiagnosticPhase::Parser,
                diagnostics,
            } => {
                assert!(
                    diagnostics.iter().any(|diag| diag.message.contains(needle)),
                    "diagnostics missing '{needle}': {diagnostics:?}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parses_space_basic_attributes() {
        let doc = parse(":space reg addr=32 word=64 type=register align=64 endian=big");
        match &doc.items[0] {
            IsaItem::Space(space) => {
                assert_eq!(space.name, "reg");
                assert_eq!(space.kind, SpaceKind::Register);
                assert!(space.attributes.contains(&SpaceAttribute::AddressBits(32)));
                assert!(space.attributes.contains(&SpaceAttribute::WordSize(64)));
                assert!(space.attributes.contains(&SpaceAttribute::Alignment(64)));
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn parses_logic_space_size() {
        let doc = parse(":space powerpc_insn addr=32 word=32 type=logic endian=big");
        match &doc.items[0] {
            IsaItem::Space(space) => {
                assert_eq!(space.name, "powerpc_insn");
                assert_eq!(space.kind, SpaceKind::Logic);
                assert!(space.attributes.contains(&SpaceAttribute::WordSize(32)));
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn rejects_space_without_type() {
        let err = parse_str(PathBuf::from("test.isa"), ":space reg addr=32").unwrap_err();
        expect_parser_diag(err, "requires a type");
    }

    #[test]
    fn rejects_space_without_addr() {
        let err = parse_str(
            PathBuf::from("test.isa"),
            ":space reg word=64 type=register",
        )
        .unwrap_err();
        expect_parser_diag(err, "requires an addr");
    }

    #[test]
    fn rejects_space_without_word() {
        let err = parse_str(
            PathBuf::from("test.isa"),
            ":space reg addr=32 type=register",
        )
        .unwrap_err();
        expect_parser_diag(err, "requires a word");
    }

    #[test]
    fn rejects_unknown_space_attribute() {
        let err = parse_str(
            PathBuf::from("test.isa"),
            ":space reg addr=32 word=64 type=register foo=bar",
        )
        .unwrap_err();
        expect_parser_diag(err, "unknown :space attribute");
    }

    #[test]
    fn rejects_size_attribute_hinting_to_use_word() {
        let err = parse_str(
            PathBuf::from("test.isa"),
            ":space logic addr=32 word=32 type=logic size=32",
        )
        .unwrap_err();
        expect_parser_diag(err, "unknown :space attribute");
    }

    #[test]
    fn registers_space_names_for_followup_contexts() {
        let doc = parse(
            ":space reg addr=32 word=64 type=register\n:reg GPR size=64 subfields={\n    VALUE @(0..63)\n}",
        );
        assert_eq!(doc.items.len(), 2, "space definition plus field member");
        assert!(matches!(doc.items[0], IsaItem::Space(_)));
        assert!(matches!(doc.items[1], IsaItem::SpaceMember(_)));
    }
}
