use crate::soc::isa::ast::{
    ContextReference,
    FieldDecl,
    FieldIndexRange,
    IsaItem,
    SpaceKind,
    SpaceMember,
    SpaceMemberDecl,
    SubFieldDecl,
    SubFieldOp,
};
use crate::soc::isa::error::IsaError;

use super::{
    literals::parse_numeric_literal,
    Parser,
    Token,
    TokenKind,
};

pub(super) fn parse_space_context_directive(
    parser: &mut Parser,
    space: &str,
    kind: SpaceKind,
) -> Result<IsaItem, IsaError> {
    match kind {
        SpaceKind::Logic => Err(IsaError::Parser(format!(
            "logic space :{space} contexts are not supported yet"
        ))),
        _ => parse_register_form(parser, space),
    }
}

fn parse_register_form(parser: &mut Parser, space: &str) -> Result<IsaItem, IsaError> {
    let name = parser.expect_identifier("field tag")?;
    let range = if parser.check(TokenKind::Range)? {
        let token = parser.consume()?;
        Some(parse_index_range(&token)?)
    } else {
        None
    };

    let mut offset = None;
    let mut size = None;
    let mut reset = None;
    let mut description = None;
    let mut redirect = None;
    let mut subfields: Vec<SubFieldDecl> = Vec::new();
    let mut seen_subfields = false;

    while !parser.check(TokenKind::EOF)? && !parser.check(TokenKind::Colon)? {
        let attr_name = parser.expect_identifier("field attribute name")?;
        parser.expect(TokenKind::Equals, "'=' after field attribute name")?;
        match attr_name.to_ascii_lowercase().as_str() {
            "offset" => {
                ensure_unique(attr_name.as_str(), &offset)?;
                offset = Some(parse_number(parser, "offset")?);
            }
            "size" => {
                ensure_unique(attr_name.as_str(), &size)?;
                let value = parse_number(parser, "size")?;
                if value == 0 || value > 512 {
                    return Err(IsaError::Parser(format!(
                        "field size must be between 1 and 512 bits, got {value}"
                    )));
                }
                size = Some(value as u32);
            }
            "reset" => {
                ensure_unique(attr_name.as_str(), &reset)?;
                reset = Some(parse_number(parser, "reset" )?);
            }
            "descr" => {
                ensure_unique(attr_name.as_str(), &description)?;
                let value = parser.expect(TokenKind::String, "string literal for descr")?;
                description = Some(value.lexeme);
            }
            "redirect" => {
                ensure_unique(attr_name.as_str(), &redirect)?;
                redirect = Some(parse_context_reference(parser)?);
            }
            "subfields" => {
                if seen_subfields {
                    return Err(IsaError::Parser(format!(
                        "duplicate subfields block for field {name}"
                    )));
                }
                let block = parse_subfields_block(parser)?;
                subfields = block;
                seen_subfields = true;
            }
            other => {
                return Err(IsaError::Parser(format!(
                    "unknown field attribute '{other}'"
                )));
            }
        }
    }

    Ok(IsaItem::SpaceMember(SpaceMemberDecl {
        space: space.to_string(),
        member: SpaceMember::Field(FieldDecl {
            space: space.to_string(),
            name,
            range,
            offset,
            size,
            reset,
            description,
            redirect,
            subfields,
        }),
    }))
}

fn ensure_unique<T>(name: &str, slot: &Option<T>) -> Result<(), IsaError> {
    if slot.is_some() {
        Err(IsaError::Parser(format!(
            "field attribute '{name}' specified multiple times"
        )))
    } else {
        Ok(())
    }
}

fn parse_number(parser: &mut Parser, context: &str) -> Result<u64, IsaError> {
    let token = parser.expect(TokenKind::Number, &format!("numeric literal for {context}"))?;
    parse_numeric_literal(&token.lexeme).map_err(|err| {
        IsaError::Parser(format!("invalid numeric literal '{}' for {context}: {err}", token.lexeme))
    })
}

fn parse_index_range(token: &Token) -> Result<FieldIndexRange, IsaError> {
    let text = token.lexeme.trim();
    if !text.starts_with('[') || !text.ends_with(']') {
        return Err(IsaError::Parser(format!(
            "invalid index range '{}': expected [start..end]",
            text
        )));
    }
    let inner = &text[1..text.len() - 1];
    let normalized: String = inner.chars().filter(|ch| !ch.is_whitespace()).collect();
    let parts: Vec<&str> = normalized.split("..").collect();
    if parts.len() != 2 {
        return Err(IsaError::Parser(format!(
            "invalid index range '{}': missing '..'",
            text
        )));
    }
    let start = parse_numeric_literal(parts[0]).map_err(|err| {
        IsaError::Parser(format!("invalid start index '{}': {err}", parts[0]))
    })?;
    let end = parse_numeric_literal(parts[1]).map_err(|err| {
        IsaError::Parser(format!("invalid end index '{}': {err}", parts[1]))
    })?;
    if end < start {
        return Err(IsaError::Parser(format!(
            "index range end must be >= start ({}..{})",
            start, end
        )));
    }
    if end - start + 1 > 65_535 {
        return Err(IsaError::Parser(
            "index range must contain at most 65535 entries".into(),
        ));
    }
    let start_u32 = u32::try_from(start).map_err(|_| {
        IsaError::Parser(format!("start index '{start}' does not fit in u32"))
    })?;
    let end_u32 = u32::try_from(end).map_err(|_| {
        IsaError::Parser(format!("end index '{end}' does not fit in u32"))
    })?;
    Ok(FieldIndexRange {
        start: start_u32,
        end: end_u32,
    })
}

