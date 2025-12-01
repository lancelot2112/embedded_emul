//! Expression evaluation helpers shared by the semantics runtime.
//!
//! The evaluator stays focused on pure expression trees produced by
//! `SemanticProgram`. Higher-level constructs such as register or host calls
//! are handled by the runtime once execution plumbing lands.

use crate::soc::isa::error::IsaError;
use crate::soc::isa::semantics::context::ExecutionContext;
use crate::soc::isa::semantics::program::{BitSlice, ContextCall, Expr, ExprBinaryOp, ExprUnaryOp};
use crate::soc::isa::semantics::value::SemanticValue;

/// Resolves `$context::foo()` style expressions when evaluating semantic IR.
pub trait ContextCallResolver {
    fn evaluate_context_call(
        &mut self,
        call: &ContextCall,
        args: Vec<SemanticValue>,
    ) -> Result<SemanticValue, IsaError>;
}

/// Default resolver that simply errors when a context call is encountered.
pub struct NoContextResolver;

impl ContextCallResolver for NoContextResolver {
    fn evaluate_context_call(
        &mut self,
        call: &ContextCall,
        _args: Vec<SemanticValue>,
    ) -> Result<SemanticValue, IsaError> {
        Err(IsaError::Machine(format!(
            "context call '${}::{}' requires runtime dispatch",
            call.space, call.name
        )))
    }
}

/// Stateless evaluator that resolves `Expr` nodes against the current execution
/// context. It clones `SemanticValue`s on demand so callers can keep ownership
/// of the originals stored inside the context map.
pub struct ExpressionEvaluator<'ctx, 'params, R: ContextCallResolver> {
    context: &'ctx ExecutionContext<'params>,
    resolver: R,
}

impl<'ctx, 'params> ExpressionEvaluator<'ctx, 'params, NoContextResolver> {
    pub fn new(context: &'ctx ExecutionContext<'params>) -> Self {
        Self {
            context,
            resolver: NoContextResolver,
        }
    }
}

