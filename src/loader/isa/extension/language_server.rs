use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::{
    Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, InitializeParams,
    InitializeResult, InitializedParams, MessageType, NumberOrString, Position, Range,
    SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokens,
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions, SemanticTokensParams,
    SemanticTokensResult, SemanticTokensServerCapabilities, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions, Url,
};
use tower_lsp::{Client, LanguageServer, LspService, Server, async_trait};

use crate::loader::isa::lexer::{Lexer, Token, TokenKind};
use crate::loader::isa::parse_str;
use crate::soc::isa::diagnostic::{DiagnosticLevel, IsaDiagnostic, SourceSpan};
use crate::soc::isa::error::IsaError;
use crate::soc::isa::validator::Validator;

const SUPPORTED_EXTENSIONS: &[&str] = &["isa", "isaext", "coredef", "sysdef"];
const DIRECTIVE_KEYWORDS: &[&str] = &[
    "space",
    "param",
    "fileset",
    "reg",
    "include",
    "macro",
    "insn",
    "instruction",
    "operator",
    "form",
    "subspace",
    "state",
    "context",
];
const NAMED_DIRECTIVES: &[&str] = &[
    "space", "reg", "insn", "macro", "fileset", "context", "state", "operator", "subspace", "form",
];
const SEMANTIC_TOKEN_TYPES: &[&str] = &[
    "comment",
    "string",
    "number",
    "nanemuDirective",
    "nanemuSpace",
    "nanemuEntity",
    "nanemuOption",
    "nanemuOptionValue",
];

#[derive(Clone)]
struct DocumentEntry {
    text: String,
    version: i32,
}

#[derive(Clone, Copy)]
enum HighlightKind {
    Comment,
    String,
    Number,
    Directive,
    Space,
    Entity,
    Option,
    OptionValue,
}

impl HighlightKind {
    fn index(self) -> u32 {
        match self {
            HighlightKind::Comment => 0,
            HighlightKind::String => 1,
            HighlightKind::Number => 2,
            HighlightKind::Directive => 3,
            HighlightKind::Space => 4,
            HighlightKind::Entity => 5,
            HighlightKind::Option => 6,
            HighlightKind::OptionValue => 7,
        }
    }
}

struct HighlightSpan {
    line: u32,
    start: u32,
    length: u32,
    kind: HighlightKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NameKind {
    Space,
    Entity,
}

/// Minimal LSP backend that surfaces parser diagnostics for ISA family documents.
pub struct IsaLanguageServer {
    client: Client,
    documents: Arc<RwLock<HashMap<Url, DocumentEntry>>>,
    legend: SemanticTokensLegend,
}

impl IsaLanguageServer {
    pub fn new(client: Client) -> Self {
        let legend = SemanticTokensLegend {
            token_types: SEMANTIC_TOKEN_TYPES
                .iter()
                .map(|value| SemanticTokenType::new(*value))
                .collect(),
            token_modifiers: Vec::<SemanticTokenModifier>::new(),
        };
        Self {
            client,
            documents: Arc::new(RwLock::new(HashMap::new())),
            legend,
        }
    }

    async fn refresh_document(&self, uri: &Url) {
        if !Self::is_supported_uri(uri) {
            return;
        }
        let snapshot = {
            let docs = self.documents.read().await;
            docs.get(uri).cloned()
        };
        if let Some(entry) = snapshot {
            let diagnostics = self.parse_document(uri, &entry.text);
            self.client
                .publish_diagnostics(uri.clone(), diagnostics, Some(entry.version))
                .await;
        }
    }

    async fn remove_document(&self, uri: &Url) {
        self.documents.write().await.remove(uri);
        self.client
            .publish_diagnostics(uri.clone(), Vec::new(), None)
            .await;
    }

    fn parse_document(&self, uri: &Url, text: &str) -> Vec<Diagnostic> {
        if !Self::is_supported_uri(uri) {
            return Vec::new();
        }
        match Self::parse_and_validate(Self::path_from_uri(uri), text) {
            Ok(_) => Vec::new(),
            Err(err) => Self::diagnostics_from_error(uri, err),
        }
    }

    fn parse_and_validate(path: PathBuf, text: &str) -> Result<(), IsaError> {
        let doc = parse_str(path, text)?;
        let docs = vec![doc];
        let mut validator = Validator::new();
        validator.validate(&docs)?;
        Ok(())
    }

    fn diagnostics_from_error(uri: &Url, err: IsaError) -> Vec<Diagnostic> {
        match err {
            IsaError::Diagnostics { diagnostics, .. } => diagnostics
                .into_iter()
                .map(|diag| Self::isa_diag_to_lsp(uri, diag))
                .collect(),
            other => vec![Self::message_only_diagnostic(&other.to_string())],
        }
    }

