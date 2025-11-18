//! Loading pipeline that resolves include trees, parses files, and produces a machine description.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::ast::{IncludeDecl, IsaDocument, IsaItem};
use super::error::IsaError;
use super::machine::MachineDescription;
use super::parser::parse_str;
use super::validator::Validator;

pub struct IsaLoader {
    visited: BTreeSet<PathBuf>,
    stack: Vec<PathBuf>,
}

impl IsaLoader {
    pub fn new() -> Self {
        Self {
            visited: BTreeSet::new(),
            stack: Vec::new(),
        }
    }

    pub fn load_machine<P: AsRef<Path>>(&mut self, entry: P) -> Result<MachineDescription, IsaError> {
        let docs = self.collect_documents(entry.as_ref())?;
        let mut validator = Validator::new();
        validator.validate(&docs)?;
        validator.finalize_machine(docs)
    }

    fn collect_documents(&mut self, path: &Path) -> Result<Vec<IsaDocument>, IsaError> {
        if !self.visited.insert(path.to_path_buf()) {
            return Ok(Vec::new());
        }
        if self.stack.contains(&path.to_path_buf()) {
            let mut chain = self.stack.iter().map(|p| p.display().to_string()).collect::<Vec<_>>();
            chain.push(path.display().to_string());
            return Err(IsaError::IncludeLoop { chain });
        }
        self.stack.push(path.to_path_buf());
        let src = fs::read_to_string(path)?;
        let doc = parse_str(path.to_path_buf(), &src)?;
        let mut docs = Vec::new();
        let mut include_docs = Vec::new();
        for item in &doc.items {
            if let IsaItem::Include(include) = item {
                match self.resolve_include(path, include) {
                    Ok(mut nested) => include_docs.append(&mut nested),
                    Err(err) if include.optional => {
                        eprintln!("optional include skipped: {err}");
                    }
                    Err(err) => return Err(err),
                }
            }
        }
        docs.push(doc);
        docs.extend(include_docs);
        self.stack.pop();
        Ok(docs)
    }

    fn resolve_include(
        &mut self,
        parent: &Path,
        include: &IncludeDecl,
    ) -> Result<Vec<IsaDocument>, IsaError> {
        let include_path = if include.path.is_relative() {
            parent
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(&include.path)
        } else {
            include.path.clone()
        };
        self.collect_documents(&include_path)
    }
}
