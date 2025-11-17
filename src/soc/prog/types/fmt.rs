//! Formatting helpers shared by scalar and pretty-printer utilities.

pub fn format_hex(value: u64, width: usize) -> String {
    format!("0x{value:0width$x}")
}

pub fn format_dot(bytes: impl IntoIterator<Item = u8>) -> String {
    bytes
        .into_iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(".")
}

#[cfg(test)]
mod tests {
    //! Guarantees formatting helpers stay stable for user-facing tooling.
    use super::*;

    #[test]
    fn hex_formatter_pads_width() {
        // ensures format_hex honors the requested padding precision
        assert_eq!(format_hex(0xAB, 4), "0x00ab", "hex formatter should pad lower-than-width values");
    }

    #[test]
    fn dot_formatter_concatenates_segments() {
        // ensures dotted notation uses '.' separators in byte order
        let rendered = format_dot([192, 168, 0, 1]);
        assert_eq!(rendered, "192.168.0.1", "dot notation should separate bytes with periods");
    }
}
