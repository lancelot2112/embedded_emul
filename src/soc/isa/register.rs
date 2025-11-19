use std::collections::HashSet;

use crate::soc::prog::types::parse_index_suffix;

#[derive(Debug, Default)]
pub(crate) struct FieldInfo {
    pub(crate) subfields: HashSet<String>,
}

impl FieldInfo {
    pub(crate) fn new(subfields: HashSet<String>) -> Self {
        Self { subfields }
    }

    pub(crate) fn has_subfield(&self, name: &str) -> bool {
        self.subfields.contains(name)
    }
}

#[derive(Debug, Default)]
pub(crate) struct RangedFieldInfo {
    pub(crate) base: String,
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) subfields: HashSet<String>,
}

impl RangedFieldInfo {
    pub(crate) fn new(base: String, start: u32, end: u32, subfields: HashSet<String>) -> Self {
        Self {
            base,
            start,
            end,
            subfields,
        }
    }

    pub(crate) fn matches(&self, candidate: &str) -> bool {
        if !candidate.starts_with(&self.base) {
            return false;
        }
        let suffix = &candidate[self.base.len()..];
        if suffix.is_empty() {
            return false;
        }
        match parse_index_suffix(suffix) {
            Ok(index) => index >= self.start && index <= self.end,
            Err(_) => false,
        }
    }

    pub(crate) fn has_subfield(&self, name: &str) -> bool {
        self.subfields.contains(name)
    }
}

#[derive(Debug)]
pub(crate) enum FieldRegistrationError {
    DuplicateField,
}

pub(crate) enum FieldLookup<'a> {
    Direct(&'a FieldInfo),
    Ranged(&'a RangedFieldInfo),
}

impl<'a> FieldLookup<'a> {
    pub(crate) fn has_subfield(&self, name: &str) -> bool {
        match self {
            FieldLookup::Direct(info) => info.has_subfield(name),
            FieldLookup::Ranged(info) => info.has_subfield(name),
        }
    }
}
