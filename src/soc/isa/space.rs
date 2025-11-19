use std::collections::{HashMap, HashSet};

use super::ast::{ContextReference, FieldDecl};
use super::register::{FieldInfo, FieldLookup, FieldRegistrationError, RangedFieldInfo};

#[derive(Default)]
pub(crate) struct SpaceState {
    fields: HashMap<String, FieldInfo>,
    ranges: Vec<RangedFieldInfo>,
}

impl SpaceState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn lookup_field(&self, name: &str) -> Option<FieldLookup<'_>> {
        if let Some(info) = self.fields.get(name) {
            return Some(FieldLookup::Direct(info));
        }
        for entry in &self.ranges {
            if entry.matches(name) {
                return Some(FieldLookup::Ranged(entry));
            }
        }
        None
    }

    pub(crate) fn register_field(&mut self, field: &FieldDecl) -> Result<(), FieldRegistrationError> {
        let subfields: HashSet<String> = field.subfields.iter().map(|sub| sub.name.clone()).collect();
        if let Some(range) = &field.range {
            if self.ranges.iter().any(|entry| entry.base == field.name) {
                return Err(FieldRegistrationError::DuplicateField);
            }
            self.ranges.push(RangedFieldInfo::new(
                field.name.clone(),
                range.start,
                range.end,
                subfields,
            ));
        } else {
            if self.fields.contains_key(&field.name) {
                return Err(FieldRegistrationError::DuplicateField);
            }
            self.fields
                .insert(field.name.clone(), FieldInfo::new(subfields));
        }
        Ok(())
    }
}

pub(crate) fn resolve_reference_path(
    current_space: &str,
    reference: &ContextReference,
) -> (String, Vec<String>) {
    if let Some(first) = reference.segments.first() {
        if first.starts_with('$') {
            let space = first.trim_start_matches('$').to_string();
            let rest = reference.segments[1..].to_vec();
            return (space, rest);
        }
    }
    (current_space.to_string(), reference.segments.clone())
}
