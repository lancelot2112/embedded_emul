//! Compact expression bytecode used by dynamic layout evaluation.

use smallvec::SmallVec;

use super::arena::TypeId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpCode {
    PushConst(u64),
    ReadMember(u32),
    ReadVariable(i64),
    SizeOf(TypeId),
    CountOf(TypeId),
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Deref,
}

pub trait EvalContext {
    fn read_member(&mut self, handle: u32) -> u64;
    fn read_variable(&mut self, variable_id: i64) -> u64;
    fn sizeof(&self, ty: TypeId) -> u64;
    fn count_of(&self, ty: TypeId) -> u64;
    fn deref(&mut self, value: u64) -> u64;
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExprProgram {
    ops: SmallVec<[OpCode; 8]>,
}

impl ExprProgram {
    pub fn new() -> Self {
        Self {
            ops: SmallVec::new(),
        }
    }

    pub fn push(&mut self, op: OpCode) {
        self.ops.push(op);
    }

    pub fn evaluate<C: EvalContext>(&self, ctx: &mut C) -> u64 {
        let mut stack: SmallVec<[u64; 8]> = SmallVec::new();
        for op in &self.ops {
            match *op {
                OpCode::PushConst(value) => stack.push(value),
                OpCode::ReadMember(handle) => stack.push(ctx.read_member(handle)),
                OpCode::ReadVariable(id) => stack.push(ctx.read_variable(id)),
                OpCode::SizeOf(ty) => stack.push(ctx.sizeof(ty)),
                OpCode::CountOf(ty) => stack.push(ctx.count_of(ty)),
                OpCode::Add => apply_binary(&mut stack, |a, b| a + b),
                OpCode::Sub => apply_binary(&mut stack, |a, b| a.wrapping_sub(b)),
                OpCode::Mul => apply_binary(&mut stack, |a, b| a * b),
                OpCode::Div => apply_binary(&mut stack, |a, b| if b == 0 { 0 } else { a / b }),
                OpCode::Neg => {
                    if let Some(val) = stack.pop() {
                        stack.push(val.wrapping_neg());
                    }
                }
                OpCode::Deref => {
                    if let Some(addr) = stack.pop() {
                        stack.push(ctx.deref(addr));
                    }
                }
            }
        }
        stack.pop().unwrap_or(0)
    }
}

fn apply_binary<F>(stack: &mut SmallVec<[u64; 8]>, func: F)
where
    F: Fn(u64, u64) -> u64,
{
    if stack.len() >= 2 {
        let rhs = stack.pop().unwrap();
        let lhs = stack.pop().unwrap();
        stack.push(func(lhs, rhs));
    }
}

#[cfg(test)]
mod tests {
    //! Exercises the tiny expression VM to guarantee deterministic evaluation semantics.
    use super::*;

    struct MockContext;

    impl EvalContext for MockContext {
        fn read_member(&mut self, handle: u32) -> u64 {
            handle as u64
        }

        fn read_variable(&mut self, variable_id: i64) -> u64 {
            variable_id as u64
        }

        fn sizeof(&self, _ty: TypeId) -> u64 {
            4
        }

        fn count_of(&self, _ty: TypeId) -> u64 {
            2
        }

        fn deref(&mut self, value: u64) -> u64 {
            value + 1
        }
    }

    #[test]
    fn program_executes_stack_ops() {
        // stack-based evaluation should honor operator order with left-to-right execution
        let mut program = ExprProgram::new();
        program.push(OpCode::PushConst(4));
        program.push(OpCode::PushConst(1));
        program.push(OpCode::Add);
        program.push(OpCode::PushConst(2));
        program.push(OpCode::Mul);
        let mut ctx = MockContext;
        let value = program.evaluate(&mut ctx);
        assert_eq!(value, 10, "(4 + 1) * 2 should equal ten");
    }
}
