//! Instruction metadata and derived pattern descriptions used for decoding
//! machine words back into structured operations.

use crate::soc::isa::ast::InstructionDecl;
use crate::soc::isa::semantics::SemanticBlock;
use crate::soc::prog::types::BitFieldSpec;

use super::format::default_display_template;

#[derive(Debug, Clone)]
pub struct Instruction {
    pub space: String,
    pub name: String,
    pub form: Option<String>,
    pub description: Option<String>,
    pub operands: Vec<String>,
    pub display: Option<String>,
    pub operator: Option<String>,
    pub mask: Option<InstructionMask>,
    pub encoding: Option<BitFieldSpec>,
    pub semantics: Option<SemanticBlock>,
}

impl Instruction {
    pub fn from_decl(decl: InstructionDecl) -> Self {
        Self {
            space: decl.space,
            name: decl.name,
            form: decl.form,
            description: decl.description,
            operands: decl.operands,
            display: decl.display,
            operator: decl.operator,
            mask: decl.mask.map(|mask| InstructionMask {
                fields: mask.fields,
            }),
            encoding: decl.encoding,
            semantics: decl.semantics,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstructionMask {
    pub fields: Vec<crate::soc::isa::ast::MaskField>,
}

#[derive(Debug, Clone)]
pub struct InstructionPattern {
    pub instruction_idx: usize,
    pub space: String,
    pub form: Option<String>,
    pub mask: u64,
    pub value: u64,
    pub operand_names: Vec<String>,
    pub display: Option<String>,
    pub operator: Option<String>,
    pub specificity: u32,
}

impl InstructionPattern {
    pub fn with_display_override(
        &mut self,
        instr_display: Option<String>,
        form_display: Option<String>,
        operands: &[String],
    ) {
        self.display = instr_display
            .or(form_display)
            .or_else(|| default_display_template(self.form.as_ref(), operands));
    }
}
