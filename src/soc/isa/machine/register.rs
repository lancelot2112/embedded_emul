//! Register metadata helpers for the machine module. Defines register binding
//! declarations, formatting helpers, and register-space utilities.

use std::iter::Peekable;
use std::str::Chars;

use crate::soc::isa::ast::{FieldDecl, FieldIndexRange, SubFieldOp};

#[derive(Debug, Clone)]
pub struct RegisterInfo {
    pub name: String,
    pub range: Option<FieldIndexRange>,
    display: Option<String>,
}

impl RegisterInfo {
    pub fn from_decl(decl: FieldDecl) -> Self {
        Self {
            name: decl.name,
            range: decl.range,
            display: decl.display,
        }
    }

    pub fn format(&self, value: u64) -> String {
        if let Some(pattern) = &self.display {
            return format_register_display(pattern, value);
        }
        if self.range.is_some() {
            format!("{}{}", self.name, value)
        } else {
            self.name.clone()
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegisterBinding {
    pub space: String,
    pub field: String,
}

pub(super) fn derive_register_binding(ops: &[SubFieldOp]) -> Option<RegisterBinding> {
    ops.iter().find_map(parse_register_op)
}

fn parse_register_op(op: &SubFieldOp) -> Option<RegisterBinding> {
    if let Some(binding) = parse_context_style_register(op) {
        return Some(binding);
    }
    if op.kind.eq_ignore_ascii_case("reg") {
        if let Some(field) = &op.subtype {
            return Some(RegisterBinding {
                space: "reg".into(),
                field: field.clone(),
            });
        }
    }
    None
}

fn parse_context_style_register(op: &SubFieldOp) -> Option<RegisterBinding> {
    if !op.kind.starts_with('$') {
        return None;
    }
    let mut segments: Vec<&str> = op.kind.split("::").collect();
    if segments.len() < 2 {
        return None;
    }
    let space = segments.remove(0).trim_start_matches('$');
    let field = segments.remove(0);
    if space.is_empty() || field.is_empty() {
        return None;
    }
    Some(RegisterBinding {
        space: space.to_string(),
        field: field.to_string(),
    })
}

fn format_register_display(pattern: &str, value: u64) -> String {
    let mut result = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            result.push(ch);
            continue;
        }

        if matches!(chars.peek(), Some('%')) {
            chars.next();
            result.push('%');
            continue;
        }

        if let Some(fragment) = next_display_fragment(&mut chars, value) {
            result.push_str(&fragment);
        } else {
            result.push('%');
        }
    }
    result
}

fn next_display_fragment(iter: &mut Peekable<Chars<'_>>, value: u64) -> Option<String> {
    let mut zero_pad = false;
    let mut width_digits = String::new();

    while let Some(&ch) = iter.peek() {
        if ch == '0' && width_digits.is_empty() {
            zero_pad = true;
            iter.next();
            continue;
        }
        if ch.is_ascii_digit() {
            width_digits.push(ch);
            iter.next();
        } else {
            break;
        }
    }

    let width = if width_digits.is_empty() {
        None
    } else {
        width_digits.parse().ok()
    };

    let spec = iter.next()?;
    Some(match spec {
        'd' | 'u' => format_number(value, width, zero_pad, NumberFormat::Decimal),
        'x' => format_number(value, width, zero_pad, NumberFormat::HexLower),
        'X' => format_number(value, width, zero_pad, NumberFormat::HexUpper),
        '%' => "%".into(),
        other => {
            let mut literal = String::from("%");
            if zero_pad {
                literal.push('0');
            }
            if let Some(w) = width {
                literal.push_str(&w.to_string());
            }
            literal.push(other);
            literal
        }
    })
}

#[derive(Clone, Copy)]
enum NumberFormat {
    Decimal,
    HexLower,
    HexUpper,
}

fn format_number(value: u64, width: Option<usize>, zero_pad: bool, format: NumberFormat) -> String {
    match format {
        NumberFormat::Decimal => match (width, zero_pad) {
            (Some(w), true) => format!("{value:0width$}", width = w),
            (Some(w), false) => format!("{value:width$}", width = w),
            (None, _) => format!("{value}"),
        },
        NumberFormat::HexLower => match (width, zero_pad) {
            (Some(w), true) => format!("{value:0width$x}", width = w),
            (Some(w), false) => format!("{value:width$x}", width = w),
            (None, _) => format!("{value:x}"),
        },
        NumberFormat::HexUpper => match (width, zero_pad) {
            (Some(w), true) => format!("{value:0width$X}", width = w),
            (Some(w), false) => format!("{value:width$X}", width = w),
            (None, _) => format!("{value:X}"),
        },
    }
}
