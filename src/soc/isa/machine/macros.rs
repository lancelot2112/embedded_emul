//! Macro metadata captured from `.isa` sources and stored on the machine runtime.

use crate::soc::isa::ast::MacroDecl;
use crate::soc::isa::diagnostic::SourceSpan;
use crate::soc::isa::semantics::SemanticBlock;

#[derive(Debug, Clone)]
pub struct MacroInfo {
    pub name: String,
    pub parameters: Vec<String>,
    pub semantics: SemanticBlock,
    pub span: SourceSpan,
}

impl MacroInfo {
    pub fn from_decl(decl: MacroDecl) -> Self {
        Self {
            name: decl.name,
            parameters: decl.parameters,
            semantics: decl.semantics,
            span: decl.span,
        }
    }
}
