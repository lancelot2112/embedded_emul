use std::collections::HashMap;

use crate::soc::isa::semantics::value::SemanticValue;

/// Scratch execution context used while evaluating a semantic program.
#[derive(Debug)]
pub struct ExecutionContext<'a> {
    params: &'a HashMap<String, SemanticValue>,
    locals: HashMap<String, SemanticValue>,
}

impl<'a> ExecutionContext<'a> {
    pub fn new(params: &'a HashMap<String, SemanticValue>) -> Self {
        Self {
            params,
            locals: HashMap::new(),
        }
    }

    pub fn set_local(&mut self, name: impl Into<String>, value: SemanticValue) {
        self.locals.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<&SemanticValue> {
        self.locals.get(name).or_else(|| self.params.get(name))
    }

    pub fn take_local(&mut self, name: &str) -> Option<SemanticValue> {
        self.locals.remove(name)
    }
}

#[cfg(test)]
mod tests {
    use super::ExecutionContext;
    use crate::soc::isa::semantics::value::SemanticValue;
    use std::collections::HashMap;

    fn sample_params() -> HashMap<String, SemanticValue> {
        HashMap::from([
            ("ra".to_string(), SemanticValue::int(10)),
            ("flag".to_string(), SemanticValue::bool(true)),
        ])
    }

    #[test]
    fn execution_context_scopes_locals_and_params() {
        let params = sample_params();
        let mut ctx = ExecutionContext::new(&params);

        assert_eq!(ctx.get("ra").and_then(|v| v.as_int().ok()), Some(10));
        assert_eq!(ctx.get("flag").and_then(|v| v.as_bool().ok()), Some(true));
        assert!(ctx.get("temp").is_none());

        ctx.set_local("ra", SemanticValue::int(99));
        ctx.set_local("temp", SemanticValue::int(1));

        assert_eq!(ctx.get("ra").and_then(|v| v.as_int().ok()), Some(99));
        assert_eq!(ctx.get("temp").and_then(|v| v.as_int().ok()), Some(1));
        assert_eq!(params.get("ra").and_then(|v| v.as_int().ok()), Some(10));

        assert_eq!(ctx.take_local("ra"), Some(SemanticValue::int(99)));
        assert_eq!(ctx.get("ra").and_then(|v| v.as_int().ok()), Some(10));
    }
}
