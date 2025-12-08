use std::collections::HashSet;

use crate::soc::isa::ast::SpaceKind;
use crate::soc::isa::diagnostic::{DiagnosticLevel, DiagnosticPhase, IsaDiagnostic, SourceSpan};
use crate::soc::isa::error::IsaError;
use crate::soc::isa::machine::{Instruction, MachineDescription, MacroInfo};

use super::program::{
    AssignTarget, ContextCall, ContextKind, Expr, RegisterRef, SemanticProgram, SemanticStmt,
};

pub struct SemanticAnalyzer<'machine> {
    machine: &'machine MachineDescription,
    global_params: Vec<String>,
}

impl<'machine> SemanticAnalyzer<'machine> {
    pub fn new(machine: &'machine MachineDescription) -> Self {
        let global_params = machine.parameters.keys().cloned().collect();
        Self {
            machine,
            global_params,
        }
    }

    pub fn analyze_macro(
        &self,
        mac: &MacroInfo,
        program: &SemanticProgram,
    ) -> Result<(), IsaError> {
        let params = self
            .global_params
            .iter()
            .cloned()
            .chain(mac.parameters.iter().cloned());
        let mut scope = AnalyzerScope::new(params);
        self.validate_program(program, &mut scope)
    }

    pub fn analyze_instruction(
        &self,
        instruction: &Instruction,
        program: &SemanticProgram,
    ) -> Result<(), IsaError> {
        let operands = self.instruction_operands(instruction)?;
        let params = self.global_params.iter().cloned().chain(operands);
        let mut scope = AnalyzerScope::new(params);
        self.validate_program(program, &mut scope)
    }

    fn validate_program(
        &self,
        program: &SemanticProgram,
        scope: &mut AnalyzerScope,
    ) -> Result<(), IsaError> {
        let mut diagnostics = Vec::new();
        for stmt in &program.statements {
            self.validate_statement(stmt, scope, &mut diagnostics)?;
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(IsaError::Diagnostics {
                phase: DiagnosticPhase::Validation,
                diagnostics,
            })
        }
    }

    fn validate_statement(
        &self,
        stmt: &SemanticStmt,
        scope: &mut AnalyzerScope,
        diags: &mut Vec<IsaDiagnostic>,
    ) -> Result<(), IsaError> {
        match stmt {
            SemanticStmt::Assign { target, expr } => {
                self.validate_expr(expr, scope, diags)?;
                self.bind_target(target, scope, diags);
            }
            SemanticStmt::Expr(expr) | SemanticStmt::Return(expr) => {
                self.validate_expr(expr, scope, diags)?;
            }
        }
        Ok(())
    }

    fn bind_target(
        &self,
        target: &AssignTarget,
        scope: &mut AnalyzerScope,
        diags: &mut Vec<IsaDiagnostic>,
    ) {
        match target {
            AssignTarget::Variable(name) => scope.define(name.clone()),
            AssignTarget::Tuple(names) => {
                for name in names {
                    scope.define(name.clone());
                }
            }
            AssignTarget::Register(reference) => {
                self.validate_register_reference(reference, diags);
            }
        }
    }

    fn validate_expr(
        &self,
        expr: &Expr,
        scope: &mut AnalyzerScope,
        diags: &mut Vec<IsaDiagnostic>,
    ) -> Result<(), IsaError> {
        match expr {
            Expr::Number(_) => {}
            Expr::Variable { name, span } => {
                if !scope.has_variable(name) {
                    self.push_diag(
                        diags,
                        "semantics.undefined-variable",
                        format!("variable '{name}' is not defined in this scope"),
                        Some(span.clone()),
                    );
                }
            }
            Expr::Parameter { name, span } => {
                if !scope.has_parameter(name) {
                    self.push_diag(
                        diags,
                        "semantics.undefined-parameter",
                        format!("parameter '#{name}' is not defined for this context"),
                        Some(span.clone()),
                    );
                }
            }
            Expr::Call(call) => {
                for arg in &call.args {
                    self.validate_expr(arg, scope, diags)?;
                }
                self.validate_context_call(call, diags)?;
            }
            Expr::Tuple(items) => {
                for expr in items {
                    self.validate_expr(expr, scope, diags)?;
                }
            }
            Expr::BinaryOp { lhs, rhs, .. } => {
                self.validate_expr(lhs, scope, diags)?;
                self.validate_expr(rhs, scope, diags)?;
            }
            Expr::BitSlice { expr, .. } => {
                self.validate_expr(expr, scope, diags)?;
            }
            Expr::UnaryOp { expr, .. } => {
                self.validate_expr(expr, scope, diags)?;
            }
        }
        Ok(())
    }

