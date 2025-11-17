//! Sequence, slice, and vector metadata including dynamic count expressions.

use smallvec::SmallVec;

use super::arena::{StringId, TypeId};
use super::expr::ExprProgram;

#[derive(Clone, Debug, PartialEq)]
pub struct SequenceType {
    pub element: TypeId,
    pub stride_bytes: u32,
    pub count: SequenceCount,
}

impl SequenceType {
    pub fn new(element: TypeId, stride_bytes: u32, count: SequenceCount) -> Self {
        Self {
            element,
            stride_bytes,
            count,
        }
    }

    pub fn element_count(&self) -> Option<u32> {
        match &self.count {
            SequenceCount::Static(value) => Some(*value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SequenceCount {
    Static(u32),
    Dynamic(CountSource),
}

#[derive(Clone, Debug, PartialEq)]
pub enum CountSource {
    Expression(Box<ExprProgram>),
    MemberPath(SmallVec<[StringId; 4]>),
}

#[cfg(test)]
mod tests {
    //! Sequence specific invariants to guard against accidental regressions.
    use super::*;
    use crate::soc::prog::types::arena::TypeId;
    use crate::soc::prog::types::expr::ExprProgram;

    #[test]
    fn static_counts_are_reported() {
        // ensures SequenceType::element_count returns the static literal when available
        let sequence = SequenceType::new(TypeId::from_index(0), 4, SequenceCount::Static(5));
        assert_eq!(sequence.element_count(), Some(5), "static multiplier should be extracted successfully");
    }

    #[test]
    fn dynamic_counts_wrap_expression() {
        // verifying that CountSource::Expression stores the provided program as-is
        let program = ExprProgram::new();
        let sequence = SequenceType::new(
            TypeId::from_index(0),
            4,
            SequenceCount::Dynamic(CountSource::Expression(Box::new(program.clone()))),
        );
        match sequence.count {
            SequenceCount::Dynamic(CountSource::Expression(_)) => {}
            _ => panic!("expression-based count should match constructor"),
        }
    }
}
