use std::collections::BTreeMap;

use super::ast::{FormDecl, SubFieldDecl};

#[derive(Default)]
pub(crate) struct LogicForm {
    pub subfields: BTreeMap<String, SubFieldDecl>,
}

#[derive(Default)]
pub(crate) struct LogicSpaceState {
    _word_size: u32,
    forms: BTreeMap<String, LogicForm>,
}

impl LogicSpaceState {
    pub(crate) fn new(word_size: u32) -> Self {
        Self {
            _word_size: word_size,
            forms: BTreeMap::new(),
        }
    }

    pub(crate) fn register_form(&mut self, form: &FormDecl) -> Result<(), LogicFormError> {
        if self.forms.contains_key(&form.name) {
            return Err(LogicFormError::DuplicateForm {
                name: form.name.clone(),
            });
        }
        if form.subfields.is_empty() {
            return Err(LogicFormError::MissingSubfields {
                name: form.name.clone(),
            });
        }
        let mut merged = if let Some(parent) = &form.parent {
            let parent_form =
                self.forms
                    .get(parent)
                    .ok_or_else(|| LogicFormError::MissingParent {
                        parent: parent.clone(),
                    })?;
            parent_form.subfields.clone()
        } else {
            BTreeMap::new()
        };
        for sub in &form.subfields {
            if merged.contains_key(&sub.name) {
                return Err(LogicFormError::DuplicateSubfield {
                    name: sub.name.clone(),
                });
            }
            merged.insert(sub.name.clone(), sub.clone());
        }
        self.forms
            .insert(form.name.clone(), LogicForm { subfields: merged });
        Ok(())
    }

    pub(crate) fn form(&self, name: &str) -> Option<&LogicForm> {
        self.forms.get(name)
    }
}

#[derive(Debug)]
pub(crate) enum LogicFormError {
    DuplicateForm { name: String },
    MissingSubfields { name: String },
    MissingParent { parent: String },
    DuplicateSubfield { name: String },
}