fn parse_context_reference(parser: &mut Parser) -> Result<ContextReference, IsaError> {
    let mut segments = Vec::new();
    segments.push(parser.expect_identifier("context reference segment")?);
    while parser.check(TokenKind::DoubleColon)? {
        parser.consume()?;
        segments.push(parser.expect_identifier("context reference segment")?);
    }
    Ok(ContextReference { segments })
}

fn parse_subfields_block(parser: &mut Parser) -> Result<Vec<SubFieldDecl>, IsaError> {
    parser.expect(TokenKind::LBrace, "'{' to start subfields block")?;
    let mut entries = Vec::new();
    loop {
        if parser.check(TokenKind::EOF)? {
            return Err(IsaError::Parser(
                "unterminated subfields block; missing closing '}'".into(),
            ));
        }
        if parser.check(TokenKind::RBrace)? {
            parser.consume()?;
            break;
        }
        let name = parser.expect_identifier("subfield name")?;
        let bit_spec = parser.expect(TokenKind::BitExpr, "bit specification '@(...)'")?;
        let mut operations = Vec::new();
        let mut description = None;

        loop {
            if parser.check(TokenKind::Identifier)? {
                let peek = parser.peek()?;
                match peek.lexeme.as_str() {
                    "op" => {
                        parser.consume()?;
                        parser.expect(TokenKind::Equals, "'=' after op attribute")?;
                        if !operations.is_empty() {
                            return Err(IsaError::Parser(format!(
                                "subfield {name} op attribute specified multiple times"
                            )));
                        }
                        operations = parse_subfield_ops(parser)?;
                    }
                    "descr" => {
                        parser.consume()?;
                        parser.expect(TokenKind::Equals, "'=' after descr attribute")?;
                        if description.is_some() {
                            return Err(IsaError::Parser(format!(
                                "subfield {name} descr attribute specified multiple times"
                            )));
                        }
                        let value =
                            parser.expect(TokenKind::String, "string literal for descr attribute")?;
                        description = Some(value.lexeme);
                    }
                    _ => break,
                }
            } else {
                break;
            }
        }

        entries.push(SubFieldDecl {
            name,
            bit_spec: bit_spec.lexeme,
            operations,
            description,
        });
    }
    Ok(entries)
}

fn parse_subfield_ops(parser: &mut Parser) -> Result<Vec<SubFieldOp>, IsaError> {
    let mut ops = Vec::new();
    loop {
        let token = parser.expect(TokenKind::Identifier, "subfield op type")?;
        let mut parts = token.lexeme.splitn(2, '.');
        let kind = parts.next().unwrap().to_string();
        let subtype = parts.next().map(|value| value.to_string());
        ops.push(SubFieldOp { kind, subtype });
        if parser.check(TokenKind::Pipe)? {
            parser.consume()?;
        } else {
            break;
        }
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::soc::isa::ast::{IsaItem, SpaceMember};
    use crate::soc::isa::error::IsaError;

    use super::super::parse_str;

    fn parse(source: &str) -> crate::soc::isa::ast::IsaDocument {
        parse_str(PathBuf::from("test.isa"), source).expect("parse")
    }

    #[test]
    fn parses_register_form_with_subfields() {
        let doc = parse(
            ":space reg addr=32 word=64 type=register\n:reg GPR[0..31] offset=0x100 size=64 descr=\"regs\" subfields={\n    VALUE @(0..63) op=target\n}",
        );
        assert_eq!(doc.items.len(), 2, "expected space and member");
        let field = match &doc.items[1] {
            IsaItem::SpaceMember(member) => match &member.member {
                SpaceMember::Field(field) => field,
                other => panic!("unexpected member: {other:?}"),
            },
            other => panic!("unexpected item: {other:?}"),
        };
        assert_eq!(field.space, "reg");
        assert_eq!(field.name, "GPR");
        let range = field.range.as_ref().expect("range parsed");
        assert_eq!(range.start, 0);
        assert_eq!(range.end, 31);
        assert_eq!(field.offset, Some(0x100));
        assert_eq!(field.size, Some(64));
        assert_eq!(field.description.as_deref(), Some("regs"));
        assert_eq!(field.subfields.len(), 1);
        let sub = &field.subfields[0];
        assert_eq!(sub.name, "VALUE");
        assert_eq!(sub.operations.len(), 1);
        assert_eq!(sub.operations[0].kind, "target");
    }

    #[test]
    fn rejects_logic_space_contexts() {
        let err = parse_str(
            PathBuf::from("test.isa"),
            ":space logic addr=32 word=32 type=logic\n:logic FORM subfields={\n    OPCD @(0..5)\n}",
        )
        .unwrap_err();
        assert!(matches!(err, IsaError::Parser(msg) if msg.contains("logic space")));
    }

    #[test]
    fn errors_on_duplicate_subfields_block() {
        let err = parse_str(
            PathBuf::from("test.isa"),
            ":space reg addr=32 word=32 type=register\n:reg R0 subfields={} subfields={}",
        )
        .unwrap_err();
        assert!(matches!(err, IsaError::Parser(msg) if msg.contains("duplicate subfields")));
    }
}
