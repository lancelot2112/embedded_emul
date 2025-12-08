//! Runtime-shaped aggregate helpers backed by the expression VM.

use super::aggregate::{AggregateKind, AggregateType};
use super::arena::{StringId, TypeId};
use super::arena_record::{ArenaSpan, LayoutSize, MemberRecord};
use super::expr::{EvalContext, ExprProgram};

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicField {
    pub label: Option<StringId>,
    pub ty: TypeId,
    pub size_expr: Option<ExprProgram>,
    pub count_expr: Option<ExprProgram>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DynamicAggregate {
    pub name_id: Option<StringId>,
    pub fields: Vec<DynamicField>,
}

impl DynamicAggregate {
    pub fn new(name_id: Option<StringId>) -> Self {
        Self {
            name_id,
            fields: Vec::new(),
        }
    }

    pub fn push_field(&mut self, field: DynamicField) {
        self.fields.push(field);
    }

    pub fn materialize<C: EvalContext>(&self, ctx: &mut C) -> (AggregateType, Vec<MemberRecord>) {
        let mut offset_bits = 0usize;
        let mut members = Vec::with_capacity(self.fields.len());
        for field in &self.fields {
            let byte_size = if let Some(expr) = &field.size_expr {
                expr.clone().evaluate(ctx) as usize
            } else {
                0
            };
            let count = if let Some(expr) = &field.count_expr {
                expr.clone().evaluate(ctx) as usize
            } else {
                1
            };
            let total_bits = byte_size.max(1) * 8 * count;
            let mut member = MemberRecord::new(field.label, field.ty, offset_bits);
            member.bit_size = Some(total_bits);
            members.push(member);
            offset_bits += total_bits;
        }

        let span = ArenaSpan::new(0, members.len());
        let layout = LayoutSize {
            bytes: offset_bits / 8,
            trailing_bits: (offset_bits % 8),
        };
        let mut agg = AggregateType::new(AggregateKind::Struct, span, layout);
        agg.has_dynamic = true;
        (agg, members)
    }
}

#[cfg(test)]
mod tests {
    //! Validates that dynamic aggregates correctly evaluate expressions for layout decisions.
    use super::*;
    use crate::soc::prog::types::arena::{TypeArena, TypeId};
    use crate::soc::prog::types::arena_record::TypeRecord;
    use crate::soc::prog::types::expr::{EvalContext, OpCode};
    use crate::soc::prog::types::scalar::{DisplayFormat, ScalarEncoding, ScalarType};

    struct StaticContext;

    impl EvalContext for StaticContext {
        fn read_member(&mut self, _handle: u32) -> u64 {
            0
        }

        fn read_variable(&mut self, _variable_id: i64) -> u64 {
            0
        }

        fn sizeof(&self, _ty: TypeId) -> u64 {
            4
        }

        fn count_of(&self, _ty: TypeId) -> u64 {
            1
        }

        fn deref(&mut self, value: u64) -> u64 {
            value
        }
    }

    #[test]
    fn dynamic_layout_accumulates_offsets() {
        // ensures dynamic materialization respects evaluated expressions
        let mut arena = TypeArena::new();
        let scalar = ScalarType::new(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Default);
        let scalar_id = arena.push_record(TypeRecord::Scalar(scalar));
        let field = DynamicField {
            label: None,
            ty: scalar_id,
            size_expr: Some({
                let mut expr = ExprProgram::new();
                expr.push(OpCode::PushConst(2));
                expr
            }),
            count_expr: None,
        };
        let mut aggregate = DynamicAggregate::new(None);
        aggregate.push_field(field.clone());
        let mut ctx = StaticContext;
        let (agg, members) = aggregate.materialize(&mut ctx);
        assert!(
            agg.has_dynamic,
            "materialized aggregate should carry dynamic flag"
        );
        assert_eq!(
            members.len(),
            1,
            "single field aggregate produces one member"
        );
    }
}