    fn validate_context_call(
        &self,
        call: &ContextCall,
        diags: &mut Vec<IsaDiagnostic>,
    ) -> Result<(), IsaError> {
        match call.kind {
            ContextKind::Register => {
                self.validate_register_call(call, diags);
                Ok(())
            }
            ContextKind::Macro => {
                self.validate_macro_call(call, diags);
                Ok(())
            }
            ContextKind::Instruction => self.validate_instruction_call(call, diags),
            ContextKind::Host => {
                self.validate_host_call(call, diags);
                Ok(())
            }
        }
    }

    fn validate_macro_call(&self, call: &ContextCall, diags: &mut Vec<IsaDiagnostic>) {
        if let Some(mac) = self.machine.macros.iter().find(|mac| mac.name == call.name) {
            let expected = mac.parameters.len();
            if call.args.len() != expected {
                self.push_arity_diag(call, expected, call.args.len(), diags);
            }
        } else {
            self.push_diag(
                diags,
                "semantics.unknown-macro",
                format!("macro '${}::{}' does not exist", call.space, call.name),
                Some(call.span.clone()),
            );
        }
    }

    fn validate_instruction_call(
        &self,
        call: &ContextCall,
        diags: &mut Vec<IsaDiagnostic>,
    ) -> Result<(), IsaError> {
        let mut matches = self
            .machine
            .instructions
            .iter()
            .filter(|instr| instr.name == call.name);
        let Some(first) = matches.next() else {
            self.push_diag(
                diags,
                "semantics.unknown-instruction",
                format!(
                    "instruction '${}::{}' does not exist",
                    call.space, call.name
                ),
                Some(call.span.clone()),
            );
            return Ok(());
        };
        if matches.next().is_some() {
            self.push_diag(
                diags,
                "semantics.ambiguous-instruction",
                format!(
                    "instruction call '${}::{}' is ambiguous across spaces",
                    call.space, call.name
                ),
                Some(call.span.clone()),
            );
            return Ok(());
        }
        let operands = self.instruction_operands(first)?;
        if call.args.len() != operands.len() {
            self.push_arity_diag(call, operands.len(), call.args.len(), diags);
        }
        Ok(())
    }

    fn validate_host_call(&self, call: &ContextCall, diags: &mut Vec<IsaDiagnostic>) {
        match call.name.as_str() {
            "add" | "sub" => {
                if call.args.len() != 4 {
                    self.push_arity_diag(call, 4, call.args.len(), diags);
                }
            }
            "mul" => {
                if call.args.len() != 3 {
                    self.push_arity_diag(call, 3, call.args.len(), diags);
                }
            }
            other => {
                self.push_diag(
                    diags,
                    "semantics.unknown-host",
                    format!("host helper '${}::{other}' is not supported", call.space),
                    Some(call.span.clone()),
                );
            }
        }
    }

    fn validate_register_call(&self, call: &ContextCall, diags: &mut Vec<IsaDiagnostic>) {
        self.validate_register_components(
            &call.space,
            &call.name,
            call.subpath.first().map(|value| value.as_str()),
            Some(call.span.clone()),
            diags,
        );
    }

    fn validate_register_reference(&self, reference: &RegisterRef, diags: &mut Vec<IsaDiagnostic>) {
        self.validate_register_components(
            &reference.space,
            &reference.name,
            reference.subfield.as_deref(),
            reference.span.clone(),
            diags,
        );
    }

    fn validate_register_components(
        &self,
        space_name: &str,
        register_name: &str,
        subfield: Option<&str>,
        span: Option<SourceSpan>,
        diags: &mut Vec<IsaDiagnostic>,
    ) {
        let Some(space) = self.machine.spaces.get(space_name) else {
            self.push_diag(
                diags,
                "semantics.unknown-register-space",
                format!("register space '{space_name}' is not defined"),
                span.clone(),
            );
            return;
        };
        if space.kind != SpaceKind::Register {
            self.push_diag(
                diags,
                "semantics.invalid-register-space",
                format!("space '{space_name}' is not a register space"),
                span.clone(),
            );
            return;
        }
        let mut register = space.registers.get(register_name);
        if register.is_none()
            && let Some((metadata, _)) = self
                .machine
                .register_schema()
                .find_by_label(space_name, register_name)
        {
            register = space.registers.get(&metadata.name);
        }
        let Some(register) = register else {
            self.push_diag(
                diags,
                "semantics.unknown-register",
                format!(
                    "register '{}::{}' is not defined",
                    space_name, register_name
                ),
                span.clone(),
            );
            return;
        };
        if let Some(field) = subfield
            && !register.subfields.iter().any(|sub| sub.name == field)
        {
            self.push_diag(
                diags,
                "semantics.unknown-register-subfield",
                format!(
                    "register '{}::{}' has no subfield '{}'",
                    space_name, register_name, field
                ),
                span,
            );
        }
    }