impl<'ctx, 'params, R> ExpressionEvaluator<'ctx, 'params, R>
where
    R: ContextCallResolver,
{
    pub fn with_resolver(context: &'ctx ExecutionContext<'params>, resolver: R) -> Self {
        Self { context, resolver }
    }

    pub fn evaluate(&mut self, expr: &Expr) -> Result<SemanticValue, IsaError> {
        self.eval(expr)
    }

    fn eval(&mut self, expr: &Expr) -> Result<SemanticValue, IsaError> {
        match expr {
            Expr::Number(value) => Self::literal(*value),
            Expr::Variable { name, .. } => self.lookup_variable(name),
            Expr::Parameter { name, .. } => self.lookup_parameter(name),
            Expr::Call(call) => self.evaluate_call(call),
            Expr::Tuple(items) => self.evaluate_tuple(items),
            Expr::BinaryOp { op, lhs, rhs } => self.evaluate_binary(*op, lhs, rhs),
            Expr::BitSlice { expr, slice } => self.evaluate_bit_slice(expr, slice),
            Expr::UnaryOp { op, expr } => self.evaluate_unary(*op, expr),
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

    fn evaluate_call(&mut self, call: &ContextCall) -> Result<SemanticValue, IsaError> {
        let mut args = Vec::with_capacity(call.args.len());
        for expr in &call.args {
            args.push(self.eval(expr)?);
        }
        self.resolver.evaluate_context_call(call, args)
    }

    fn evaluate_tuple(&mut self, items: &[Expr]) -> Result<SemanticValue, IsaError> {
        let mut values = Vec::with_capacity(items.len());
        for expr in items {
            values.push(self.eval(expr)?);
        }
        Ok(SemanticValue::tuple(values))
    }

    fn evaluate_binary(
        &mut self,
        op: ExprBinaryOp,
        lhs: &Expr,
        rhs: &Expr,
    ) -> Result<SemanticValue, IsaError> {
        match op {
            ExprBinaryOp::LogicalOr => {
                let left = self.eval(lhs)?.as_bool()?;
                if left {
                    Ok(SemanticValue::bool(true))
                } else {
                    let right = self.eval(rhs)?.as_bool()?;
                    Ok(SemanticValue::bool(right))
                }
            }
            ExprBinaryOp::LogicalAnd => {
                let left = self.eval(lhs)?.as_bool()?;
                if !left {
                    Ok(SemanticValue::bool(false))
                } else {
                    let right = self.eval(rhs)?.as_bool()?;
                    Ok(SemanticValue::bool(right))
                }
            }
            ExprBinaryOp::BitOr => self.int_binary(lhs, rhs, |l, r| l | r),
            ExprBinaryOp::BitXor => self.int_binary(lhs, rhs, |l, r| l ^ r),
            ExprBinaryOp::BitAnd => self.int_binary(lhs, rhs, |l, r| l & r),
            ExprBinaryOp::Add => self.int_binary(lhs, rhs, |l, r| l.wrapping_add(r)),
            ExprBinaryOp::Sub => self.int_binary(lhs, rhs, |l, r| l.wrapping_sub(r)),
            ExprBinaryOp::Eq => {
                let left = self.eval(lhs)?;
                let right = self.eval(rhs)?;
                let result = match (&left, &right) {
                    (SemanticValue::Bool(a), SemanticValue::Bool(b)) => *a == *b,
                    _ => left.as_int()? == right.as_int()?,
                };
                Ok(SemanticValue::bool(result))
            }
            ExprBinaryOp::Ne => {
                let left = self.eval(lhs)?;
                let right = self.eval(rhs)?;
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

    fn int_binary<F>(&mut self, lhs: &Expr, rhs: &Expr, op: F) -> Result<SemanticValue, IsaError>
    where
        F: FnOnce(i64, i64) -> i64,
    {
        let left = self.eval(lhs)?.as_int()?;
        let right = self.eval(rhs)?.as_int()?;
        Ok(SemanticValue::int(op(left, right)))
    }

    fn int_compare<F>(&mut self, lhs: &Expr, rhs: &Expr, cmp: F) -> Result<SemanticValue, IsaError>
    where
        F: FnOnce(i64, i64) -> bool,
    {
        let left = self.eval(lhs)?.as_int()?;
        let right = self.eval(rhs)?.as_int()?;
        Ok(SemanticValue::bool(cmp(left, right)))
    }

    fn evaluate_bit_slice(
        &mut self,
        expr: &Expr,
        slice: &BitSlice,
    ) -> Result<SemanticValue, IsaError> {
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
        let value = self.eval(expr)?.as_int()? as u64;
        let width = slice.end - slice.start + 1;
        let mask = mask_for_bits(width);
        let sliced = (value >> slice.start) & mask;
        Ok(SemanticValue::int(sliced as i64))
    }

    fn evaluate_unary(&mut self, op: ExprUnaryOp, expr: &Expr) -> Result<SemanticValue, IsaError> {
        match op {
            ExprUnaryOp::LogicalNot => {
                let value = self.eval(expr)?.as_bool()?;
                Ok(SemanticValue::bool(!value))
            }
        }
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
    use std::path::PathBuf;

    use crate::soc::isa::diagnostic::{SourcePosition, SourceSpan};

    fn test_span() -> SourceSpan {
        SourceSpan::point(PathBuf::from("<expr>"), SourcePosition::new(1, 1))
    }

    #[test]
    fn evaluates_literal_numbers() {
        let params = HashMap::new();
        let ctx = ExecutionContext::new(&params);
        let mut evaluator = ExpressionEvaluator::new(&ctx);
        let expr = Expr::Number(42);
        let value = evaluator.evaluate(&expr).expect("literal eval");
        assert_eq!(value.as_int().unwrap(), 42);
    }

    #[test]
    fn resolves_variables_from_context() {
        let mut params = HashMap::new();
        params.insert("acc".into(), SemanticValue::int(10));
        let ctx = ExecutionContext::new(&params);
        let mut evaluator = ExpressionEvaluator::new(&ctx);
        let expr = Expr::Variable {
            name: "acc".into(),
            span: test_span(),
        };
        let value = evaluator.evaluate(&expr).expect("variable eval");
        assert_eq!(value.as_int().unwrap(), 10);
    }

    #[test]
    fn logical_ops_short_circuit() {
        let mut params = HashMap::new();
        params.insert("truthy".into(), SemanticValue::bool(true));
        params.insert("falsy".into(), SemanticValue::bool(false));
        let ctx = ExecutionContext::new(&params);
        let mut evaluator = ExpressionEvaluator::new(&ctx);
        let or_expr = Expr::BinaryOp {
            op: ExprBinaryOp::LogicalOr,
            lhs: Box::new(Expr::Variable {
                name: "truthy".into(),
                span: test_span(),
            }),
            rhs: Box::new(Expr::Variable {
                name: "missing".into(),
                span: test_span(),
            }),
        };
        let or_value = evaluator.evaluate(&or_expr).expect("logical or");
        assert!(or_value.as_bool().unwrap());

        let and_expr = Expr::BinaryOp {
            op: ExprBinaryOp::LogicalAnd,
            lhs: Box::new(Expr::Variable {
                name: "falsy".into(),
                span: test_span(),
            }),
            rhs: Box::new(Expr::Variable {
                name: "missing".into(),
                span: test_span(),
            }),
        };
        let and_value = evaluator.evaluate(&and_expr).expect("logical and");
        assert!(!and_value.as_bool().unwrap());
    }

    #[test]
    fn applies_bit_slices() {
        let params = HashMap::new();
        let ctx = ExecutionContext::new(&params);
        let mut evaluator = ExpressionEvaluator::new(&ctx);
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
        let mut evaluator = ExpressionEvaluator::new(&ctx);
        let expr = Expr::Call(crate::soc::isa::semantics::program::ContextCall {
            kind: crate::soc::isa::semantics::program::ContextKind::Register,
            space: "reg".into(),
            name: "ACC".into(),
            subpath: Vec::new(),
            args: Vec::new(),
            span: test_span(),
        });
        let err = evaluator.evaluate(&expr).expect_err("call should error");
        assert!(matches!(err, IsaError::Machine(msg) if msg.contains("requires runtime dispatch")));
    }
}
