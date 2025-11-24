//! Core runtime primitives for the semantics interpreter.
//!
//! The value model, execution context, and register helpers now live in their
//! own modules so this file can focus on orchestrating evaluation.

use std::cell::Cell;
use std::collections::HashMap;

use crate::soc::core::state::CoreState;
use crate::soc::isa::error::IsaError;
use crate::soc::isa::machine::{HostServices, Instruction, MachineDescription};
use crate::soc::isa::semantics::context::ExecutionContext;
use crate::soc::isa::semantics::expression::{ContextCallResolver, ExpressionEvaluator};
use crate::soc::isa::semantics::program::{
    AssignTarget, ContextCall, ContextKind, Expr, RegisterRef, SemanticProgram, SemanticStmt,
};
use crate::soc::isa::semantics::register::RegisterAccess;
use crate::soc::isa::semantics::value::SemanticValue;

#[derive(Debug, Default)]
pub struct SemanticRuntime;

const MAX_CALL_DEPTH: usize = 32;

impl SemanticRuntime {
    pub fn new() -> Self {
        Self
    }

    /// Provides access to register helpers bound to the supplied machine description.
    pub fn register_access<'machine>(
        &'machine self,
        machine: &'machine MachineDescription,
    ) -> RegisterAccess<'machine> {
        RegisterAccess::new(machine)
    }

    /// Evaluates a semantic expression using the provided execution context and core state.
    fn evaluate_expression<'ctx>(
        &self,
        machine: &MachineDescription,
        state: &mut CoreState,
        host: &mut dyn HostServices,
        stack: &CallStack,
        context: &ExecutionContext<'ctx>,
        expr: &Expr,
    ) -> Result<SemanticValue, IsaError> {
        let registers = self.register_access(machine);
        let resolver = RuntimeCallResolver::new(self, machine, state, host, stack, registers);
        let mut evaluator = ExpressionEvaluator::with_resolver(context, resolver);
        evaluator.evaluate(expr)
    }

    /// Executes a semantic program and returns the first value produced by a `return` statement.
    pub fn execute_program(
        &self,
        machine: &MachineDescription,
        state: &mut CoreState,
        host: &mut dyn HostServices,
        params: &HashMap<String, SemanticValue>,
        program: &SemanticProgram,
    ) -> Result<Option<SemanticValue>, IsaError> {
        let mut context = ExecutionContext::new(params);
        let stack = CallStack::new(MAX_CALL_DEPTH);
        self.execute_with_context(machine, state, host, &stack, &mut context, program)
    }

    fn execute_with_context<'ctx>(
        &self,
        machine: &MachineDescription,
        state: &mut CoreState,
        host: &mut dyn HostServices,
        stack: &CallStack,
        context: &mut ExecutionContext<'ctx>,
        program: &SemanticProgram,
    ) -> Result<Option<SemanticValue>, IsaError> {
        let _frame = stack.enter()?;
        for stmt in &program.statements {
            if let Some(value) =
                self.execute_statement(machine, state, host, stack, context, stmt)?
            {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }

    fn execute_nested_program(
        &self,
        machine: &MachineDescription,
        state: &mut CoreState,
        host: &mut dyn HostServices,
        stack: &CallStack,
        params: HashMap<String, SemanticValue>,
        program: &SemanticProgram,
    ) -> Result<Option<SemanticValue>, IsaError> {
        let bound_params = params;
        let mut context = ExecutionContext::new(&bound_params);
        self.execute_with_context(machine, state, host, stack, &mut context, program)
    }

    fn execute_statement<'ctx>(
        &self,
        machine: &MachineDescription,
        state: &mut CoreState,
        host: &mut dyn HostServices,
        stack: &CallStack,
        context: &mut ExecutionContext<'ctx>,
        stmt: &SemanticStmt,
    ) -> Result<Option<SemanticValue>, IsaError> {
        match stmt {
            SemanticStmt::Assign { target, expr } => {
                let value = self.evaluate_expression(machine, state, host, stack, context, expr)?;
                self.assign_target(machine, state, host, stack, context, target, value)?;
                Ok(None)
            }
            SemanticStmt::Expr(expr) => {
                let _ = self.evaluate_expression(machine, state, host, stack, context, expr)?;
                Ok(None)
            }
            SemanticStmt::Return(expr) => {
                let value = self.evaluate_expression(machine, state, host, stack, context, expr)?;
                Ok(Some(value))
            }
        }
    }

    fn assign_target<'ctx>(
        &self,
        machine: &MachineDescription,
        state: &mut CoreState,
        host: &mut dyn HostServices,
        stack: &CallStack,
        context: &mut ExecutionContext<'ctx>,
        target: &AssignTarget,
        value: SemanticValue,
    ) -> Result<(), IsaError> {
        match target {
            AssignTarget::Variable(name) => {
                context.set_local(name.clone(), value);
                Ok(())
            }
            AssignTarget::Tuple(names) => {
                let tuple = value.try_into_tuple()?;
                tuple.ensure_len(names.len())?;
                for (name, element) in names.iter().zip(tuple.into_vec()) {
                    context.set_local(name.clone(), element);
                }
                Ok(())
            }
            AssignTarget::Register(reference) => {
                self.write_register_target(machine, state, host, stack, context, reference, value)
            }
        }
    }

    fn write_register_target<'ctx>(
        &self,
        machine: &MachineDescription,
        state: &mut CoreState,
        host: &mut dyn HostServices,
        stack: &CallStack,
        context: &ExecutionContext<'ctx>,
        reference: &RegisterRef,
        value: SemanticValue,
    ) -> Result<(), IsaError> {
        let index =
            self.evaluate_register_index(machine, state, host, stack, context, reference)?;
        let registers = self.register_access(machine);
        let resolved = registers.resolve(reference, index)?;
        resolved.write(state, value.as_int()?)
    }

    fn evaluate_register_index<'ctx>(
        &self,
        machine: &MachineDescription,
        state: &mut CoreState,
        host: &mut dyn HostServices,
        stack: &CallStack,
        context: &ExecutionContext<'ctx>,
        reference: &RegisterRef,
    ) -> Result<Option<i64>, IsaError> {
        if let Some(expr) = &reference.index {
            let value = self.evaluate_expression(machine, state, host, stack, context, expr)?;
            Ok(Some(value.as_int()?))
        } else {
            Ok(None)
        }
    }
}