    fn isa_diag_to_lsp(uri: &Url, diag: IsaDiagnostic) -> Diagnostic {
        let IsaDiagnostic {
            phase,
            level,
            code,
            message,
            span,
        } = diag;
        let range = span.as_ref().map(Self::range_from_span).unwrap_or_default();
        let severity = Some(match level {
            DiagnosticLevel::Error => DiagnosticSeverity::ERROR,
            DiagnosticLevel::Warning => DiagnosticSeverity::WARNING,
        });
        Diagnostic {
            range,
            severity,
            code: Some(NumberOrString::String(code.into())),
            source: Some(format!("{:?}", phase)),
            message,
            related_information: span.as_ref().and_then(|span| {
                if Self::matches_uri(uri, span) {
                    None
                } else {
                    Some(vec![DiagnosticRelatedInformation {
                        location: tower_lsp::lsp_types::Location::new(
                            Url::from_file_path(&span.path).unwrap_or_else(|_| uri.clone()),
                            Self::range_from_span(span),
                        ),
                        message: format!("referenced in {}", span.path.display()),
                    }])
                }
            }),
            ..Diagnostic::default()
        }
    }

    fn message_only_diagnostic(message: &str) -> Diagnostic {
        Diagnostic {
            range: Range::default(),
            severity: Some(DiagnosticSeverity::ERROR),
            message: message.to_string(),
            source: Some("nanemu-isa".into()),
            ..Diagnostic::default()
        }
    }

    fn range_from_span(span: &SourceSpan) -> Range {
        Range {
            start: Position {
                line: span.start.line.saturating_sub(1) as u32,
                character: span.start.column.saturating_sub(1) as u32,
            },
            end: Position {
                line: span.end.line.saturating_sub(1) as u32,
                character: span.end.column.saturating_sub(1) as u32,
            },
        }
    }

    fn path_from_uri(uri: &Url) -> PathBuf {
        uri.to_file_path()
            .unwrap_or_else(|_| PathBuf::from(uri.path()))
    }

