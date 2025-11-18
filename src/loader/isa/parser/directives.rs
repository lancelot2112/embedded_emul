use crate::soc::isa::ast::IsaItem;
use crate::soc::isa::error::IsaError;

use super::{parameters::parse_parameter_decl, Parser, TokenKind};

impl<'src> Parser<'src> {
    pub(super) fn parse_directive(&mut self) -> Result<IsaItem, IsaError> {
        self.expect(TokenKind::Colon, "directive introducer ':'")?;
        let name = self.expect_identifier("directive name")?;
        match name.as_str() {
            "fileset" => self.parse_fileset_directive(),
            "param" => self.parse_param_directive(),
            _ => Err(IsaError::Parser(format!("unsupported directive :{name}"))),
        }
    }

    fn parse_fileset_directive(&mut self) -> Result<IsaItem, IsaError> {
        let decl = parse_parameter_decl(self, "fileset parameter name")?;
        Ok(IsaItem::Parameter(decl))
    }

    fn parse_param_directive(&mut self) -> Result<IsaItem, IsaError> {
        let decl = parse_parameter_decl(self, "parameter name")?;
        Ok(IsaItem::Parameter(decl))
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
}
