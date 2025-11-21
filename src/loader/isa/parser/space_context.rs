use crate::soc::isa::ast::{
    ContextReference, FieldDecl, FieldIndexRange, FormDecl, InstructionDecl, IsaItem, MaskField,
    MaskSelector, MaskSpec, SpaceKind, SpaceMember, SpaceMemberDecl, SubFieldDecl, SubFieldOp,
};
use crate::soc::isa::error::IsaError;
use crate::soc::isa::semantics::SemanticBlock;
use crate::soc::prog::types::parse_u64_literal;

use super::{Parser, Token, TokenKind, spans::span_from_tokens};

pub(super) fn parse_space_context_directive(
    parser: &mut Parser,
    space: &str,
    kind: SpaceKind,
) -> Result<IsaItem, IsaError> {
    match kind {
        SpaceKind::Logic => parse_logic_context_directive(parser, space),
        _ => parse_register_form(parser, space),
    }
}

fn parse_logic_context_directive(parser: &mut Parser, space: &str) -> Result<IsaItem, IsaError> {
    let mut qualifier = None;
    if parser.check(TokenKind::DoubleColon)? {
        parser.consume()?;
        qualifier = Some(parser.expect_identifier("logic context qualifier")?);
    }

    let name_token = parser.expect_identifier_token("form or instruction name")?;
    let name = name_token.lexeme.clone();
    let operands = if parser.check(TokenKind::LParen)? {
        parse_operand_list(parser)?
    } else {
        Vec::new()
    };

    let mut description: Option<String> = None;
    let mut mask: Option<MaskSpec> = None;
    let mut subfields: Option<Vec<SubFieldDecl>> = None;
    let mut semantics: Option<SemanticBlock> = None;
    let mut display: Option<String> = None;
    let mut operator: Option<String> = None;

    while !parser.check(TokenKind::EOF)? && !parser.check(TokenKind::Colon)? {
        let attr_token = parser.expect_identifier_token("logic attribute name")?;
        let attr_name = attr_token.lexeme.to_ascii_lowercase();
        parser.expect(TokenKind::Equals, "'=' after logic attribute name")?;
        match attr_name.as_str() {
            "descr" => {
                if description.is_some() {
                    return Err(IsaError::Parser(format!(
                        "duplicate descr attribute for '{name}'"
                    )));
                }
                let value = parser.expect(TokenKind::String, "string literal for descr")?;
                description = Some(value.lexeme);
            }
            "mask" => {
                if mask.is_some() {
                    return Err(IsaError::Parser(format!(
                        "duplicate mask attribute for instruction '{name}'"
                    )));
                }
                mask = Some(parse_mask_block(parser)?);
            }
            "subfields" => {
                if subfields.is_some() {
                    return Err(IsaError::Parser(format!(
                        "duplicate subfields block for '{name}'"
                    )));
                }
                subfields = Some(parse_subfields_block(parser)?);
            }
            "semantics" => {
                if semantics.is_some() {
                    return Err(IsaError::Parser(format!(
                        "duplicate semantics block for instruction '{name}'"
                    )));
                }
                let block = parser.parse_semantic_block("semantics block")?;
                semantics = Some(block);
            }
            "disp" => {
                if display.is_some() {
                    return Err(IsaError::Parser(format!(
                        "duplicate disp attribute for '{name}'"
                    )));
                }
                let value = parser.expect(TokenKind::String, "string literal for disp")?;
                display = Some(value.lexeme);
            }
            "op" => {
                if operator.is_some() {
                    return Err(IsaError::Parser(format!(
                        "duplicate op attribute for '{name}'"
                    )));
                }
                let value = parser.expect(TokenKind::String, "string literal for op")?;
                operator = Some(value.lexeme);
            }
            other => {
                return Err(IsaError::Parser(format!(
                    "unknown logic attribute '{other}'"
                )));
            }
        }
    }

    let end_token = parser
        .last_consumed_token()
        .cloned()
        .unwrap_or_else(|| name_token.clone());
    let span = span_from_tokens(parser.file_path(), &name_token, &end_token);

    let has_instruction_attrs = mask.is_some() || semantics.is_some() || !operands.is_empty();
    if qualifier.is_none() && subfields.is_none() && !has_instruction_attrs {
        return Err(IsaError::Parser(format!(
            "form '{name}' must declare a subfields block"
        )));
    }

    if let Some(subfields) = subfields {
        if !operands.is_empty() {
            return Err(IsaError::Parser(format!(
                "forms cannot declare operand lists (found before '{name}')"
            )));
        }
        if mask.is_some() {
            return Err(IsaError::Parser(format!(
                "forms cannot declare mask attributes ('{name}')"
            )));
        }
        if semantics.is_some() {
            return Err(IsaError::Parser(format!(
                "forms cannot declare semantics blocks ('{name}')"
            )));
        }
        if operator.is_some() {
            return Err(IsaError::Parser(format!(
                "forms cannot declare an op attribute ('{name}')"
            )));
        }
        let form = FormDecl {
            space: space.to_string(),
            name,
            parent: qualifier,
            description: description.clone(),
            display: display.clone(),
            subfields,
            span,
        };
        return Ok(IsaItem::SpaceMember(SpaceMemberDecl {
            space: space.to_string(),
            member: SpaceMember::Form(form),
        }));
    }

    if qualifier.is_none() {
        return Err(IsaError::Parser(format!(
            "logic instruction '{name}' must specify a form using '::FormName'"
        )));
    }

    let instruction = InstructionDecl {
        space: space.to_string(),
        form: qualifier,
        name,
        description,
        operands,
        mask,
        encoding: None,
        semantics,
        display,
        operator,
        span,
    };
    Ok(IsaItem::SpaceMember(SpaceMemberDecl {
        space: space.to_string(),
        member: SpaceMember::Instruction(instruction),
    }))
}

