pub(super) fn parse_numeric_literal(text: &str) -> Result<u64, &'static str> {
    if text.starts_with('-') {
        return Err("negative values are not supported here");
    }
    let cleaned = text.replace('_', "");
    let (radix, digits) = if let Some(stripped) = cleaned.strip_prefix("0x") {
        (16, stripped)
    } else if let Some(stripped) = cleaned.strip_prefix("0b") {
        (2, stripped)
    } else if let Some(stripped) = cleaned.strip_prefix("0o") {
        (8, stripped)
    } else {
        (10, cleaned.as_str())
    };
    if digits.is_empty() {
        return Err("numeric literal missing digits");
    }
    u64::from_str_radix(digits, radix).map_err(|_| "numeric literal out of range")
}

#[cfg(test)]
mod tests {
    use super::parse_numeric_literal;

    #[test]
    fn parses_hex_literal() {
        assert_eq!(parse_numeric_literal("0x10").unwrap(), 16);
    }

    #[test]
    fn rejects_negative_literal() {
        assert!(parse_numeric_literal("-1").is_err());
    }
}