    fn is_supported_uri(uri: &Url) -> bool {
        Self::path_from_uri(uri)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                let ext = ext.to_ascii_lowercase();
                SUPPORTED_EXTENSIONS.contains(&ext.as_str())
            })
            .unwrap_or(false)
    }

    fn matches_uri(uri: &Url, span: &SourceSpan) -> bool {
        if let Ok(span_uri) = Url::from_file_path(&span.path) {
            uri == &span_uri
        } else {
            false
        }
    }

    fn build_semantic_tokens(&self, uri: &Url, text: &str) -> Vec<SemanticToken> {
        Self::build_semantic_tokens_for_text(uri, text)
    }

    fn build_semantic_tokens_for_text(uri: &Url, text: &str) -> Vec<SemanticToken> {
        let mut spans = Self::collect_comment_spans(text);
        let tokens = Self::collect_tokens(uri, text);
        let mut option_idents = HashSet::new();
        let mut option_equals = HashSet::new();
        let mut option_values = HashSet::new();
        for idx in 0..tokens.len().saturating_sub(1) {
            if tokens[idx].kind == TokenKind::Identifier
                && tokens[idx + 1].kind == TokenKind::Equals
            {
                option_idents.insert(idx);
                option_equals.insert(idx + 1);
                if idx + 2 < tokens.len() && tokens[idx + 2].kind == TokenKind::Identifier {
                    option_values.insert(idx + 2);
                }
            }
        }

        let mut line_cache: Option<Vec<Vec<char>>> = None;
        let mut expect_directive = false;
        let mut expect_name: Option<NameKind> = None;
        let mut expect_chained_name: Option<NameKind> = None;
        let mut last_name_kind: Option<NameKind> = None;

        for (idx, token) in tokens.iter().enumerate() {
            match token.kind {
                TokenKind::EOF => break,
                TokenKind::Colon => {
                    expect_directive = true;
                    expect_chained_name = None;
                }
                TokenKind::DoubleColon => {
                    expect_chained_name = last_name_kind;
                }
                TokenKind::Identifier => {
                    if expect_directive {
                        let lower = token.lexeme.to_ascii_lowercase();
                        if DIRECTIVE_KEYWORDS.contains(&lower.as_str()) {
                            if lower == "space" {
                                spans.push(Self::span_from_token(
                                    token,
                                    HighlightKind::Space,
                                    None,
                                ));
                                expect_name = Some(NameKind::Space);
                            } else {
                                spans.push(Self::span_from_token(
                                    token,
                                    HighlightKind::Directive,
                                    None,
                                ));
                                if NAMED_DIRECTIVES.contains(&lower.as_str()) {
                                    expect_name = Some(NameKind::Entity);
                                } else {
                                    expect_name = None;
                                }
                            }
                        } else {
                            spans.push(Self::span_from_token(
                                token,
                                HighlightKind::Directive,
                                None,
                            ));
                            expect_name = None;
                        }
                        expect_directive = false;
                        expect_chained_name = None;
                        last_name_kind = None;
                    } else if let Some(kind) =
                        expect_chained_name.take().or_else(|| expect_name.take())
                    {
                        let highlight_kind = match kind {
                            NameKind::Space => HighlightKind::Space,
                            NameKind::Entity => HighlightKind::Entity,
                        };
                        spans.push(Self::span_from_token(token, highlight_kind, None));
                        last_name_kind = Some(kind);
                    } else if option_idents.contains(&idx) {
                        spans.push(Self::span_from_token(token, HighlightKind::Option, None));
                        last_name_kind = None;
                    } else if option_values.contains(&idx) {
                        spans.push(Self::span_from_token(
                            token,
                            HighlightKind::OptionValue,
                            None,
                        ));
                        last_name_kind = None;
                    } else {
                        last_name_kind = None;
                    }
                }
                TokenKind::Number => {
                    spans.push(Self::span_from_token(token, HighlightKind::Number, None));
                    last_name_kind = None;
                }
                TokenKind::String => {
                    if line_cache.is_none() {
                        line_cache = Some(Self::collect_line_chars(text));
                    }
                    let length_override = line_cache.as_ref().and_then(|lines| {
                        Self::string_literal_length(
                            lines,
                            token.line.saturating_sub(1),
                            token.column.saturating_sub(1),
                        )
                    });
                    spans.push(Self::span_from_token(
                        token,
                        HighlightKind::String,
                        length_override,
                    ));
                    last_name_kind = None;
                }
                TokenKind::Equals => {
                    if option_equals.contains(&idx) {
                        spans.push(Self::span_from_token(token, HighlightKind::Option, None));
                    }
                    last_name_kind = None;
                }
                _ => {}
            }
        }

        spans.sort_by(|a, b| (a.line, a.start).cmp(&(b.line, b.start)));
        Self::encode_semantic_tokens(spans)
    }

    fn collect_tokens(uri: &Url, text: &str) -> Vec<Token> {
        let mut lexer = Lexer::new(text, Self::path_from_uri(uri));
        let mut tokens = Vec::new();
        loop {
            match lexer.next_token() {
                Ok(token) => {
                    let eof = token.kind == TokenKind::EOF;
                    tokens.push(token);
                    if eof {
                        break;
                    }
                }
                Err(err) => {
                    eprintln!("semantic token lexer error: {err}");
                    break;
                }
            }
        }
        tokens
    }

    fn span_from_token(
        token: &Token,
        kind: HighlightKind,
        length_override: Option<u32>,
    ) -> HighlightSpan {
        let line = token.line.saturating_sub(1) as u32;
        let start = token.column.saturating_sub(1) as u32;
        let length = length_override.unwrap_or_else(|| token.lexeme.chars().count() as u32);
        HighlightSpan {
            line,
            start,
            length: length.max(1),
            kind,
        }
    }

    fn collect_line_chars(text: &str) -> Vec<Vec<char>> {
        text.split('\n')
            .map(|line| line.chars().collect())
            .collect()
    }

    fn string_literal_length(
        lines: &[Vec<char>],
        line_idx: usize,
        start_col: usize,
    ) -> Option<u32> {
        let line = lines.get(line_idx)?;
        if start_col >= line.len() {
            return None;
        }
        let mut length = 0u32;
        let mut seen_opening = false;
        let mut escape = false;
        for &ch in &line[start_col..] {
            length += 1;
            if !seen_opening {
                seen_opening = true;
                escape = ch == '\\';
                continue;
            }
            if escape {
                escape = false;
                continue;
            }
            if ch == '\\' {
                escape = true;
                continue;
            }
            if ch == '"' {
                break;
            }
        }
        Some(length)
    }

    fn encode_semantic_tokens(spans: Vec<HighlightSpan>) -> Vec<SemanticToken> {
        let mut data = Vec::with_capacity(spans.len());
        let mut prev_line = 0u32;
        let mut prev_start = 0u32;
        for span in spans {
            let delta_line = span.line.saturating_sub(prev_line);
            let delta_start = if delta_line == 0 {
                span.start.saturating_sub(prev_start)
            } else {
                span.start
            };
            data.push(SemanticToken {
                delta_line,
                delta_start,
                length: span.length,
                token_type: span.kind.index(),
                token_modifiers_bitset: 0,
            });
            prev_line = span.line;
            prev_start = span.start;
        }
        data
    }

    fn collect_comment_spans(text: &str) -> Vec<HighlightSpan> {
        let mut spans = Vec::new();
        let mut chars = text.chars().peekable();
        let mut line = 0u32;
        let mut column = 0u32;
        let mut in_string = false;
        let mut escape = false;

        while let Some(ch) = chars.next() {
            match ch {
                '\n' => {
                    line += 1;
                    column = 0;
                    escape = false;
                    in_string = false;
                    continue;
                }
                '\r' => {
                    column = 0;
                    continue;
                }
                '"' if !escape => {
                    in_string = !in_string;
                    column += 1;
                    continue;
                }
                '\\' if in_string && !escape => {
                    escape = true;
                    column += 1;
                    continue;
                }
                _ => {
                    escape = false;
                }
            }

            if !in_string && ch == '/' && chars.peek() == Some(&'/') {
                chars.next();
                let start = column;
                let mut length = 2u32;
                while let Some(next) = chars.peek() {
                    if *next == '\n' || *next == '\r' {
                        break;
                    }
                    chars.next();
                    length += 1;
                }
                spans.push(HighlightSpan {
                    line,
                    start,
                    length,
                    kind: HighlightKind::Comment,
                });
                column += length;
                continue;
            }

            column += 1;
        }

        spans
    }
}