fn parse_operand_list(parser: &mut Parser) -> Result<Vec<String>, IsaError> {
    parser.expect(TokenKind::LParen, "'(' to start operand list")?;
    let mut operands = Vec::new();
    loop {
        if parser.check(TokenKind::RParen)? {
            parser.consume()?;
            break;
        }
        let name = parser.expect_identifier("operand name")?;
        operands.push(name);
        if parser.check(TokenKind::Comma)? {
            parser.consume()?;
        } else {
            parser.expect(TokenKind::RParen, "')' to close operand list")?;
            break;
        }
    }
    Ok(operands)
}

fn parse_mask_block(parser: &mut Parser) -> Result<MaskSpec, IsaError> {
    parser.expect(TokenKind::LBrace, "'{' to start mask block")?;
    let mut fields = Vec::new();
    loop {
        if parser.check(TokenKind::RBrace)? {
            parser.consume()?;
            break;
        }
        if parser.check(TokenKind::Comma)? {
            parser.consume()?;
            continue;
        }
        let selector = parse_mask_selector(parser)?;
        parser.expect(TokenKind::Equals, "'=' after mask field name")?;
        let value = parser.expect(TokenKind::Number, "numeric literal for mask value")?;
        let parsed = parse_u64_literal(&value.lexeme).map_err(|err| {
            IsaError::Parser(format!("invalid mask literal '{}': {err}", value.lexeme))
        })?;
        fields.push(MaskField {
            selector,
            value: parsed,
        });
        if parser.check(TokenKind::Comma)? {
            parser.consume()?;
        }
    }
    if fields.is_empty() {
        return Err(IsaError::Parser(
            "mask block must contain at least one field".into(),
        ));
    }
    Ok(MaskSpec { fields })
}

fn parse_mask_selector(parser: &mut Parser) -> Result<MaskSelector, IsaError> {
    if parser.check(TokenKind::BitExpr)? {
        let token = parser.consume()?;
        Ok(MaskSelector::BitExpr(token.lexeme))
    } else {
        let token = parser.expect_identifier_token("mask field name")?;
        Ok(MaskSelector::Field(token.lexeme))
    }
}

