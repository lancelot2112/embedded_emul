use std::path::PathBuf;

/// Phase of the pipeline that produced a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticPhase {
    Lexer,
    Parser,
    Validation,
}

/// Severity of an ISA diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticLevel {
    Error,
    Warning,
}

/// A precise source position (1-indexed line/column) inside an ISA document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

impl SourcePosition {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// A half-open [start, end) span referencing a specific ISA file.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceSpan {
    pub path: PathBuf,
    pub start: SourcePosition,
    pub end: SourcePosition,
}

impl SourceSpan {
    pub fn new(path: PathBuf, start: SourcePosition, end: SourcePosition) -> Self {
        Self { path, start, end }
    }

    pub fn point(path: PathBuf, position: SourcePosition) -> Self {
        Self {
            path,
            start: position,
            end: position,
        }
    }
}

/// Structured diagnostic suitable for tooling integration.
#[derive(Debug, Clone)]
pub struct IsaDiagnostic {
    pub phase: DiagnosticPhase,
    pub level: DiagnosticLevel,
    pub code: &'static str,
    pub message: String,
    pub span: Option<SourceSpan>,
}

impl IsaDiagnostic {
    pub fn new(
        phase: DiagnosticPhase,
        level: DiagnosticLevel,
        code: &'static str,
        message: impl Into<String>,
        span: Option<SourceSpan>,
    ) -> Self {
        Self {
            phase,
            level,
            code,
            message: message.into(),
            span,
        }
    }

    pub fn format_human(&self) -> String {
        let location = self
            .span
            .as_ref()
            .map(|span| {
                format!(
                    "{}:{}:{}",
                    span.path.display(),
                    span.start.line,
                    span.start.column
                )
            })
            .unwrap_or_else(|| "<unknown>".to_string());
        format!(
            "{level:?} {code}: {message} @ {location}",
            level = self.level,
            code = self.code,
            message = self.message,
            location = location
        )
    }
}
