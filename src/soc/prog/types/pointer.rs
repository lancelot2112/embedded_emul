//! Pointer, reference, and callable pointer metadata.

use super::arena::TypeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind {
    Data,
    Reference,
    Function,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerQualifiers {
    pub is_const: bool,
    pub is_volatile: bool,
}

impl PointerQualifiers {
    pub const fn new(is_const: bool, is_volatile: bool) -> Self {
        Self { is_const, is_volatile }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressSpace {
    Default,
    Segmented(u16),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PointerType {
    pub target: TypeId,
    pub kind: PointerKind,
    pub qualifiers: PointerQualifiers,
    pub address_space: AddressSpace,
}

impl PointerType {
    pub fn new(target: TypeId, kind: PointerKind) -> Self {
        Self {
            target,
            kind,
            qualifiers: PointerQualifiers::new(false, false),
            address_space: AddressSpace::Default,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Pointer level guarantees that keep walker logic predictable.
    use super::*;

    #[test]
    fn pointer_defaults_to_data_kind() {
        // ensures constructor seeds deterministic defaults used by interpreters
        let ptr = PointerType::new(TypeId::from_index(1), PointerKind::Data);
        assert_eq!(ptr.kind, PointerKind::Data, "constructor should echo requested pointer kind");
        assert!(matches!(ptr.address_space, AddressSpace::Default), "default pointer should not force segmented addressing");
    }
}
