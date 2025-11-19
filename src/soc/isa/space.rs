use std::collections::{HashMap, HashSet};

use super::ast::{ContextReference, FieldDecl, FieldIndexRange};
use super::register::{FieldInfo, FieldRegistrationError};

#[derive(Default)]
pub(crate) struct SpaceState {
    fields: HashMap<String, FieldInfo>,
}

impl SpaceState {
    pub(crate) fn lookup_field(&self, name: &str) -> Option<&FieldInfo> {
        self.fields.get(name)
    }

    pub(crate) fn register_field(
        &mut self,
        field: &FieldDecl,
    ) -> Result<(), FieldRegistrationError> {
        let append_only = is_subfield_append(field);
        let targets = expand_field_names(field);
        if append_only && field.subfields.is_empty() {
            return Err(FieldRegistrationError::EmptySubfieldAppend {
                name: field.name.clone(),
            });
        }

        for target in targets {
            if append_only {
                let info = self.fields.get_mut(&target).ok_or_else(|| {
                    FieldRegistrationError::MissingBaseField {
                        name: target.clone(),
                    }
                })?;
                info.merge_subfields(field.subfields.iter().map(|sub| sub.name.clone()));
            } else {
                if self.fields.contains_key(&target) {
                    return Err(FieldRegistrationError::DuplicateField { name: target });
                }
                let subfields: HashSet<String> =
                    field.subfields.iter().map(|sub| sub.name.clone()).collect();
                self.fields.insert(target, FieldInfo::new(subfields));
            }
        }

        Ok(())
    }
}

pub(crate) fn resolve_reference_path(
    current_space: &str,
    reference: &ContextReference,
) -> (String, Vec<String>) {
    if let Some(first) = reference.segments.first()
        && first.starts_with('$')
    {
        let space = first.trim_start_matches('$').to_string();
        let rest = reference.segments[1..].to_vec();
        return (space, rest);
    }
    (current_space.to_string(), reference.segments.clone())
}

fn expand_field_names(field: &FieldDecl) -> Vec<String> {
    if let Some(FieldIndexRange { start, end }) = &field.range {
        let mut names = Vec::new();
        for index in *start..=*end {
            names.push(format!("{}{}", field.name, index));
        }
        names
    } else {
        vec![field.name.clone()]
    }
}

fn is_subfield_append(field: &FieldDecl) -> bool {
    let structural_present = field.offset.is_some()
        || field.size.is_some()
        || field.reset.is_some()
        || field.description.is_some()
        || field.redirect.is_some();
    !structural_present
}