#[async_trait]
impl LanguageServer for IsaLanguageServer {
    async fn initialize(&self, _: InitializeParams) -> LspResult<InitializeResult> {
        let capabilities = ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Options(
                TextDocumentSyncOptions {
                    open_close: Some(true),
                    change: Some(TextDocumentSyncKind::FULL),
                    ..Default::default()
                },
            )),
            semantic_tokens_provider: Some(
                SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                    legend: self.legend.clone(),
                    full: Some(SemanticTokensFullOptions::Bool(true)),
                    ..SemanticTokensOptions::default()
                }),
            ),
            ..ServerCapabilities::default()
        };
        Ok(InitializeResult {
            capabilities,
            server_info: Some(ServerInfo {
                name: "nanemu-isa".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "ISA language server initialized")
            .await;
    }

    async fn shutdown(&self) -> LspResult<()> {
        Ok(())
    }

    async fn did_open(&self, params: tower_lsp::lsp_types::DidOpenTextDocumentParams) {
        let doc = params.text_document;
        if !Self::is_supported_uri(&doc.uri) {
            return;
        }
        self.documents.write().await.insert(
            doc.uri.clone(),
            DocumentEntry {
                text: doc.text.clone(),
                version: doc.version,
            },
        );
        self.refresh_document(&doc.uri).await;
    }

    async fn did_change(&self, params: tower_lsp::lsp_types::DidChangeTextDocumentParams) {
        if params.content_changes.is_empty() {
            return;
        }
        let uri = params.text_document.uri;
        if !Self::is_supported_uri(&uri) {
            return;
        }
        let new_text = params
            .content_changes
            .last()
            .map(|change| change.text.clone())
            .unwrap_or_default();
        self.documents.write().await.insert(
            uri.clone(),
            DocumentEntry {
                text: new_text,
                version: params.text_document.version,
            },
        );
        self.refresh_document(&uri).await;
    }

    async fn did_close(&self, params: tower_lsp::lsp_types::DidCloseTextDocumentParams) {
        self.remove_document(&params.text_document.uri).await;
    }

    async fn semantic_tokens_full(
        &self,
        params: SemanticTokensParams,
    ) -> LspResult<Option<SemanticTokensResult>> {
        let uri = params.text_document.uri;
        let snapshot = {
            let docs = self.documents.read().await;
            docs.get(&uri).cloned()
        };

        if let Some(entry) = snapshot {
            let data = self.build_semantic_tokens(&uri, &entry.text);
            Ok(Some(
                SemanticTokens {
                    result_id: None,
                    data,
                }
                .into(),
            ))
        } else {
            Ok(Some(
                SemanticTokens {
                    result_id: None,
                    data: Vec::new(),
                }
                .into(),
            ))
        }
    }
}

async fn run_stdio_language_server_impl<F>(factory: F) -> LspResult<()>
where
    F: Fn(Client) -> IsaLanguageServer,
{
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::build(|client| factory(client)).finish();
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}

pub async fn run_stdio_language_server() -> LspResult<()> {
    run_stdio_language_server_impl(IsaLanguageServer::new).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tower_lsp::lsp_types::Url;

    #[test]
    fn highlights_directives_options_and_comments() {
        let uri = Url::parse("file:///test.isa").unwrap();
        let text = r#":space core0 {
  option = "value"
  size = 4 // comment
}"#;

        let tokens = IsaLanguageServer::build_semantic_tokens_for_text(&uri, text);
        let captured = capture_tokens(text, &tokens);

        assert_token(&captured, "space", HighlightKind::Space);
        assert_token(&captured, "core0", HighlightKind::Space);
        assert_token(&captured, "option", HighlightKind::Option);
        assert_token(&captured, "=", HighlightKind::Option);
        assert_token(&captured, "4", HighlightKind::Number);
        assert_token_contains(&captured, "value", HighlightKind::String);
        assert_token_contains(&captured, "comment", HighlightKind::Comment);
    }

    #[test]
    fn highlights_named_and_chained_entities() {
        let uri = Url::parse("file:///test2.isa").unwrap();
        let text = ":insn add ::core0::stage0\n:reg sample";

        let tokens = IsaLanguageServer::build_semantic_tokens_for_text(&uri, text);
        let captured = capture_tokens(text, &tokens);

        assert_token(&captured, "insn", HighlightKind::Directive);
        assert_token(&captured, "add", HighlightKind::Entity);
        assert_token(&captured, "core0", HighlightKind::Entity);
        assert_token(&captured, "stage0", HighlightKind::Entity);
        assert_token(&captured, "reg", HighlightKind::Directive);
        assert_token(&captured, "sample", HighlightKind::Entity);
    }

    #[test]
    fn highlights_option_values() {
        let uri = Url::parse("file:///test3.isa").unwrap();
        let text = ":space reg addr=32 word=64 type=register align=16 endian=big";

        let tokens = IsaLanguageServer::build_semantic_tokens_for_text(&uri, text);
        let captured = capture_tokens(text, &tokens);

        for key in ["addr", "word", "type", "align", "endian"] {
            assert_token(&captured, key, HighlightKind::Option);
        }

        for value in ["register", "big"] {
            assert_token(&captured, value, HighlightKind::OptionValue);
        }

        assert_token(&captured, "32", HighlightKind::Number);
        assert_token(&captured, "64", HighlightKind::Number);
    }

    #[test]
    fn reports_validation_errors_for_invalid_redirect() {
        let text = ":space reg addr=32 word=64 type=register\n:reg alias redirect=PC";
        let err = IsaLanguageServer::parse_and_validate(PathBuf::from("test.isa"), text)
            .expect_err("expected redirect validation error");
        match err {
            IsaError::Diagnostics { diagnostics, .. } => {
                assert!(
                    diagnostics
                        .iter()
                        .any(|diag| diag.code == "validation.redirect.unknown-field")
                );
            }
            other => panic!("unexpected error variant: {:?}", other),
        }
    }

    struct CapturedToken {
        text: String,
        kind: u32,
    }

    fn capture_tokens(text: &str, tokens: &[SemanticToken]) -> Vec<CapturedToken> {
        let lines: Vec<Vec<char>> = text
            .split('\n')
            .map(|line| line.chars().collect::<Vec<_>>())
            .collect();
        let mut results = Vec::new();
        let mut line = 0u32;
        let mut start = 0u32;
        for token in tokens {
            line += token.delta_line;
            if token.delta_line == 0 {
                start += token.delta_start;
            } else {
                start = token.delta_start;
            }
            let snippet = lines
                .get(line as usize)
                .map(|chars| {
                    chars
                        .iter()
                        .skip(start as usize)
                        .take(token.length as usize)
                        .collect::<String>()
                })
                .unwrap_or_default();
            results.push(CapturedToken {
                text: snippet,
                kind: token.token_type,
            });
        }
        results
    }

    fn assert_token(tokens: &[CapturedToken], lexeme: &str, kind: HighlightKind) {
        assert!(
            tokens
                .iter()
                .any(|token| token.text == lexeme && token.kind == kind.index()),
            "missing token '{lexeme}' with kind index {}",
            kind.index()
        );
    }

    fn assert_token_contains(tokens: &[CapturedToken], needle: &str, kind: HighlightKind) {
        assert!(
            tokens
                .iter()
                .any(|token| token.text.contains(needle) && token.kind == kind.index()),
            "missing token containing '{needle}' with kind index {}",
            kind.index()
        );
    }
}