    fn instruction_operands(&self, instruction: &Instruction) -> Result<Vec<String>, IsaError> {
        if !instruction.operands.is_empty() {
            return Ok(instruction.operands.clone());
        }
        if instruction.form.is_none() {
            return Ok(Vec::new());
        }
        let form_name = instruction.form.as_ref().expect("form present");
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

    fn push_arity_diag(
        &self,
        call: &ContextCall,
        expected: usize,
        actual: usize,
        diags: &mut Vec<IsaDiagnostic>,
    ) {
        self.push_diag(
            diags,
            "semantics.call-arity",
            format!(
                "call '${}::{}' expects {expected} arguments, got {actual}",
                call.space, call.name
            ),
            Some(call.span.clone()),
        );
    }

    fn push_diag(
        &self,
        diags: &mut Vec<IsaDiagnostic>,
        code: &'static str,
        message: impl Into<String>,
        span: Option<SourceSpan>,
    ) {
        diags.push(IsaDiagnostic::new(
            DiagnosticPhase::Validation,
            DiagnosticLevel::Error,
            code,
            message,
            span,
        ));
    }
}

struct AnalyzerScope {
    parameters: HashSet<String>,
    locals: HashSet<String>,
}

impl AnalyzerScope {
    fn new<I>(params: I) -> Self
    where
        I: IntoIterator<Item = String>,
    {
        Self {
            parameters: params.into_iter().collect(),
            locals: HashSet::new(),
        }
    }

    fn has_variable(&self, name: &str) -> bool {
        self.locals.contains(name)
    }

    fn has_parameter(&self, name: &str) -> bool {
        self.parameters.contains(name)
    }

    fn define(&mut self, name: String) {
        self.locals.insert(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::endianness::Endianness;
    use crate::soc::isa::ast::{
        InstructionDecl, IsaItem, IsaSpecification, SpaceAttribute, SpaceDecl, SpaceKind,
    };
    use crate::soc::isa::diagnostic::{SourcePosition, SourceSpan};
    use crate::soc::isa::semantics::program::{Expr, SemanticStmt};
    use crate::soc::isa::semantics::{SemanticBlock, SemanticProgram};
    use std::path::PathBuf;

    fn var(name: &str, line: usize) -> Expr {
        Expr::Variable {
            name: name.into(),
            span: SourceSpan::point(PathBuf::from("test.isa"), SourcePosition::new(line, 1)),
        }
    }

    fn param_expr(name: &str, line: usize) -> Expr {
        Expr::Parameter {
            name: name.into(),
            span: SourceSpan::point(PathBuf::from("test.isa"), SourcePosition::new(line, 1)),
        }
    }

    #[test]
    fn reports_undefined_locals() {
        let mut block = SemanticBlock::empty();
        block.set_program(SemanticProgram {
            statements: vec![SemanticStmt::Return(var("missing", 12))],
        });
        let spec = specification(block);
        let err = MachineDescription::from_documents(vec![spec]).expect_err("should fail");
        match err {
            IsaError::Diagnostics { diagnostics, .. } => {
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].code, "semantics.undefined-variable");
                assert!(matches!(
                    diagnostics[0].span.as_ref().map(|span| span.start.line),
                    Some(12)
                ));
            }
            other => panic!("expected diagnostics error, got {other:?}"),
        }
    }

    #[test]
    fn reports_undefined_parameters() {
        let mut block = SemanticBlock::empty();
        block.set_program(SemanticProgram {
            statements: vec![SemanticStmt::Return(param_expr("WIDTH", 5))],
        });
        let spec = specification(block);
        let err = MachineDescription::from_documents(vec![spec]).expect_err("should fail");
        match err {
            IsaError::Diagnostics { diagnostics, .. } => {
                assert_eq!(diagnostics[0].code, "semantics.undefined-parameter");
                assert!(matches!(
                    diagnostics[0].span.as_ref().map(|span| span.start.line),
                    Some(5)
                ));
            }
            other => panic!("expected diagnostics error, got {other:?}"),
        }
    }

    fn specification(block: SemanticBlock) -> IsaSpecification {
        let path = PathBuf::from("test.isa");
        let span = SourceSpan::point(path.clone(), SourcePosition::new(1, 1));
        let items = vec![
            IsaItem::Space(SpaceDecl {
                name: "insn".into(),
                kind: SpaceKind::Logic,
                attributes: vec![
                    SpaceAttribute::WordSize(32),
                    SpaceAttribute::Endianness(Endianness::Little),
                ],
                span: span.clone(),
                enable: None,
            }),
            IsaItem::Instruction(InstructionDecl {
                space: "insn".into(),
                form: None,
                name: "noop".into(),
                description: None,
                operands: Vec::new(),
                mask: None,
                encoding: None,
                semantics: Some(block),
                display: None,
                operator: None,
                span,
            }),
        ];
        IsaSpecification::new(path, items)
    }
}