struct RuntimeCallResolver<'runtime, 'machine, 'state, 'host, 'stack> {
    runtime: &'runtime SemanticRuntime,
    machine: &'machine MachineDescription,
    registers: RegisterAccess<'machine>,
    state: &'state mut CoreState,
    host: &'host mut dyn HostServices,
    stack: &'stack CallStack,
}

struct CallStack {
    depth: Cell<usize>,
    limit: usize,
}

impl CallStack {
    fn new(limit: usize) -> Self {
        Self {
            depth: Cell::new(0),
            limit,
        }
    }

    fn enter(&self) -> Result<CallStackGuard<'_>, IsaError> {
        let current = self.depth.get();
        if current >= self.limit {
            return Err(IsaError::Machine(format!(
                "semantic call stack exceeded limit of {} frames",
                self.limit
            )));
        }
        self.depth.set(current + 1);
        Ok(CallStackGuard { stack: self })
    }
}

struct CallStackGuard<'stack> {
    stack: &'stack CallStack,
}

impl<'stack> Drop for CallStackGuard<'stack> {
    fn drop(&mut self) {
        let current = self.stack.depth.get();
        self.stack.depth.set(current.saturating_sub(1));
    }
}

impl<'runtime, 'machine, 'state, 'host, 'stack>
    RuntimeCallResolver<'runtime, 'machine, 'state, 'host, 'stack>
{
    fn new(
        runtime: &'runtime SemanticRuntime,
        machine: &'machine MachineDescription,
        state: &'state mut CoreState,
        host: &'host mut dyn HostServices,
        stack: &'stack CallStack,
        registers: RegisterAccess<'machine>,
    ) -> Self {
        Self {
            runtime,
            machine,
            registers,
            state,
            host,
            stack,
        }
    }

    fn evaluate_register_call(
        &mut self,
        call: &ContextCall,
        args: Vec<SemanticValue>,
    ) -> Result<SemanticValue, IsaError> {
        if args.len() > 1 {
            return Err(IsaError::Machine(format!(
                "register call '${}::{}' accepts at most one argument",
                call.space, call.name
            )));
        }
        if call.subpath.len() > 1 {
            return Err(IsaError::Machine(format!(
                "register call '${}::{}' cannot reference nested subfields",
                call.space, call.name
            )));
        }
        let index = match args.first() {
            Some(value) => Some(value.as_int()?),
            None => None,
        };
        let reference = RegisterRef {
            space: call.space.clone(),
            name: call.name.clone(),
            subfield: call.subpath.first().cloned(),
            index: None,
        };
        let resolved = self.registers.resolve(&reference, index)?;
        resolved.read(self.state)
    }

    fn evaluate_macro_call(
        &mut self,
        call: &ContextCall,
        args: Vec<SemanticValue>,
    ) -> Result<SemanticValue, IsaError> {
        if !call.subpath.is_empty() {
            return Err(IsaError::Machine(format!(
                "macro call '${}::{}' does not support subpaths",
                call.space, call.name
            )));
        }
        let (parameters, program) = {
            let info = self
                .machine
                .macros
                .iter()
                .find(|mac| mac.name == call.name)
                .ok_or_else(|| {
                    IsaError::Machine(format!("unknown macro '${}::{}'", call.space, call.name))
                })?;
            let program = info.semantics.ensure_program()?.clone();
            (info.parameters.clone(), program)
        };
        let params = self.bind_arguments(&parameters, args, call)?;
        self.invoke_program(params, program.as_ref())
    }

    fn evaluate_instruction_call(
        &mut self,
        call: &ContextCall,
        args: Vec<SemanticValue>,
    ) -> Result<SemanticValue, IsaError> {
        if !call.subpath.is_empty() {
            return Err(IsaError::Machine(format!(
                "instruction call '${}::{}' does not support subpaths",
                call.space, call.name
            )));
        }
        let (operands, program) = {
            let instruction = self.find_instruction(call)?;
            let operands = self.instruction_operands(instruction)?;
            let block = instruction.semantics.as_ref().ok_or_else(|| {
                IsaError::Machine(format!(
                    "instruction '${}::{}' is missing semantics",
                    instruction.space, instruction.name
                ))
            })?;
            let program = block.ensure_program()?.clone();
            (operands, program)
        };
        let params = self.bind_arguments(&operands, args, call)?;
        self.invoke_program(params, program.as_ref())
    }

    fn evaluate_host_call(
        &mut self,
        call: &ContextCall,
        args: Vec<SemanticValue>,
    ) -> Result<SemanticValue, IsaError> {
        if !call.subpath.is_empty() {
            return Err(IsaError::Machine(format!(
                "host call '${}::{}' does not support subpaths",
                call.space, call.name
            )));
        }
        match call.name.as_str() {
            "add" => self.host_add(args, call),
            "add_with_carry" => self.host_add_with_carry(args, call),
            "sub" => self.host_sub(args, call),
            "mul" => self.host_mul(args, call),
            other => Err(IsaError::Machine(format!(
                "unknown host helper '${}::{other}'",
                call.space
            ))),
        }
    }

    fn host_add(
        &mut self,
        args: Vec<SemanticValue>,
        call: &ContextCall,
    ) -> Result<SemanticValue, IsaError> {
        if args.len() != 3 {
            return Err(self.arity_error(call, 3, args.len()));
        }
        let lhs = args[0].as_int()? as u64;
        let rhs = args[1].as_int()? as u64;
        let width = self.parse_width(&args[2], call)?;
        let result = self.host.add(lhs, rhs, width);
        Ok(SemanticValue::tuple(vec![
            SemanticValue::int(result.value as i64),
            SemanticValue::bool(result.carry),
        ]))
    }

    fn host_add_with_carry(
        &mut self,
        args: Vec<SemanticValue>,
        call: &ContextCall,
    ) -> Result<SemanticValue, IsaError> {
        if !(args.len() == 3 || args.len() == 4) {
            return Err(self.arity_error(call, 4, args.len()));
        }
        let lhs = args[0].as_int()? as u64;
        let rhs = args[1].as_int()? as u64;
        let (carry_in, width_idx) = if args.len() == 3 {
            (false, 2)
        } else {
            (args[2].as_bool()?, 3)
        };
        let width = self.parse_width(&args[width_idx], call)?;
        let result = self.host.add_with_carry(lhs, rhs, carry_in, width);
        Ok(SemanticValue::tuple(vec![
            SemanticValue::int(result.value as i64),
            SemanticValue::bool(result.carry),
        ]))
    }

    fn host_sub(
        &mut self,
        args: Vec<SemanticValue>,
        call: &ContextCall,
    ) -> Result<SemanticValue, IsaError> {
        if args.len() != 3 {
            return Err(self.arity_error(call, 3, args.len()));
        }
        let lhs = args[0].as_int()? as u64;
        let rhs = args[1].as_int()? as u64;
        let width = self.parse_width(&args[2], call)?;
        let result = self.host.sub(lhs, rhs, width);
        Ok(SemanticValue::tuple(vec![
            SemanticValue::int(result.value as i64),
            SemanticValue::bool(result.carry),
        ]))
    }

    fn host_mul(
        &mut self,
        args: Vec<SemanticValue>,
        call: &ContextCall,
    ) -> Result<SemanticValue, IsaError> {
        if args.len() != 3 {
            return Err(self.arity_error(call, 3, args.len()));
        }
        let lhs = args[0].as_int()? as u64;
        let rhs = args[1].as_int()? as u64;
        let width = self.parse_width(&args[2], call)?;
        let result = self.host.mul(lhs, rhs, width);
        Ok(SemanticValue::tuple(vec![
            SemanticValue::int(result.low as i64),
            SemanticValue::int(result.high as i64),
        ]))
    }

    fn arity_error(&self, call: &ContextCall, expected: usize, actual: usize) -> IsaError {
        IsaError::Machine(format!(
            "call '${}::{}' expects {expected} arguments, got {actual}",
            call.space, call.name
        ))
    }

    fn parse_width(&self, value: &SemanticValue, call: &ContextCall) -> Result<u32, IsaError> {
        let width = value.as_int()?;
        if width < 0 {
            return Err(IsaError::Machine(format!(
                "call '${}::{}' requires non-negative width",
                call.space, call.name
            )));
        }
        let width = u32::try_from(width).map_err(|_| {
            IsaError::Machine(format!(
                "call '${}::{}' width exceeds supported range",
                call.space, call.name
            ))
        })?;
        if width > 64 {
            return Err(IsaError::Machine(format!(
                "call '${}::{}' width {width} exceeds 64-bit maximum",
                call.space, call.name
            )));
        }
        Ok(width)
    }

    fn bind_arguments(
        &self,
        names: &[String],
        args: Vec<SemanticValue>,
        call: &ContextCall,
    ) -> Result<HashMap<String, SemanticValue>, IsaError> {
        if names.len() != args.len() {
            return Err(self.arity_error(call, names.len(), args.len()));
        }
        let mut params = HashMap::with_capacity(names.len());
        for (name, value) in names.iter().cloned().zip(args.into_iter()) {
            params.insert(name, value);
        }
        Ok(params)
    }

    fn invoke_program(
        &mut self,
        params: HashMap<String, SemanticValue>,
        program: &SemanticProgram,
    ) -> Result<SemanticValue, IsaError> {
        let result = self.runtime.execute_nested_program(
            self.machine,
            self.state,
            self.host,
            self.stack,
            params,
            program,
        )?;
        Ok(result.unwrap_or_else(|| SemanticValue::Tuple(Vec::new())))
    }

    fn find_instruction(&self, call: &ContextCall) -> Result<&Instruction, IsaError> {
        let mut matches = self
            .machine
            .instructions
            .iter()
            .filter(|instr| instr.name == call.name);
        let Some(first) = matches.next() else {
            return Err(IsaError::Machine(format!(
                "unknown instruction '${}::{}'",
                call.space, call.name
            )));
        };
        if matches.next().is_some() {
            return Err(IsaError::Machine(format!(
                "instruction call '${}::{}' is ambiguous",
                first.space, call.name
            )));
        }
        Ok(first)
    }

    fn instruction_operands(&self, instruction: &Instruction) -> Result<Vec<String>, IsaError> {
        if !instruction.operands.is_empty() {
            return Ok(instruction.operands.clone());
        }
        let form_name = instruction.form.as_ref().ok_or_else(|| {
            IsaError::Machine(format!(
                "instruction '{}::{}' is missing operand metadata",
                instruction.space, instruction.name
            ))
        })?;
        let space = self.machine.spaces.get(&instruction.space).ok_or_else(|| {
            IsaError::Machine(format!(
                "instruction '{}::{}' references unknown space '{}'",
                instruction.space, instruction.name, instruction.space
            ))
        })?;
        let form = space.forms.get(form_name).ok_or_else(|| {
            IsaError::Machine(format!(
                "instruction '{}::{}' references unknown form '{}::{}'",
                instruction.space, instruction.name, instruction.space, form_name
            ))
        })?;
        Ok(form.operand_order.clone())
    }
}

