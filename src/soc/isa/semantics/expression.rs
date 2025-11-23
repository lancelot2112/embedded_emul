//! Expression evaluation helpers shared by the semantics runtime.
//!
//! The evaluator stays focused on pure expression trees produced by
//! `SemanticProgram`. Higher-level constructs such as register or host calls
//! are handled by the runtime once execution plumbing lands.

use crate::soc::isa::error::IsaError;
use crate::soc::isa::semantics::program::{BitSlice, Expr, ExprBinaryOp};
use crate::soc::isa::semantics::runtime::{ExecutionContext, SemanticValue};

/// Stateless evaluator that resolves `Expr` nodes against the current execution
/// context. It clones `SemanticValue`s on demand so callers can keep ownership
/// of the originals stored inside the context map.
pub struct ExpressionEvaluator<'ctx, 'params> {
    context: &'ctx ExecutionContext<'params>,
}

impl<'ctx, 'params> ExpressionEvaluator<'ctx, 'params> {
    pub fn new(context: &'ctx ExecutionContext<'params>) -> Self {
        Self { context }
    }

    pub fn evaluate(&self, expr: &Expr) -> Result<SemanticValue, IsaError> {
        match expr {
            Expr::Number(value) => Self::literal(*value),
            Expr::Variable(name) => self.lookup_variable(name),
            Expr::Parameter(name) => self.lookup_parameter(name),
            Expr::Call(call) => Err(IsaError::Machine(format!(
                "context call '${}::{}' requires runtime dispatch",
                call.space, call.name
            ))),
            Expr::Tuple(items) => self.evaluate_tuple(items),
            Expr::BinaryOp { op, lhs, rhs } => self.evaluate_binary(*op, lhs, rhs),
            Expr::BitSlice { expr, slice } => self.evaluate_bit_slice(expr, slice),
        }
    }

    fn lookup_variable(&self, name: &str) -> Result<SemanticValue, IsaError> {
        self.context
            .get(name)
            .cloned()
            .ok_or_else(|| IsaError::Machine(format!("unknown variable '{name}'")))
    }

    fn lookup_parameter(&self, name: &str) -> Result<SemanticValue, IsaError> {
        self.context
            .get(name)
            .cloned()
            .ok_or_else(|| IsaError::Machine(format!("unknown parameter '#{name}'")))
    }

    fn literal(value: u64) -> Result<SemanticValue, IsaError> {
        let signed = i64::try_from(value).map_err(|_| {
            IsaError::Machine(format!("literal value {value} exceeds 64-bit signed range"))
        })?;
        Ok(SemanticValue::int(signed))
    }

    fn evaluate_tuple(&self, items: &[Expr]) -> Result<SemanticValue, IsaError> {
        let mut values = Vec::with_capacity(items.len());
        for expr in items {
            values.push(self.evaluate(expr)?);
        }
        Ok(SemanticValue::tuple(values))
    }

    fn evaluate_binary(
        &self,
        op: ExprBinaryOp,
        lhs: &Expr,
        rhs: &Expr,
    ) -> Result<SemanticValue, IsaError> {
        match op {
            ExprBinaryOp::LogicalOr => {
                let left = self.evaluate(lhs)?.as_bool()?;
                if left {
                    Ok(SemanticValue::bool(true))
                } else {
                    let right = self.evaluate(rhs)?.as_bool()?;
                    Ok(SemanticValue::bool(right))
                }
            }
            ExprBinaryOp::LogicalAnd => {
                let left = self.evaluate(lhs)?.as_bool()?;
                if !left {
                    Ok(SemanticValue::bool(false))
                } else {
                    let right = self.evaluate(rhs)?.as_bool()?;
                    Ok(SemanticValue::bool(right))
                }
            }
            ExprBinaryOp::BitOr => self.int_binary(lhs, rhs, |l, r| l | r),
            ExprBinaryOp::BitXor => self.int_binary(lhs, rhs, |l, r| l ^ r),
            ExprBinaryOp::BitAnd => self.int_binary(lhs, rhs, |l, r| l & r),
            ExprBinaryOp::Add => self.int_binary(lhs, rhs, |l, r| l.wrapping_add(r)),
            ExprBinaryOp::Sub => self.int_binary(lhs, rhs, |l, r| l.wrapping_sub(r)),
            ExprBinaryOp::Eq => {
                let left = self.evaluate(lhs)?;
                let right = self.evaluate(rhs)?;
                let result = match (&left, &right) {
                    (SemanticValue::Bool(a), SemanticValue::Bool(b)) => *a == *b,
                    _ => left.as_int()? == right.as_int()?,
                };
                Ok(SemanticValue::bool(result))
            }
            ExprBinaryOp::Ne => {
                let left = self.evaluate(lhs)?;
                let right = self.evaluate(rhs)?;
                let result = match (&left, &right) {
                    (SemanticValue::Bool(a), SemanticValue::Bool(b)) => *a != *b,
                    _ => left.as_int()? != right.as_int()?,
                };
                Ok(SemanticValue::bool(result))
            }
            ExprBinaryOp::Lt => self.int_compare(lhs, rhs, |l, r| l < r),
            ExprBinaryOp::Gt => self.int_compare(lhs, rhs, |l, r| l > r),
        }
    }