fn parse_register_form(parser: &mut Parser, space: &str) -> Result<IsaItem, IsaError> {
    let name_token = parser.expect_identifier_token("field tag")?;
    let name = name_token.lexeme.clone();
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
    let mut display = None;
    let mut subfields: Vec<SubFieldDecl> = Vec::new();
    let mut seen_subfields = false;

    while !parser.check(TokenKind::EOF)? && !parser.check(TokenKind::Colon)? {
        let attr_name = parser.expect_identifier("field attribute name")?;
        parser.expect(TokenKind::Equals, "'=' after field attribute name")?;
        match attr_name.to_ascii_lowercase().as_str() {
            "offset" => {
                ensure_redirect_compatible("offset", redirect.is_some())?;
                ensure_unique(attr_name.as_str(), &offset)?;
                offset = Some(parse_number(parser, "offset")?);
            }
            "size" => {
                ensure_redirect_compatible("size", redirect.is_some())?;
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
                ensure_redirect_compatible("reset", redirect.is_some())?;
                ensure_unique(attr_name.as_str(), &reset)?;
                reset = Some(parse_number(parser, "reset")?);
            }
            "descr" => {
                ensure_unique(attr_name.as_str(), &description)?;
                let value = parser.expect(TokenKind::String, "string literal for descr")?;
                description = Some(value.lexeme);
            }
            "redirect" => {
                ensure_unique(attr_name.as_str(), &redirect)?;
                if offset.is_some() {
                    return Err(IsaError::Parser(
                        "redirect fields cannot specify an offset".into(),
                    ));
                }
                if size.is_some() {
                    return Err(IsaError::Parser(
                        "redirect fields cannot specify a size".into(),
                    ));
                }
                if reset.is_some() {
                    return Err(IsaError::Parser(
                        "redirect fields cannot specify a reset value".into(),
                    ));
                }
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
            "disp" => {
                ensure_unique(attr_name.as_str(), &display)?;
                let value = parser.expect(TokenKind::String, "string literal for disp")?;
                display = Some(value.lexeme);
            }
            other => {
                return Err(IsaError::Parser(format!(
                    "unknown field attribute '{other}'"
                )));
            }
        }
    }

    let end_token = parser
        .last_consumed_token()
        .cloned()
        .unwrap_or_else(|| name_token.clone());
    let span = span_from_tokens(parser.file_path(), &name_token, &end_token);

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
            span,
            display,
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

fn ensure_redirect_compatible(name: &str, has_redirect: bool) -> Result<(), IsaError> {
    if has_redirect {
        Err(IsaError::Parser(format!(
            "redirect fields cannot specify a {name} attribute"
        )))
    } else {
        Ok(())
    }
}

fn parse_number(parser: &mut Parser, context: &str) -> Result<u64, IsaError> {
    let token = parser.expect(TokenKind::Number, &format!("numeric literal for {context}"))?;
    parse_u64_literal(&token.lexeme).map_err(|err| {
        IsaError::Parser(format!(
            "invalid numeric literal '{}' for {context}: {err}",
            token.lexeme
        ))
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
    let start = parse_u64_literal(parts[0])
        .map_err(|err| IsaError::Parser(format!("invalid start index '{}': {err}", parts[0])))?;
    let end = parse_u64_literal(parts[1])
        .map_err(|err| IsaError::Parser(format!("invalid end index '{}': {err}", parts[1])))?;
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
    let start_u32 = u32::try_from(start)
        .map_err(|_| IsaError::Parser(format!("start index '{start}' does not fit in u32")))?;
    let end_u32 = u32::try_from(end)
        .map_err(|_| IsaError::Parser(format!("end index '{end}' does not fit in u32")))?;
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
                        let value = parser
                            .expect(TokenKind::String, "string literal for descr attribute")?;
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
        let mut op_name = token.lexeme;
        while parser.check(TokenKind::DoubleColon)? {
            parser.consume()?;
            let segment = parser.expect_identifier("context reference segment")?;
            op_name.push_str("::");
            op_name.push_str(&segment);
        }
        let mut parts = op_name.splitn(2, '.');
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

    use crate::soc::isa::ast::{IsaItem, MaskSelector, SpaceMember};
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
        let doc = parse(
            ":space logic addr=32 word=32 type=logic\n:logic FORM subfields={\n    OPCD @(0..5)\n}\n:logic::FORM add mask={OPCD=31}",
        );
        assert_eq!(doc.items.len(), 3);
        match &doc.items[1] {
            IsaItem::SpaceMember(member) => match &member.member {
                SpaceMember::Form(form) => {
                    assert_eq!(form.name, "FORM");
                    assert!(form.parent.is_none());
                }
                other => panic!("expected form, got {other:?}"),
            },
            other => panic!("unexpected item: {other:?}"),
        }
        match &doc.items[2] {
            IsaItem::SpaceMember(member) => match &member.member {
                SpaceMember::Instruction(instr) => {
                    assert_eq!(instr.name, "add");
                    assert_eq!(instr.form.as_deref(), Some("FORM"));
                    let mask = instr.mask.as_ref().expect("mask parsed");
                    assert_eq!(mask.fields.len(), 1);
                }
                other => panic!("unexpected member: {other:?}"),
            },
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn parses_logic_form_inheritance_and_instruction_operands() {
        let doc = parse(
            ":space logic addr=32 word=32 type=logic\n:logic BASE subfields={\n    OPCD @(0..5) op=func\n}\n:logic::BASE EXT subfields={\n    RT @(6..10) op=target\n}\n:logic::EXT add (RT) mask={OPCD=31, @(0..5)=0b11111} descr=\"Add\" semantics={\n    result = RT\n}\n",
        );
        assert_eq!(doc.items.len(), 4);
        let base = match &doc.items[1] {
            IsaItem::SpaceMember(member) => match &member.member {
                SpaceMember::Form(form) => form,
                other => panic!("expected form, got {other:?}"),
            },
            other => panic!("unexpected item: {other:?}"),
        };
        assert_eq!(base.name, "BASE");
        assert!(base.parent.is_none());
        assert_eq!(base.subfields.len(), 1);

        let ext = match &doc.items[2] {
            IsaItem::SpaceMember(member) => match &member.member {
                SpaceMember::Form(form) => form,
                other => panic!("expected form, got {other:?}"),
            },
            other => panic!("unexpected item: {other:?}"),
        };
        assert_eq!(ext.name, "EXT");
        assert_eq!(ext.parent.as_deref(), Some("BASE"));
        assert_eq!(ext.subfields.len(), 1);
        assert_eq!(ext.subfields[0].name, "RT");

        let instr = match &doc.items[3] {
            IsaItem::SpaceMember(member) => match &member.member {
                SpaceMember::Instruction(instr) => instr,
                other => panic!("expected instruction, got {other:?}"),
            },
            other => panic!("unexpected item: {other:?}"),
        };
        assert_eq!(instr.name, "add");
        assert_eq!(instr.form.as_deref(), Some("EXT"));
        assert_eq!(instr.operands, vec!["RT".to_string()]);
        let semantics = instr.semantics.as_ref().expect("semantics parsed");
        assert!(semantics.source.contains("result = RT"));
        let mask = instr.mask.as_ref().expect("mask parsed");
        assert_eq!(mask.fields.len(), 2);
        match &mask.fields[0].selector {
            MaskSelector::Field(name) => assert_eq!(name, "OPCD"),
            other => panic!("unexpected selector: {other:?}"),
        }
        assert!(matches!(mask.fields[1].selector, MaskSelector::BitExpr(_)));
    }

    #[test]
    fn logic_instruction_requires_form_qualifier() {
        let err = parse_str(
            PathBuf::from("test.isa"),
            ":space logic addr=32 word=32 type=logic\n:logic add (RT) mask={OPCD=31}",
        )
        .unwrap_err();
        expect_parser_diag(err, "must specify a form");
    }

    #[test]
    fn logic_form_rejects_operand_list() {
        let err = parse_str(
            PathBuf::from("test.isa"),
            ":space logic addr=32 word=32 type=logic\n:logic FORM (RT) subfields={\n    OPCD @(0..5)\n}",
        )
        .unwrap_err();
        expect_parser_diag(err, "forms cannot declare operand lists");
    }

    #[test]
    fn logic_form_rejects_mask_attribute() {
        let err = parse_str(
            PathBuf::from("test.isa"),
            ":space logic addr=32 word=32 type=logic\n:logic FORM subfields={\n    OPCD @(0..5)\n} mask={OPCD=31}",
        )
        .unwrap_err();
        expect_parser_diag(err, "forms cannot declare mask attributes");
    }

    #[test]
    fn errors_on_duplicate_subfields_block() {
        let err = parse_str(
            PathBuf::from("test.isa"),
            ":space reg addr=32 word=32 type=register\n:reg R0 subfields={} subfields={}",
        )
        .unwrap_err();
        expect_parser_diag(err, "duplicate subfields");
    }

    #[test]
    fn parses_redirect_field_without_extra_attributes() {
        let doc = parse(
            ":space reg addr=32 word=64 type=register\n:reg SP redirect=GPR1 descr=\"Stack Pointer\"",
        );
        let field = match &doc.items[1] {
            IsaItem::SpaceMember(member) => match &member.member {
                SpaceMember::Field(field) => field,
                other => panic!("unexpected member: {other:?}"),
            },
            other => panic!("unexpected item: {other:?}"),
        };
        assert!(field.redirect.is_some());
        assert!(field.offset.is_none());
        assert!(field.size.is_none());
        assert!(field.reset.is_none());
    }

    #[test]
    fn rejects_redirect_with_offset() {
        let err = parse_str(
            PathBuf::from("test.isa"),
            ":space reg addr=32 word=64 type=register\n:reg SP offset=0x0 redirect=GPR1",
        )
        .unwrap_err();
        expect_parser_diag(err, "cannot specify an offset");
    }

    #[test]
    fn rejects_redirect_followed_by_size() {
        let err = parse_str(
            PathBuf::from("test.isa"),
            ":space reg addr=32 word=64 type=register\n:reg SP redirect=GPR1 size=64",
        )
        .unwrap_err();
        expect_parser_diag(err, "cannot specify a size");
    }
}
