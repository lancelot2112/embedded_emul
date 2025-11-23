use crate::soc::isa::error::IsaError;

/// Canonical runtime value flowing through semantic programs.
#[derive(Debug, Clone, PartialEq)]
pub enum SemanticValue {
    Int(i64),
    Bool(bool),
    Word(String),
    Tuple(Vec<SemanticValue>),
}

impl SemanticValue {
    pub fn int(value: i64) -> Self {
        Self::Int(value)
    }

    pub fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    pub fn word(value: impl Into<String>) -> Self {
        Self::Word(value.into())
    }

    pub fn tuple(values: Vec<SemanticValue>) -> Self {
        Self::Tuple(values)
    }

    pub fn as_int(&self) -> Result<i64, IsaError> {
        match self {
            SemanticValue::Int(value) => Ok(*value),
            SemanticValue::Bool(value) => Ok(if *value { 1 } else { 0 }),
            SemanticValue::Word(_) => Err(IsaError::Machine(
                "word value cannot be coerced to integer".into(),
            )),
            SemanticValue::Tuple(_) => Err(IsaError::Machine(
                "tuple value cannot be coerced to integer".into(),
            )),
        }
    }

    pub fn as_bool(&self) -> Result<bool, IsaError> {
        match self {
            SemanticValue::Bool(value) => Ok(*value),
            SemanticValue::Int(value) => Ok(*value != 0),
            SemanticValue::Word(_) => Err(IsaError::Machine(
                "word value cannot be coerced to boolean".into(),
            )),
            SemanticValue::Tuple(_) => Err(IsaError::Machine(
                "tuple value cannot be coerced to boolean".into(),
            )),
        }
    }

    pub fn as_word(&self) -> Option<&str> {
        if let SemanticValue::Word(value) = self {
            Some(value.as_str())
        } else {
            None
        }
    }

    pub fn try_into_tuple(self) -> Result<TupleValue, IsaError> {
        match self {
            SemanticValue::Tuple(values) => Ok(TupleValue::new(values)),
            _ => Err(IsaError::Machine(
                "expected tuple value in assignment".into(),
            )),
        }
    }
}

/// Helper wrapper for tuple semantics so we can enforce arity checks.
#[derive(Debug, Clone, PartialEq)]
pub struct TupleValue {
    items: Vec<SemanticValue>,
}

impl TupleValue {
    pub fn new(items: Vec<SemanticValue>) -> Self {
        Self { items }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn ensure_len(&self, expected: usize) -> Result<(), IsaError> {
        if self.items.len() == expected {
            Ok(())
        } else {
            Err(IsaError::Machine(format!(
                "tuple length mismatch: expected {expected}, got {}",
                self.items.len()
            )))
        }
    }

    pub fn into_vec(self) -> Vec<SemanticValue> {
        self.items
    }
}

#[cfg(test)]
mod tests {
    use super::SemanticValue;

    #[test]
    fn semantic_value_bool_int_conversion() {
        let val_true = SemanticValue::bool(true);
        assert_eq!(val_true.as_int().unwrap(), 1);
        assert!(val_true.as_bool().unwrap());

        let val_false = SemanticValue::bool(false);
        assert_eq!(val_false.as_int().unwrap(), 0);
        assert!(!val_false.as_bool().unwrap());

        let number = SemanticValue::int(-42);
        assert_eq!(number.as_int().unwrap(), -42);
        assert!(number.as_bool().unwrap());
    }

    #[test]
    fn word_values_do_not_cast_to_scalar() {
        let word = SemanticValue::word("big");
        assert!(word.as_int().is_err());
        assert!(word.as_bool().is_err());
        assert_eq!(word.as_word(), Some("big"));
    }

    #[test]
    fn tuple_value_enforces_length() {
        let tuple = SemanticValue::tuple(vec![SemanticValue::int(5), SemanticValue::bool(false)]);
        let tuple_value = tuple.try_into_tuple().expect("tuple conversion");
        assert_eq!(tuple_value.len(), 2);
        assert!(tuple_value.ensure_len(2).is_ok());
        assert!(tuple_value.ensure_len(3).is_err());
    }
}
