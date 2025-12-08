//! Parser and helper utilities for ISA-style integer range specifications.
//!
//! Supported syntaxes:
//! * `<start>..<end>` – exclusive upper bound
//! * `<start>..=<end>` – inclusive upper bound
//! * `<start>..+<length>` – explicit length (optionally suffixed with units like `kbit`, `kB`, `MB`, `GB`)
//!
//! Length suffixes default to bytes when omitted. Bit-based suffixes must resolve to a whole
//! number of bytes so the derived iterator can remain byte-addressed.

use std::{fmt, ops::Range};

use super::literal::{LiteralError, parse_u64_literal};

const KIB: u64 = 1024;
const MIB: u64 = KIB * 1024;
const GIB: u64 = MIB * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeSpec {
    start: u64,
    end: u64,
}

impl RangeSpec {
    pub fn new(start: u64, end: u64) -> Result<Self, RangeSpecError> {
        if end < start {
            return Err(RangeSpecError::InvalidOrdering { start, end });
        }
        Ok(Self { start, end })
    }

    pub fn parse(input: &str) -> Result<Self, RangeSpecError> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(RangeSpecError::Empty);
        }
        let (lhs, rhs) = trimmed
            .split_once("..")
            .ok_or(RangeSpecError::MissingDelimiter)?;
        let start = parse_u64_literal(lhs.trim()).map_err(RangeSpecError::StartLiteral)?;
        let rhs = rhs.trim_start();
        if rhs.is_empty() {
            return Err(RangeSpecError::MissingRhs);
        }

        if let Some(rest) = rhs.strip_prefix('=') {
            Self::parse_inclusive(start, rest)
        } else if let Some(rest) = rhs.strip_prefix('+') {
            Self::parse_length(start, rest)
        } else {
            Self::parse_exclusive(start, rhs)
        }
    }

    pub fn start(&self) -> u64 {
        self.start
    }

    pub fn end(&self) -> u64 {
        self.end
    }

    pub fn len(&self) -> u64 {
        self.end - self.start
    }

    pub fn len_bytes(&self) -> u64 {
        self.len()
    }

    pub fn len_bits(&self) -> Option<u64> {
        self.len().checked_mul(8)
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn as_range(&self) -> Range<u64> {
        self.start..self.end
    }

    fn parse_exclusive(start: u64, rhs: &str) -> Result<Self, RangeSpecError> {
        let end = parse_u64_literal(rhs.trim()).map_err(RangeSpecError::EndLiteral)?;
        Self::new(start, end)
    }

    fn parse_inclusive(start: u64, rhs: &str) -> Result<Self, RangeSpecError> {
        let inclusive = parse_u64_literal(rhs.trim()).map_err(RangeSpecError::EndLiteral)?;
        let end = inclusive
            .checked_add(1)
            .ok_or(RangeSpecError::LengthOverflow)?;
        Self::new(start, end)
    }

    fn parse_length(start: u64, rhs: &str) -> Result<Self, RangeSpecError> {
        let length = parse_length_expr(rhs)?;
        let end = start
            .checked_add(length)
            .ok_or(RangeSpecError::LengthOverflow)?;
        Self::new(start, end)
    }
}

impl IntoIterator for RangeSpec {
    type Item = u64;
    type IntoIter = Range<u64>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_range()
    }
}

impl<'a> IntoIterator for &'a RangeSpec {
    type Item = u64;
    type IntoIter = Range<u64>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_range()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeSpecError {
    Empty,
    MissingDelimiter,
    MissingRhs,
    StartLiteral(LiteralError),
    EndLiteral(LiteralError),
    LengthLiteral(LiteralError),
    InvalidOrdering { start: u64, end: u64 },
    LengthOverflow,
    UnknownUnit(String),
    BitLengthNotByteAligned { bits: u64 },
    ExtraTokens(String),
}

impl fmt::Display for RangeSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RangeSpecError::Empty => write!(f, "range spec is empty"),
            RangeSpecError::MissingDelimiter => write!(f, "range spec is missing '..'"),
            RangeSpecError::MissingRhs => write!(f, "range spec missing trailing component"),
            RangeSpecError::StartLiteral(err) => write!(f, "failed to parse start literal: {err}"),
            RangeSpecError::EndLiteral(err) => write!(f, "failed to parse end literal: {err}"),
            RangeSpecError::LengthLiteral(err) => {
                write!(f, "failed to parse length literal: {err}")
            }
            RangeSpecError::InvalidOrdering { start, end } => {
                write!(f, "end {end:#x} precedes start {start:#x}")
            }
            RangeSpecError::LengthOverflow => write!(f, "range calculation overflowed"),
            RangeSpecError::UnknownUnit(unit) => write!(f, "unknown length unit '{unit}'"),
            RangeSpecError::BitLengthNotByteAligned { bits } => {
                write!(f, "bit length {bits} is not byte aligned")
            }
            RangeSpecError::ExtraTokens(tok) => {
                write!(f, "unexpected tokens '{tok}' in range length")
            }
        }
    }
}

impl std::error::Error for RangeSpecError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnitKind {
    Bytes,
    Bits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LengthUnit {
    kind: UnitKind,
    scale: u64,
}

impl LengthUnit {
    const fn bytes(scale: u64) -> Self {
        Self {
            kind: UnitKind::Bytes,
            scale,
        }
    }

    const fn bits(scale: u64) -> Self {
        Self {
            kind: UnitKind::Bits,
            scale,
        }
    }
}

fn parse_length_expr(expr: &str) -> Result<u64, RangeSpecError> {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return Err(RangeSpecError::MissingRhs);
    }
    let mut parts = trimmed.split_whitespace();
    let literal_token = parts.next().ok_or(RangeSpecError::MissingRhs)?;
    let suffix_token = parts.next();
    if let Some(extra) = parts.next() {
        return Err(RangeSpecError::ExtraTokens(extra.to_string()));
    }

