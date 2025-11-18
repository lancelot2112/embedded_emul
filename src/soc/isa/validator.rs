//! Semantic validation for parsed ISA documents and the merged machine description.

use std::collections::{BTreeMap, BTreeSet};

use super::ast::{IsaDocument, IsaItem, SpaceDecl};
use super::error::IsaError;
use super::machine::MachineDescription;

pub struct Validator {
    seen_spaces: BTreeSet<String>,
    parameters: BTreeMap<String, String>,
}

impl Validator {
    pub fn new() -> Self {
        Self {
            seen_spaces: BTreeSet::new(),
            parameters: BTreeMap::new(),
        }
    }

    pub fn validate(&mut self, docs: &[IsaDocument]) -> Result<(), IsaError> {
        for doc in docs {
            for item in &doc.items {
                match item {
                    IsaItem::Space(space) => self.validate_space(space)?,
                    IsaItem::Parameter(param) => {
                        self.parameters
                            .insert(param.name.clone(), format!("{:?}", param.value));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub fn finalize_machine(&self, docs: Vec<IsaDocument>) -> Result<MachineDescription, IsaError> {
        MachineDescription::from_documents(docs)
    }

    fn validate_space(&mut self, space: &SpaceDecl) -> Result<(), IsaError> {
        if !self.seen_spaces.insert(space.name.clone()) {
            return Err(IsaError::Validation(format!(
                "space '{}' defined multiple times",
                space.name
            )));
        }
        Ok(())
    }
}
