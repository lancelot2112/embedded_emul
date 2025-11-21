//! Loading pipeline that resolves include trees, parses files, and produces a machine description.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use crate::loader::isa::parse_str_with_spaces;
use crate::soc::isa::ast::{
    FieldDecl, IncludeDecl, IsaItem, IsaSpecification, SpaceDecl, SpaceKind, SpaceMember,
};
use crate::soc::isa::error::IsaError;
use crate::soc::isa::machine::MachineDescription;
use crate::soc::isa::validator::Validator;

#[derive(Default)]
pub struct IsaLoader {
    visited: BTreeSet<PathBuf>,
    stack: Vec<PathBuf>,
    known_spaces: HashMap<String, SpaceKind>,
}

impl IsaLoader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load_machine<P: AsRef<Path>>(
        &mut self,
        entry: P,
    ) -> Result<MachineDescription, IsaError> {
        self.visited.clear();
        self.stack.clear();
        self.known_spaces.clear();
        let docs = self.collect_documents(entry.as_ref())?;
        if Self::is_coredef(entry.as_ref()) {
            Self::verify_coredef_compatibility(entry.as_ref(), &docs)?;
        }
        let mut validator = Validator::new();
        validator.validate(&docs)?;
        validator.finalize_machine(docs)
    }

    fn collect_documents(&mut self, path: &Path) -> Result<Vec<IsaSpecification>, IsaError> {
        if !self.visited.insert(path.to_path_buf()) {
            return Ok(Vec::new());
        }
        if self.stack.contains(&path.to_path_buf()) {
            let mut chain = self
                .stack
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>();
            chain.push(path.display().to_string());
            return Err(IsaError::IncludeLoop { chain });
        }
        self.stack.push(path.to_path_buf());
        let src = fs::read_to_string(path)?;
        let doc = parse_str_with_spaces(path.to_path_buf(), &src, &self.known_spaces)?;
        self.record_spaces(&doc);
        let mut docs = Vec::new();
        if Self::is_coredef(path) {
            self.collect_coredef(path, &doc, &mut docs)?;
        } else {
            let mut include_docs = Vec::new();
            for item in &doc.items {
                if let IsaItem::Include(include) = item {
                    let include = include.clone();
                    match self.resolve_include(path, &include) {
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
        }
        self.stack.pop();
        Ok(docs)
    }

    fn resolve_include(
        &mut self,
        parent: &Path,
        include: &IncludeDecl,
    ) -> Result<Vec<IsaSpecification>, IsaError> {
        let include_path = Self::resolve_include_path(parent, include);
        self.collect_documents(&include_path)
    }

    fn resolve_include_path(parent: &Path, include: &IncludeDecl) -> PathBuf {
        if include.path.is_relative() {
            parent
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(&include.path)
        } else {
            include.path.clone()
        }
    }

    fn collect_coredef(
        &mut self,
        parent: &Path,
        doc: &IsaSpecification,
        acc: &mut Vec<IsaSpecification>,
    ) -> Result<(), IsaError> {
        let mut includes = Vec::new();
        for item in &doc.items {
            match item {
                IsaItem::Include(include) => includes.push(include.clone()),
                _ => {
                    return Err(IsaError::Machine(format!(
                        "coredef '{}' may only contain :include directives",
                        parent.display()
                    )));
                }
            }
        }
        if includes.is_empty() {
            return Err(IsaError::Machine(format!(
                "coredef '{}' must declare exactly one base .isa include",
                parent.display()
            )));
        }
        let mut base_seen = false;
        for include in includes {
            let include_path = Self::resolve_include_path(parent, &include);
            let ext = include_path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.to_ascii_lowercase())
                .unwrap_or_default();
            match ext.as_str() {
                "isa" => {
                    if base_seen {
                        return Err(IsaError::Machine(format!(
                            "coredef '{}' cannot include multiple base .isa files",
                            parent.display()
                        )));
                    }
                    base_seen = true;
                }
                "isaext" => {
                    if !base_seen {
                        return Err(IsaError::Machine(format!(
                            "coredef '{}' must include a base .isa before any .isaext files",
                            parent.display()
                        )));
                    }
                }
                other => {
                    return Err(IsaError::Machine(format!(
                        "coredef '{}' includes unsupported file extension '.{}'",
                        parent.display(),
                        other
                    )));
                }
            }

            match self.collect_documents(&include_path) {
                Ok(mut nested) => acc.append(&mut nested),
                Err(err) if include.optional => {
                    eprintln!("optional include skipped: {err}");
                }
                Err(err) => return Err(err),
            }
        }
        if !base_seen {
            return Err(IsaError::Machine(format!(
                "coredef '{}' must include exactly one base .isa",
                parent.display()
            )));
        }
        Ok(())
    }

    fn is_coredef(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("coredef"))
            .unwrap_or(false)
    }

    fn record_spaces(&mut self, doc: &IsaSpecification) {
        for item in &doc.items {
            if let IsaItem::Space(SpaceDecl { name, kind, .. }) = item {
                self.known_spaces.insert(name.clone(), kind.clone());
            }
        }
    }

    fn verify_coredef_compatibility(
        coredef: &Path,
        docs: &[IsaSpecification],
    ) -> Result<(), IsaError> {
        let mut state = CoreCompatibilityState::default();
        for doc in docs {
            match file_kind(&doc.path) {
                FileKind::Isa => state.record_base(doc),
                FileKind::IsaExt => state.check_extension(coredef, doc)?,
                FileKind::Other => {}
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct CoreCompatibilityState {
    spaces: BTreeSet<String>,
    fields: BTreeMap<String, BTreeSet<String>>,
}

impl CoreCompatibilityState {
    fn record_base(&mut self, doc: &IsaSpecification) {
        for item in &doc.items {
            if let IsaItem::Space(space) = item {
                self.register_space(&space.name);
            }
        }
        for item in &doc.items {
            if let IsaItem::SpaceMember(member) = item {
                self.ensure_space_slot(&member.space);
                if let SpaceMember::Field(field) = &member.member {
                    let names = expand_field_names(field);
                    self.register_fields(&field.space, names);
                }
            }
        }
    }

    fn check_extension(&mut self, coredef: &Path, doc: &IsaSpecification) -> Result<(), IsaError> {
        for item in &doc.items {
            if let IsaItem::Space(space) = item {
                self.register_space(&space.name);
            }
        }
        for item in &doc.items {
            match item {
                IsaItem::SpaceMember(member) => {
                    self.ensure_space_known(coredef, doc, &member.space)?;
                    if let SpaceMember::Field(field) = &member.member {
                        self.check_extension_field(coredef, doc, field)?;
                    }
                }
                IsaItem::Instruction(instr) => {
                    self.ensure_space_known(coredef, doc, &instr.space)?;
                }
                IsaItem::Space(_) | IsaItem::Parameter(_) | IsaItem::Include(_) => {}
                IsaItem::Macro(_) => {}
            }
        }
        Ok(())
    }

    fn ensure_space_known(
        &self,
        coredef: &Path,
        doc: &IsaSpecification,
        space: &str,
    ) -> Result<(), IsaError> {
        if self.spaces.contains(space) {
            Ok(())
        } else {
            Err(IsaError::Machine(format!(
                "coredef '{}' extension '{}' references unknown space '{}'",
                coredef.display(),
                doc.path.display(),
                space
            )))
        }
    }

    fn check_extension_field(
        &mut self,
        coredef: &Path,
        doc: &IsaSpecification,
        field: &FieldDecl,
    ) -> Result<(), IsaError> {
        let names = expand_field_names(field);
        if is_append_only(field) {
            for name in &names {
                if !self
                    .fields
                    .get(&field.space)
                    .map(|set| set.contains(name))
                    .unwrap_or(false)
                {
                    return Err(IsaError::Machine(format!(
                        "coredef '{}' extension '{}' appends to undefined field '{}::{}'",
                        coredef.display(),
                        doc.path.display(),
                        field.space,
                        name
                    )));
                }
            }
        } else {
            self.register_fields(&field.space, names);
        }
        Ok(())
    }

    fn register_space(&mut self, space: &str) {
        self.spaces.insert(space.to_string());
        self.ensure_space_slot(space);
    }

    fn register_fields(&mut self, space: &str, names: Vec<String>) {
        self.ensure_space_slot(space);
        let set = self.fields.get_mut(space).unwrap();
        for name in names {
            set.insert(name);
        }
    }

    fn ensure_space_slot(&mut self, space: &str) {
        self.fields.entry(space.to_string()).or_default();
        self.spaces.insert(space.to_string());
    }
}

#[derive(PartialEq, Eq)]
enum FileKind {
    Isa,
    IsaExt,
    Other,
}

fn file_kind(path: &Path) -> FileKind {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "isa" => FileKind::Isa,
        "isaext" => FileKind::IsaExt,
        _ => FileKind::Other,
    }
}

fn expand_field_names(field: &FieldDecl) -> Vec<String> {
    if let Some(range) = &field.range {
        let mut names = Vec::new();
        for index in range.start..=range.end {
            names.push(format!("{}{}", field.name, index));
        }
        names
    } else {
        vec![field.name.clone()]
    }
}

fn is_append_only(field: &FieldDecl) -> bool {
    field.offset.is_none()
        && field.size.is_none()
        && field.reset.is_none()
        && field.description.is_none()
        && field.redirect.is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn coredef_rejects_multiple_base_isas() {
        let dir = tempdir().expect("tempdir");
        write_file(dir.path(), "base1.isa", "");
        write_file(dir.path(), "base2.isa", "");
        let coredef = write_file(
            dir.path(),
            "core.coredef",
            ":include \"base1.isa\"\n:include \"base2.isa\"",
        );
        let mut loader = IsaLoader::new();
        let err = loader.collect_documents(coredef.as_path()).unwrap_err();
        assert!(matches!(
            err,
            IsaError::Machine(msg) if msg.contains("multiple base .isa")
        ));
    }

    #[test]
    fn coredef_requires_base_before_extension() {
        let dir = tempdir().expect("tempdir");
        write_file(dir.path(), "base.isa", "");
        write_file(dir.path(), "extra.isaext", "");
        let coredef = write_file(
            dir.path(),
            "core.coredef",
            ":include \"extra.isaext\"\n:include \"base.isa\"",
        );
        let mut loader = IsaLoader::new();
        let err = loader.collect_documents(coredef.as_path()).unwrap_err();
        assert!(matches!(
            err,
            IsaError::Machine(msg) if msg.contains("base .isa before any .isaext")
        ));
    }

    #[test]
    fn coredef_extension_requires_base_field_for_appends() {
        let dir = tempdir().expect("tempdir");
        write_file(
            dir.path(),
            "base.isa",
            ":space reg addr=32 word=64 type=register\n:reg MSR subfields={ BASE @(0) }\n",
        );
        write_file(
            dir.path(),
            "extra.isaext",
            ":reg MSR_MISSING\nsubfields={\n    NEWOV @(31)\n}\n",
        );
        let coredef = write_file(
            dir.path(),
            "core.coredef",
            ":include \"base.isa\"\n:include \"extra.isaext\"",
        );
        let mut loader = IsaLoader::new();
        let docs = loader
            .collect_documents(coredef.as_path())
            .expect("documents");
        let err = IsaLoader::verify_coredef_compatibility(coredef.as_path(), &docs).unwrap_err();
        assert!(matches!(
            err,
            IsaError::Machine(msg) if msg.contains("appends to undefined field")
        ));
    }

    #[test]
    fn coredef_extension_appends_existing_field() {
        let dir = tempdir().expect("tempdir");
        write_file(
            dir.path(),
            "base.isa",
            ":space reg addr=32 word=64 type=register\n:reg MSR subfields={ BASE @(0) }\n",
        );
        write_file(
            dir.path(),
            "extra.isaext",
            ":reg MSR\nsubfields={\n    NEWOV @(31)\n}\n",
        );
        let coredef = write_file(
            dir.path(),
            "core.coredef",
            ":include \"base.isa\"\n:include \"extra.isaext\"",
        );
        let mut loader = IsaLoader::new();
        let docs = loader
            .collect_documents(coredef.as_path())
            .expect("documents");
        IsaLoader::verify_coredef_compatibility(coredef.as_path(), &docs)
            .expect("compatibility passes");
    }

    #[test]
    fn coredef_extension_can_define_new_space() {
        let dir = tempdir().expect("tempdir");
        write_file(
            dir.path(),
            "base.isa",
            ":space reg addr=32 word=64 type=register\n:reg MSR subfields={ BASE @(0) }\n",
        );
        write_file(
            dir.path(),
            "extra.isaext",
            ":space vle addr=32 type=logic word=16 align=16 endian=big\n",
        );
        let coredef = write_file(
            dir.path(),
            "core.coredef",
            ":include \"base.isa\"\n:include \"extra.isaext\"",
        );
        let mut loader = IsaLoader::new();
        let docs = loader
            .collect_documents(coredef.as_path())
            .expect("documents");
        IsaLoader::verify_coredef_compatibility(coredef.as_path(), &docs)
            .expect("compatibility allows new spaces");
    }

    fn write_file(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, contents).expect("write file");
        path
    }
}