    match parse_u64_literal(literal_token) {
        Ok(value) => {
            if let Some(suffix) = suffix_token {
                let unit = parse_unit(suffix)
                    .ok_or_else(|| RangeSpecError::UnknownUnit(suffix.to_string()))?;
                apply_unit(value, unit)
            } else {
                Ok(value)
            }
        }
        Err(err) => {
            if let Some(_suffix) = suffix_token {
                return Err(RangeSpecError::LengthLiteral(err));
            }
            if let Some((literal, suffix)) = strip_appended_suffix(literal_token) {
                let value = parse_u64_literal(literal).map_err(RangeSpecError::LengthLiteral)?;
                let unit = parse_unit(suffix)
                    .ok_or_else(|| RangeSpecError::UnknownUnit(suffix.to_string()))?;
                apply_unit(value, unit)
            } else {
                Err(RangeSpecError::LengthLiteral(err))
            }
        }
    }
}

fn strip_appended_suffix(token: &str) -> Option<(&str, &str)> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut suffix_start = trimmed.len();
    for (idx, ch) in trimmed.char_indices().rev() {
        if ch.is_ascii_alphabetic() {
            suffix_start = idx;
        } else {
            break;
        }
    }
    if suffix_start == trimmed.len() {
        return None;
    }
    let literal = trimmed[..suffix_start].trim_end();
    if literal.is_empty() {
        return None;
    }
    Some((literal, &trimmed[suffix_start..]))
}

fn apply_unit(value: u64, unit: LengthUnit) -> Result<u64, RangeSpecError> {
    let scaled = value
        .checked_mul(unit.scale)
        .ok_or(RangeSpecError::LengthOverflow)?;
    match unit.kind {
        UnitKind::Bytes => Ok(scaled),
        UnitKind::Bits => {
            if scaled % 8 != 0 {
                return Err(RangeSpecError::BitLengthNotByteAligned { bits: scaled });
            }
            Ok(scaled / 8)
        }
    }
}

fn parse_unit(raw: &str) -> Option<LengthUnit> {
    let token = raw.trim();
    if token.is_empty() {
        return None;
    }
    let lower = token.to_ascii_lowercase();
    if lower == "bit" || lower == "bits" {
        return Some(LengthUnit::bits(1));
    }
    if lower == "byte" || lower == "bytes" {
        return Some(LengthUnit::bytes(1));
    }
    if token == "b" {
        return Some(LengthUnit::bits(1));
    }
    if token == "B" {
        return Some(LengthUnit::bytes(1));
    }
    match lower.as_str() {
        "kbit" | "kbits" => Some(LengthUnit::bits(KIB)),
        "mbit" | "mbits" => Some(LengthUnit::bits(MIB)),
        "gbit" | "gbits" => Some(LengthUnit::bits(GIB)),
        "kbyte" | "kbytes" => Some(LengthUnit::bytes(KIB)),
        "mbyte" | "mbytes" => Some(LengthUnit::bytes(MIB)),
        "gbyte" | "gbytes" => Some(LengthUnit::bytes(GIB)),
        _ => parse_short_unit(token),
    }
}

fn parse_short_unit(token: &str) -> Option<LengthUnit> {
    let lower = token.to_ascii_lowercase();
    match lower.as_str() {
        "kb" => Some(if token.ends_with('B') {
            LengthUnit::bytes(KIB)
        } else {
            LengthUnit::bits(KIB)
        }),
        "mb" => Some(if token.ends_with('B') {
            LengthUnit::bytes(MIB)
        } else {
            LengthUnit::bits(MIB)
        }),
        "gb" => Some(if token.ends_with('B') {
            LengthUnit::bytes(GIB)
        } else {
            LengthUnit::bits(GIB)
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exclusive_range() {
        let range = RangeSpec::parse("0x1000..0x1010").expect("parse");
        assert_eq!(range.start(), 0x1000);
        assert_eq!(range.end(), 0x1010);
        assert_eq!(range.len_bytes(), 0x10);
    }

    #[test]
    fn parses_inclusive_range() {
        let range = RangeSpec::parse("0x1000 ..= 0x100F").expect("parse");
        assert_eq!(range.len_bytes(), 0x10);
    }

    #[test]
    fn parses_length_range() {
        let range = RangeSpec::parse("0x0..+0x20").expect("parse");
        assert_eq!(range.end(), 0x20);
    }

    #[test]
    fn parses_length_with_byte_suffix() {
        let range = RangeSpec::parse("0x0..+1kB").expect("parse");
        assert_eq!(range.len_bytes(), 1024);
    }

    #[test]
    fn parses_length_with_bit_suffix() {
        let range = RangeSpec::parse("0x0..+16kb").expect("parse");
        assert_eq!(range.len_bytes(), 2048);
        assert_eq!(range.len_bits(), Some(2048 * 8));
    }

    #[test]
    fn iterator_matches_std_range() {
        let range = RangeSpec::parse("0..+4").expect("parse");
        let collected: Vec<u64> = range.into_iter().collect();
        assert_eq!(collected, vec![0, 1, 2, 3]);
    }

    #[test]
    fn rejects_misaligned_bits() {
        let err = RangeSpec::parse("0..+3bit").unwrap_err();
        assert!(matches!(
            err,
            RangeSpecError::BitLengthNotByteAligned { .. }
        ));
    }

    #[test]
    fn rejects_inverted_bounds() {
        let err = RangeSpec::parse("0x20..0x10").unwrap_err();
        assert!(matches!(err, RangeSpecError::InvalidOrdering { .. }));
    }
}