    fn int_binary<F>(&self, lhs: &Expr, rhs: &Expr, op: F) -> Result<SemanticValue, IsaError>
    where
        F: FnOnce(i64, i64) -> i64,
    {
        let left = self.evaluate(lhs)?.as_int()?;
        let right = self.evaluate(rhs)?.as_int()?;
        Ok(SemanticValue::int(op(left, right)))
    }

    fn int_compare<F>(&self, lhs: &Expr, rhs: &Expr, cmp: F) -> Result<SemanticValue, IsaError>
    where
        F: FnOnce(i64, i64) -> bool,
    {
        let left = self.evaluate(lhs)?.as_int()?;
        let right = self.evaluate(rhs)?.as_int()?;
        Ok(SemanticValue::bool(cmp(left, right)))
    }

    fn evaluate_bit_slice(&self, expr: &Expr, slice: &BitSlice) -> Result<SemanticValue, IsaError> {
        if slice.end < slice.start {
            return Err(IsaError::Machine(format!(
                "bit slice end {} precedes start {}",
                slice.end, slice.start
            )));
        }
        if slice.end >= 64 {
            return Err(IsaError::Machine(format!(
                "bit slice @({}..{}) exceeds 64-bit width",
                slice.start, slice.end
            )));
        }
        let value = self.evaluate(expr)?.as_int()? as u64;
        let width = slice.end - slice.start + 1;
        let mask = mask_for_bits(width);
        let sliced = (value >> slice.start) & mask;
        Ok(SemanticValue::int(sliced as i64))
    }
}

fn mask_for_bits(width: u32) -> u64 {
    if width >= 64 {
        u64::MAX
    } else if width == 0 {
        0
    } else {
        (1u64 << width) - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn evaluates_literal_numbers() {
        let params = HashMap::new();
        let ctx = ExecutionContext::new(&params);
        let evaluator = ExpressionEvaluator::new(&ctx);
        let expr = Expr::Number(42);
        let value = evaluator.evaluate(&expr).expect("literal eval");
        assert_eq!(value.as_int().unwrap(), 42);
    }

    #[test]
    fn resolves_variables_from_context() {
        let mut params = HashMap::new();
        params.insert("acc".into(), SemanticValue::int(10));
        let ctx = ExecutionContext::new(&params);
        let evaluator = ExpressionEvaluator::new(&ctx);
        let expr = Expr::Variable("acc".into());
        let value = evaluator.evaluate(&expr).expect("variable eval");
        assert_eq!(value.as_int().unwrap(), 10);
    }

    #[test]
    fn logical_ops_short_circuit() {
        let mut params = HashMap::new();
        params.insert("truthy".into(), SemanticValue::bool(true));
        params.insert("falsy".into(), SemanticValue::bool(false));
        let ctx = ExecutionContext::new(&params);
        let evaluator = ExpressionEvaluator::new(&ctx);
        let or_expr = Expr::BinaryOp {
            op: ExprBinaryOp::LogicalOr,
            lhs: Box::new(Expr::Variable("truthy".into())),
            rhs: Box::new(Expr::Variable("missing".into())),
        };
        let or_value = evaluator.evaluate(&or_expr).expect("logical or");
        assert!(or_value.as_bool().unwrap());

        let and_expr = Expr::BinaryOp {
            op: ExprBinaryOp::LogicalAnd,
            lhs: Box::new(Expr::Variable("falsy".into())),
            rhs: Box::new(Expr::Variable("missing".into())),
        };
        let and_value = evaluator.evaluate(&and_expr).expect("logical and");
        assert!(!and_value.as_bool().unwrap());
    }

    #[test]
    fn applies_bit_slices() {
        let params = HashMap::new();
        let ctx = ExecutionContext::new(&params);
        let evaluator = ExpressionEvaluator::new(&ctx);
        let expr = Expr::BitSlice {
            expr: Box::new(Expr::Number(0b110110)),
            slice: BitSlice { start: 1, end: 3 },
        };
        let value = evaluator.evaluate(&expr).expect("slice eval");
        assert_eq!(value.as_int().unwrap(), 0b011);
    }

    #[test]
    fn call_nodes_report_missing_dispatch() {
        let params = HashMap::new();
        let ctx = ExecutionContext::new(&params);
        let evaluator = ExpressionEvaluator::new(&ctx);
        let expr = Expr::Call(crate::soc::isa::semantics::program::ContextCall {
            kind: crate::soc::isa::semantics::program::ContextKind::Register,
            space: "reg".into(),
            name: "ACC".into(),
            subpath: Vec::new(),
            args: Vec::new(),
        });
        let err = evaluator.evaluate(&expr).expect_err("call should error");
        assert!(
            matches!(err, IsaError::Machine(msg) if msg.contains("requires runtime dispatch"))
        );
    }
}
