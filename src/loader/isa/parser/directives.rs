use crate::soc::isa::ast::IsaItem;
use crate::soc::isa::error::IsaError;

use super::{
    parameters::parse_parameter_decl,
    space::parse_space_directive,
    space_context::parse_space_context_directive,
    Parser,
    TokenKind,
};

impl<'src> Parser<'src> {
    pub(super) fn parse_directive(&mut self) -> Result<IsaItem, IsaError> {
        self.expect(TokenKind::Colon, "directive introducer ':'")?;
        let name = self.expect_identifier("directive name")?;
        let item = match name.as_str() {
            "fileset" => self.parse_fileset_directive(),
            "param" => self.parse_param_directive(),
            "space" => parse_space_directive(self),
            _ => {
                if self.is_known_space(&name) {
                    self.parse_space_context(&name)
                } else {
                    Err(IsaError::Parser(format!("unsupported directive :{name}")))
                }
            }
        }?;
        self.ensure_directive_boundary(&name)?;
        Ok(item)
    }

    fn parse_fileset_directive(&mut self) -> Result<IsaItem, IsaError> {
        let decl = parse_parameter_decl(self, "fileset parameter name")?;
        Ok(IsaItem::Parameter(decl))
    }

    fn parse_param_directive(&mut self) -> Result<IsaItem, IsaError> {
        let decl = parse_parameter_decl(self, "parameter name")?;
        Ok(IsaItem::Parameter(decl))
    }

    fn parse_space_context(&mut self, name: &str) -> Result<IsaItem, IsaError> {
        let kind = self.space_kind(name).ok_or_else(|| {
            IsaError::Parser(format!("space :{name} context referenced before definition"))
        })?;
        parse_space_context_directive(self, name, kind)
    }

    fn ensure_directive_boundary(&mut self, directive: &str) -> Result<(), IsaError> {
        if self.check(TokenKind::EOF)? || self.check(TokenKind::Colon)? {
            return Ok(());
        }

        let mut extras = Vec::new();
        while !self.check(TokenKind::EOF)? {
            if self.check(TokenKind::Colon)? {
                break;
            }
            extras.push(self.consume()?);
        }

        let snippet = extras
            .into_iter()
            .map(|token| token.lexeme)
            .filter(|lex| !lex.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let message = if snippet.is_empty() {
            format!("unexpected trailing tokens after :{directive}")
        } else {
            format!("unexpected trailing tokens after :{directive}: {snippet}")
        };
        Err(IsaError::Parser(message))
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::soc::isa::ast::{IsaItem, ParameterDecl, ParameterValue};
    use crate::soc::isa::error::IsaError;

    use super::super::parse_str;

    fn parse(source: &str) -> crate::soc::isa::ast::IsaDocument {
        parse_str(PathBuf::from("test.isa"), source).expect("parse")
    }

    #[test]
    fn parses_fileset_bitdir_enum() {
        let doc = parse(":fileset BITDIR = LSB0");
        assert_eq!(doc.items.len(), 1, "one parameter expected");
        match &doc.items[0] {
            IsaItem::Parameter(ParameterDecl { name, value }) => {
                assert_eq!(name, "BITDIR");
                assert!(matches!(value, ParameterValue::Word(val) if val == "LSB0"));
            }
            other => panic!("unexpected item: {:?}", other),
        }
    }

    #[test]
    fn parses_fileset_string_literal() {
        let doc = parse(":fileset TAG = \"core\"");
        match &doc.items[0] {
            IsaItem::Parameter(ParameterDecl { name, value }) => {
                assert_eq!(name, "TAG");
                assert!(matches!(value, ParameterValue::Word(val) if val == "core"));
            }
            _ => panic!("expected parameter"),
        }
    }

    #[test]
    fn parses_fileset_number_literal() {
        let doc = parse(":fileset CACHE_SIZE = 0x10");
        match &doc.items[0] {
            IsaItem::Parameter(ParameterDecl { name, value }) => {
                assert_eq!(name, "CACHE_SIZE");
                assert!(matches!(value, ParameterValue::Number(16)));
            }
            _ => panic!("expected parameter"),
        }
    }

    #[test]
    fn parses_param_identifier_literal() {
        let doc = parse(":param ENDIAN = big");
        match &doc.items[0] {
            IsaItem::Parameter(ParameterDecl { name, value }) => {
                assert_eq!(name, "ENDIAN");
                assert!(matches!(value, ParameterValue::Word(val) if val == "big"));
            }
            _ => panic!("expected parameter"),
        }
    }

    #[test]
    fn parses_param_numeric_literal() {
        let doc = parse(":param REGISTER_SIZE = 64");
        match &doc.items[0] {
            IsaItem::Parameter(ParameterDecl { name, value }) => {
                assert_eq!(name, "REGISTER_SIZE");
                assert!(matches!(value, ParameterValue::Number(64)));
            }
            _ => panic!("expected parameter"),
        }
    }

    #[test]
    fn rejects_unknown_directive() {
        let err = parse_str(PathBuf::from("test.isa"), ":unknown foo").unwrap_err();
        assert!(matches!(err, IsaError::Parser(msg) if msg.contains("unsupported directive")));
    }

    #[test]
    fn parses_basic_space_context() {
        let doc = parse(
            ":space reg addr=32 word=64 type=register\n:reg GPR size=64 subfields={\n    VALUE @(0..63)\n}",
        );
        assert_eq!(doc.items.len(), 2, "space + member item expected");
        assert!(matches!(doc.items[0], IsaItem::Space(_)));
        match &doc.items[1] {
            IsaItem::SpaceMember(member) => {
                assert_eq!(member.space, "reg");
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn errors_on_trailing_tokens_after_directive() {
        let err = parse_str(PathBuf::from("test.isa"), ":param ENDIAN=big extra")
            .unwrap_err();
        assert!(matches!(err, IsaError::Parser(msg) if msg.contains("unexpected trailing tokens")));
    }
}
