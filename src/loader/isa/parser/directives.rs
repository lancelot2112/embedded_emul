use std::path::PathBuf;

use super::spans::span_from_tokens;
use super::{
    Parser, TokenKind, parameters::parse_parameter_decl, space::parse_space_directive,
    space_context::parse_space_context_directive,
};
use crate::soc::isa::ast::{IncludeDecl, IsaItem, MacroDecl, ParameterValue};
use crate::soc::isa::error::IsaError;
use crate::soc::prog::types::bitfield::BitOrder;

impl<'src> Parser<'src> {
    pub(super) fn parse_directive(&mut self) -> Result<Option<IsaItem>, IsaError> {
        self.expect(TokenKind::Colon, "directive introducer ':'")?;
        let name = self.expect_identifier("directive name")?;
        let item = match name.as_str() {
            "fileset" => self.parse_fileset_directive(),
            "param" => self.parse_param_directive().map(Some),
            "space" => parse_space_directive(self).map(Some),
            "include" => self.parse_include_directive().map(Some),
            "macro" => self.parse_macro_directive().map(Some),
            "extends" => {
                self.parse_extends_directive()?;
                Ok(None)
            }
            _ => {
                if self.is_known_space(&name) {
                    self.parse_space_context(&name).map(Some)
                } else {
                    Err(IsaError::Parser(format!("unsupported directive :{name}")))
                }
            }
        }?;
        self.ensure_directive_boundary(&name)?;
        Ok(item)
    }

    fn parse_fileset_directive(&mut self) -> Result<Option<IsaItem>, IsaError> {
        let decl = parse_parameter_decl(self, "fileset parameter name")?;
        if decl.name.eq_ignore_ascii_case("BITORDER") {
            let ParameterValue::Word(value) = &decl.value else {
                return Err(IsaError::Parser(
                    ":fileset BITORDER must use a word literal (MSB0/LSB0)".into(),
                ));
            };
            let upper = value.to_ascii_uppercase();
            let order = match upper.as_str() {
                "MSB0" => BitOrder::Msb0,
                "LSB0" => BitOrder::Lsb0,
                other => {
                    return Err(IsaError::Parser(format!(
                        "unknown BITORDER value '{other}': expected MSB0 or LSB0"
                    )))
                }
            };
            self.set_bit_order(order);
            Ok(None)
        } else {
            Ok(Some(IsaItem::Parameter(decl)))
        }
    }

    fn parse_param_directive(&mut self) -> Result<IsaItem, IsaError> {
        let decl = parse_parameter_decl(self, "parameter name")?;
        Ok(IsaItem::Parameter(decl))
    }

    fn parse_include_directive(&mut self) -> Result<IsaItem, IsaError> {
        if !self.allows_include() {
            return Err(IsaError::Parser(
                ":include directive is only allowed inside .coredef files".into(),
            ));
        }
        let path = self.expect(TokenKind::String, "string literal with include path")?;
        Ok(IsaItem::Include(IncludeDecl {
            path: PathBuf::from(path.lexeme),
            optional: false,
        }))
    }

    fn parse_macro_directive(&mut self) -> Result<IsaItem, IsaError> {
        let name_token = self.expect_identifier_token("macro name")?;
        let params = self.parse_macro_parameters()?;
        let semantics = self.parse_semantic_block("macro body")?;
        let end_token = self
            .last_consumed_token()
            .cloned()
            .unwrap_or_else(|| name_token.clone());
        let span = span_from_tokens(self.file_path(), &name_token, &end_token);
        Ok(IsaItem::Macro(MacroDecl {
            name: name_token.lexeme,
            parameters: params,
            semantics,
            span,
        }))
    }

    fn parse_extends_directive(&mut self) -> Result<(), IsaError> {
        if !self.allows_extends() {
            return Err(IsaError::Parser(
                ":extends directive is only allowed inside .isaext files".into(),
            ));
        }
        let path = self.expect(TokenKind::String, "string literal with extends path")?;
        self.record_extend(PathBuf::from(path.lexeme));
        Ok(())
    }

