//! Callable/subroutine signature metadata focused purely on layout relationships.

use smallvec::SmallVec;

use super::arena::{StringId, TypeId};

#[derive(Clone, Debug, PartialEq)]
pub struct CallableType {
    pub name_id: Option<StringId>,
    pub returns: SmallVec<[TypeId; 2]>,
    pub params: Vec<TypeId>,
}

impl CallableType {
    pub fn new(name_id: Option<StringId>) -> Self {
        Self {
            name_id,
            returns: SmallVec::new(),
            params: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Callable invariants required by pointer + walker modules.
    use super::*;

    #[test]
    fn constructor_initializes_empty_lists() {
        // constructor should leave returns/params empty so builders can push as needed
        let callable = CallableType::new(None);
        assert!(callable.returns.is_empty(), "callable starts without return types");
        assert!(callable.params.is_empty(), "callable starts without parameters");
    }
}