impl<'runtime, 'machine, 'state, 'host, 'stack> ContextCallResolver
    for RuntimeCallResolver<'runtime, 'machine, 'state, 'host, 'stack>
{
    fn evaluate_context_call(
        &mut self,
        call: &ContextCall,
        args: Vec<SemanticValue>,
    ) -> Result<SemanticValue, IsaError> {
        match call.kind {
            ContextKind::Register => self.evaluate_register_call(call, args),
            ContextKind::Host => self.evaluate_host_call(call, args),
            ContextKind::Macro => self.evaluate_macro_call(call, args),
            ContextKind::Instruction => self.evaluate_instruction_call(call, args),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::core::specification::CoreSpec;
    use crate::soc::device::Endianness;
    use crate::soc::isa::ast::{
        ContextReference, FieldDecl, FieldIndexRange, InstructionDecl, IsaItem, IsaSpecification,
        MacroDecl, SpaceAttribute, SpaceDecl, SpaceKind, SpaceMember, SpaceMemberDecl,
        SubFieldDecl,
    };
    use crate::soc::isa::diagnostic::{SourcePosition, SourceSpan};
    use crate::soc::isa::error::IsaError;
    use crate::soc::isa::machine::{MachineDescription, SoftwareHost};
    use crate::soc::isa::semantics::SemanticBlock;
    use crate::soc::isa::semantics::program::{
        AssignTarget, ContextCall, ContextKind, Expr, ExprBinaryOp, RegisterRef, SemanticProgram,
        SemanticStmt,
    };
    use crate::soc::isa::semantics::value::SemanticValue;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn evaluate_expression_reads_register_calls() {
        let (runtime, machine, mut state) = test_runtime_state();
        state
            .write_register("reg::GPR1", 0x1234)
            .expect("seed gpr1");

        let mut params = HashMap::new();
        params.insert("idx".into(), SemanticValue::int(1));
        let expr = Expr::Call(ContextCall {
            kind: ContextKind::Register,
            space: "reg".into(),
            name: "GPR".into(),
            subpath: Vec::new(),
            args: vec![Expr::Parameter("idx".into())],
        });
        let program = SemanticProgram {
            statements: vec![SemanticStmt::Return(expr)],
        };

        let mut host = SoftwareHost::default();
        let value = runtime
            .execute_program(&machine, &mut state, &mut host, &params, &program)
            .expect("execute program")
            .expect("return value");
        assert_eq!(value.as_int().unwrap(), 0x1234);
    }

    #[test]
    fn statement_execution_handles_variable_assignment_and_return() {
        let (runtime, machine, mut state) = test_runtime_state();
        let program = SemanticProgram {
            statements: vec![
                SemanticStmt::Assign {
                    target: AssignTarget::Variable("tmp".into()),
                    expr: Expr::Number(42),
                },
                SemanticStmt::Return(Expr::Tuple(vec![Expr::Variable("tmp".into())])),
            ],
        };

        let params = HashMap::new();
        let mut host = SoftwareHost::default();
        let result = runtime
            .execute_program(&machine, &mut state, &mut host, &params, &program)
            .expect("execute program")
            .expect("return value");

        match result {
            SemanticValue::Tuple(values) => {
                assert_eq!(values.len(), 1);
                assert_eq!(values[0].as_int().unwrap(), 42);
            }
            other => panic!("expected tuple return, got {other:?}"),
        }
    }

    #[test]
    fn statement_execution_supports_tuple_destructuring() {
        let (runtime, machine, mut state) = test_runtime_state();
        let program = SemanticProgram {
            statements: vec![
                SemanticStmt::Assign {
                    target: AssignTarget::Tuple(vec!["res".into(), "carry".into()]),
                    expr: Expr::Tuple(vec![Expr::Number(10), Expr::Number(1)]),
                },
                SemanticStmt::Return(Expr::Tuple(vec![
                    Expr::Variable("carry".into()),
                    Expr::Variable("res".into()),
                ])),
            ],
        };

        let params = HashMap::new();
        let mut host = SoftwareHost::default();
        let result = runtime
            .execute_program(&machine, &mut state, &mut host, &params, &program)
            .expect("execute program")
            .expect("return value");

        match result {
            SemanticValue::Tuple(values) => {
                assert_eq!(values.len(), 2);
                assert_eq!(values[0].as_int().unwrap(), 1);
                assert_eq!(values[1].as_int().unwrap(), 10);
            }
            _ => panic!("expected tuple return"),
        }
    }

    #[test]
    fn statement_execution_writes_register_targets() {
        let (runtime, machine, mut state) = test_runtime_state();
        let program = SemanticProgram {
            statements: vec![SemanticStmt::Assign {
                target: AssignTarget::Register(RegisterRef {
                    space: "reg".into(),
                    name: "ACC".into(),
                    subfield: None,
                    index: None,
                }),
                expr: Expr::Number(0x55),
            }],
        };

        let params = HashMap::new();
        let mut host = SoftwareHost::default();
        let result = runtime
            .execute_program(&machine, &mut state, &mut host, &params, &program)
            .expect("execute program");
        assert!(result.is_none());

        let raw = state.read_register("reg::ACC").expect("read acc");
        assert_eq!(raw, 0x55);
    }

    #[test]
    fn statement_execution_resolves_register_index_expressions() {
        let (runtime, machine, mut state) = test_runtime_state();
        let program = SemanticProgram {
            statements: vec![SemanticStmt::Assign {
                target: AssignTarget::Register(RegisterRef {
                    space: "reg".into(),
                    name: "GPR".into(),
                    subfield: None,
                    index: Some(Expr::Parameter("idx".into())),
                }),
                expr: Expr::Number(0xDEADBEEF),
            }],
        };

        let mut params = HashMap::new();
        params.insert("idx".into(), SemanticValue::int(1));
        let mut host = SoftwareHost::default();
        runtime
            .execute_program(&machine, &mut state, &mut host, &params, &program)
            .expect("execute program");

        let raw = state.read_register("reg::GPR1").expect("read gpr1");
        assert_eq!(raw, 0xDEADBEEF);
    }

    #[test]
    fn tuple_assignment_arity_mismatch_raises_error() {
        let (runtime, machine, mut state) = test_runtime_state();
        let program = SemanticProgram {
            statements: vec![SemanticStmt::Assign {
                target: AssignTarget::Tuple(vec!["a".into(), "b".into()]),
                expr: Expr::Tuple(vec![Expr::Number(1)]),
            }],
        };

        let params = HashMap::new();
        let mut host = SoftwareHost::default();
        let result = runtime.execute_program(&machine, &mut state, &mut host, &params, &program);
        assert!(result.is_err());
    }

    #[test]
    fn host_call_invokes_services() {
        let (runtime, machine, mut state) = test_runtime_state();
        let program = SemanticProgram {
            statements: vec![
                SemanticStmt::Assign {
                    target: AssignTarget::Tuple(vec!["res".into(), "carry".into()]),
                    expr: Expr::Call(ContextCall {
                        kind: ContextKind::Host,
                        space: "host".into(),
                        name: "add".into(),
                        subpath: Vec::new(),
                        args: vec![Expr::Number(5), Expr::Number(7), Expr::Number(32)],
                    }),
                },
                SemanticStmt::Return(Expr::Variable("res".into())),
            ],
        };

        let params = HashMap::new();
        let mut host = SoftwareHost::default();
        let value = runtime
            .execute_program(&machine, &mut state, &mut host, &params, &program)
            .expect("execute program")
            .expect("return value");
        assert_eq!(value.as_int().unwrap(), 12);
    }

    #[test]
    fn macro_call_reuses_semantics() {
        let (runtime, machine, mut state) = test_runtime_state();
        let program = SemanticProgram {
            statements: vec![SemanticStmt::Return(Expr::Call(ContextCall {
                kind: ContextKind::Macro,
                space: "macro".into(),
                name: "inc".into(),
                subpath: Vec::new(),
                args: vec![Expr::Number(4)],
            }))],
        };

        let params = HashMap::new();
        let mut host = SoftwareHost::default();
        let value = runtime
            .execute_program(&machine, &mut state, &mut host, &params, &program)
            .expect("execute program")
            .expect("return value");
        assert_eq!(value.as_int().unwrap(), 5);
    }

    #[test]
    fn instruction_call_executes_semantics() {
        let (runtime, machine, mut state) = test_runtime_state();
        let program = SemanticProgram {
            statements: vec![
                SemanticStmt::Expr(Expr::Call(ContextCall {
                    kind: ContextKind::Instruction,
                    space: "insn".into(),
                    name: "mirror".into(),
                    subpath: Vec::new(),
                    args: vec![Expr::Number(9)],
                })),
                SemanticStmt::Return(Expr::Call(ContextCall {
                    kind: ContextKind::Register,
                    space: "reg".into(),
                    name: "ACC".into(),
                    subpath: Vec::new(),
                    args: Vec::new(),
                })),
            ],
        };

        let params = HashMap::new();
        let mut host = SoftwareHost::default();
        let value = runtime
            .execute_program(&machine, &mut state, &mut host, &params, &program)
            .expect("execute program")
            .expect("return value");
        assert_eq!(value.as_int().unwrap(), 9);
        let acc = state.read_register("reg::ACC").expect("read acc");
        assert_eq!(acc, 9);
    }

    #[test]
    fn recursive_macro_call_hits_limit() {
        let (runtime, machine, mut state) = test_runtime_state();
        let program = SemanticProgram {
            statements: vec![SemanticStmt::Expr(Expr::Call(ContextCall {
                kind: ContextKind::Macro,
                space: "macro".into(),
                name: "loopback".into(),
                subpath: Vec::new(),
                args: vec![Expr::Number(1)],
            }))],
        };

        let params = HashMap::new();
        let mut host = SoftwareHost::default();
        let err = runtime
            .execute_program(&machine, &mut state, &mut host, &params, &program)
            .expect_err("recursive macro should fail");
        match err {
            IsaError::Machine(msg) => assert!(msg.contains("call stack")),
            other => panic!("expected machine error, got {other:?}"),
        }
    }

    fn test_runtime_state() -> (SemanticRuntime, MachineDescription, CoreState) {
        let machine = build_machine();
        let core_spec = build_core_spec();
        let state = CoreState::new(core_spec).expect("core state");
        (SemanticRuntime::new(), machine, state)
    }

    fn build_machine() -> MachineDescription {
        let span = SourceSpan::point(PathBuf::from("test.isa"), SourcePosition::new(1, 1));
        let mut items = Vec::new();
        items.push(IsaItem::Space(SpaceDecl {
            name: "reg".into(),
            kind: SpaceKind::Register,
            attributes: vec![
                SpaceAttribute::WordSize(32),
                SpaceAttribute::Endianness(Endianness::Little),
            ],
            span: span.clone(),
            enable: None,
        }));

        items.push(IsaItem::SpaceMember(SpaceMemberDecl {
            space: "reg".into(),
            member: SpaceMember::Field(FieldDecl {
                space: "reg".into(),
                name: "ACC".into(),
                range: None,
                offset: None,
                size: Some(16),
                reset: None,
                description: None,
                redirect: None,
                subfields: Vec::new(),
                span: span.clone(),
                display: None,
            }),
        }));

        items.push(IsaItem::SpaceMember(SpaceMemberDecl {
            space: "reg".into(),
            member: SpaceMember::Field(FieldDecl {
                space: "reg".into(),
                name: "GPR".into(),
                range: Some(FieldIndexRange { start: 0, end: 1 }),
                offset: None,
                size: Some(32),
                reset: None,
                description: None,
                redirect: None,
                subfields: Vec::new(),
                span: span.clone(),
                display: None,
            }),
        }));

        items.push(IsaItem::SpaceMember(SpaceMemberDecl {
            space: "reg".into(),
            member: SpaceMember::Field(FieldDecl {
                space: "reg".into(),
                name: "FLAGS".into(),
                range: None,
                offset: None,
                size: Some(8),
                reset: None,
                description: None,
                redirect: None,
                subfields: vec![SubFieldDecl {
                    name: "ZERO".into(),
                    bit_spec: "@(0..1)".into(),
                    operations: Vec::new(),
                    description: None,
                }],
                span: span.clone(),
                display: None,
            }),
        }));

        items.push(IsaItem::SpaceMember(SpaceMemberDecl {
            space: "reg".into(),
            member: SpaceMember::Field(FieldDecl {
                space: "reg".into(),
                name: "ALIAS".into(),
                range: None,
                offset: None,
                size: Some(32),
                reset: None,
                description: None,
                redirect: Some(ContextReference {
                    segments: vec!["GPR0".into()],
                }),
                subfields: Vec::new(),
                span: span.clone(),
                display: None,
            }),
        }));

        items.push(IsaItem::Space(SpaceDecl {
            name: "insn".into(),
            kind: SpaceKind::Logic,
            attributes: vec![
                SpaceAttribute::WordSize(32),
                SpaceAttribute::Endianness(Endianness::Little),
            ],
            span: span.clone(),
            enable: None,
        }));

        let mut inc_block = SemanticBlock::empty();
        inc_block.set_program(SemanticProgram {
            statements: vec![
                SemanticStmt::Assign {
                    target: AssignTarget::Variable("tmp".into()),
                    expr: Expr::BinaryOp {
                        op: ExprBinaryOp::Add,
                        lhs: Box::new(Expr::Parameter("value".into())),
                        rhs: Box::new(Expr::Number(1)),
                    },
                },
                SemanticStmt::Return(Expr::Variable("tmp".into())),
            ],
        });

        let mut loop_block = SemanticBlock::empty();
        loop_block.set_program(SemanticProgram {
            statements: vec![SemanticStmt::Expr(Expr::Call(ContextCall {
                kind: ContextKind::Macro,
                space: "macro".into(),
                name: "loopback".into(),
                subpath: Vec::new(),
                args: vec![Expr::Parameter("value".into())],
            }))],
        });

        let mut mirror_block = SemanticBlock::empty();
        mirror_block.set_program(SemanticProgram {
            statements: vec![
                SemanticStmt::Assign {
                    target: AssignTarget::Register(RegisterRef {
                        space: "reg".into(),
                        name: "ACC".into(),
                        subfield: None,
                        index: None,
                    }),
                    expr: Expr::Parameter("VAL".into()),
                },
                SemanticStmt::Return(Expr::Parameter("VAL".into())),
            ],
        });

        items.push(IsaItem::Macro(MacroDecl {
            name: "inc".into(),
            parameters: vec!["value".into()],
            semantics: inc_block,
            span: span.clone(),
        }));

        items.push(IsaItem::Macro(MacroDecl {
            name: "loopback".into(),
            parameters: vec!["value".into()],
            semantics: loop_block,
            span: span.clone(),
        }));

        items.push(IsaItem::Instruction(InstructionDecl {
            space: "insn".into(),
            form: None,
            name: "mirror".into(),
            description: None,
            operands: vec!["VAL".into()],
            mask: None,
            encoding: None,
            semantics: Some(mirror_block),
            display: None,
            operator: None,
            span: span.clone(),
        }));

        let spec = IsaSpecification::new(PathBuf::from("test.isa"), items);
        MachineDescription::from_documents(vec![spec]).expect("machine description")
    }

    fn build_core_spec() -> Arc<CoreSpec> {
        Arc::new(
            CoreSpec::builder("demo", Endianness::Little)
                .register("reg::ACC", 16)
                .register("reg::GPR0", 32)
                .register("reg::GPR1", 32)
                .register("reg::FLAGS", 8)
                .build()
                .expect("core spec"),
        )
    }
}