    fn parse_macro_parameters(&mut self) -> Result<Vec<String>, IsaError> {
        self.expect(TokenKind::LParen, "'(' to start macro parameter list")?;
        let mut params = Vec::new();
        if self.check(TokenKind::RParen)? {
            self.consume()?;
            return Ok(params);
        }
        loop {
            let param = self.expect_identifier("macro parameter")?;
            params.push(param);
            if self.check(TokenKind::Comma)? {
                self.consume()?;
                continue;
            }
            self.expect(TokenKind::RParen, "')' to close macro parameter list")?;
            break;
        }
        Ok(params)
    }

    fn parse_space_context(&mut self, name: &str) -> Result<IsaItem, IsaError> {
        let kind = self.space_kind(name).ok_or_else(|| {
            IsaError::Parser(format!(
                "space :{name} context referenced before definition"
            ))
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
    use crate::soc::isa::diagnostic::DiagnosticPhase;
    use crate::soc::isa::error::IsaError;

    use super::super::parse_str;

    fn parse(source: &str) -> crate::soc::isa::ast::IsaSpecification {
        parse_str(PathBuf::from("test.isa"), source).expect("parse")
    }

    fn parse_core(source: &str) -> crate::soc::isa::ast::IsaSpecification {
        parse_str(PathBuf::from("test.coredef"), source).expect("parse")
    }

    fn parse_ext(source: &str) -> crate::soc::isa::ast::IsaSpecification {
        parse_str(PathBuf::from("test.isaext"), source).expect("parse")
    }

    fn expect_parser_diag(err: IsaError, needle: &str) {
        match err {
            IsaError::Diagnostics {
                phase: DiagnosticPhase::Parser,
                diagnostics,
            } => {
                assert!(
                    diagnostics.iter().any(|diag| diag.message.contains(needle)),
                    "diagnostics missing needle '{needle}': {diagnostics:?}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
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
        expect_parser_diag(err, "unsupported directive");
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
        let err = parse_str(PathBuf::from("test.isa"), ":param ENDIAN=big extra").unwrap_err();
        expect_parser_diag(err, "unexpected trailing tokens");
    }

    #[test]
    fn parser_reports_multiple_errors() {
        let source = ":param FOO=1 extra\n:param BAR=2 extra";
        let err = parse_str(PathBuf::from("test.isa"), source).unwrap_err();
        match err {
            IsaError::Diagnostics {
                phase: DiagnosticPhase::Parser,
                diagnostics,
            } => {
                assert!(
                    diagnostics.len() >= 2,
                    "expected multiple diagnostics: {diagnostics:?}"
                );
                let trailing = diagnostics
                    .iter()
                    .filter(|diag| diag.message.contains("unexpected trailing tokens"))
                    .count();
                assert!(
                    trailing >= 2,
                    "expected two trailing token diagnostics: {diagnostics:?}"
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn parses_include_in_coredef() {
        let doc = parse_core(":include \"base.isa\"");
        assert_eq!(doc.items.len(), 1);
        match &doc.items[0] {
            IsaItem::Include(include) => {
                assert_eq!(include.path, PathBuf::from("base.isa"));
                assert!(!include.optional);
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn rejects_include_outside_coredef() {
        let err = parse_str(PathBuf::from("test.isa"), ":include \"base.isa\"").unwrap_err();
        expect_parser_diag(err, "only allowed inside .coredef");
    }

    #[test]
    fn parses_macro_directive() {
        let doc = parse(":macro upd_cr0(res) {$reg::CR0 = #res}");
        assert_eq!(doc.items.len(), 1);
        match &doc.items[0] {
            IsaItem::Macro(mac) => {
                assert_eq!(mac.name, "upd_cr0");
                assert_eq!(mac.parameters, vec!["res".to_string()]);
                assert!(mac.semantics.source.contains("#res"));
            }
            other => panic!("unexpected item: {other:?}"),
        }
    }

    #[test]
    fn parses_extends_in_isaext() {
        let doc = parse_ext(":extends \"base.isa\"");
        assert!(doc.items.is_empty(), "extends should not create AST items");
        assert_eq!(doc.extends, vec![PathBuf::from("base.isa")]);
    }

    #[test]
    fn rejects_extends_outside_isaext() {
        let err = parse_str(PathBuf::from("test.isa"), ":extends \"base.isa\"").unwrap_err();
        expect_parser_diag(err, "only allowed inside .isaext");
    }
}
