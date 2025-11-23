//! Core runtime primitives for the semantics interpreter.
//!
//! The value model, execution context, and register helpers now live in their
//! own modules so this file can focus on orchestrating evaluation.

use crate::soc::core::state::CoreState;
use crate::soc::isa::error::IsaError;
use crate::soc::isa::machine::MachineDescription;
use crate::soc::isa::semantics::context::ExecutionContext;
use crate::soc::isa::semantics::expression::{ContextCallResolver, ExpressionEvaluator};
use crate::soc::isa::semantics::program::{ContextCall, ContextKind, Expr, RegisterRef};
use crate::soc::isa::semantics::register::RegisterAccess;
use crate::soc::isa::semantics::value::SemanticValue;

#[derive(Debug, Default)]
pub struct SemanticRuntime;

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
    pub fn evaluate_expression<'ctx>(
        &self,
        machine: &MachineDescription,
        state: &mut CoreState,
        context: &ExecutionContext<'ctx>,
        expr: &Expr,
    ) -> Result<SemanticValue, IsaError> {
        let registers = self.register_access(machine);
        let resolver = RuntimeCallResolver::new(registers, state);
        let mut evaluator = ExpressionEvaluator::with_resolver(context, resolver);
        evaluator.evaluate(expr)
    }
}

struct RuntimeCallResolver<'machine, 'state> {
    registers: RegisterAccess<'machine>,
    state: &'state mut CoreState,
}

impl<'machine, 'state> RuntimeCallResolver<'machine, 'state> {
    fn new(registers: RegisterAccess<'machine>, state: &'state mut CoreState) -> Self {
        Self { registers, state }
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
}

impl<'machine, 'state> ContextCallResolver for RuntimeCallResolver<'machine, 'state> {
    fn evaluate_context_call(
        &mut self,
        call: &ContextCall,
        args: Vec<SemanticValue>,
    ) -> Result<SemanticValue, IsaError> {
        match call.kind {
            ContextKind::Register => self.evaluate_register_call(call, args),
            _ => Err(IsaError::Machine(format!(
                "context call '${}::{}' is not supported yet",
                call.space, call.name
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::core::specification::CoreSpec;
    use crate::soc::device::Endianness;
    use crate::soc::isa::ast::{
        ContextReference, FieldDecl, FieldIndexRange, IsaItem, IsaSpecification, SpaceAttribute,
        SpaceDecl, SpaceKind, SpaceMember, SpaceMemberDecl, SubFieldDecl,
    };
    use crate::soc::isa::diagnostic::{SourcePosition, SourceSpan};
    use crate::soc::isa::machine::MachineDescription;
    use crate::soc::isa::semantics::context::ExecutionContext;
    use crate::soc::isa::semantics::program::{ContextCall, ContextKind, Expr};
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
        let ctx = ExecutionContext::new(&params);

        let expr = Expr::Call(ContextCall {
            kind: ContextKind::Register,
            space: "reg".into(),
            name: "GPR".into(),
            subpath: Vec::new(),
            args: vec![Expr::Parameter("idx".into())],
        });

        let value = runtime
            .evaluate_expression(&machine, &mut state, &ctx, &expr)
            .expect("evaluate expr");
        assert_eq!(value.as_int().unwrap(), 0x1234);
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
