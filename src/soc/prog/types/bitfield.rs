//! Shared bitfield metadata that captures concatenated segment descriptions used by both
//! runtime symbol traversal and instruction decoding.

use std::fmt;

use smallvec::SmallVec;

use super::scalar::ScalarStorage;

const MAX_BITFIELD_BITS: u16 = 64;

#[inline(always)]
fn mask_for_width(width: u16) -> u64 {
    1u64.unbounded_shl(width as u32).wrapping_sub(1)
}

fn lower_bits_mask(offset: u16) -> u64 {
    if offset == 0 {
        0
    } else if offset as u32 >= 64 {
        u64::MAX
    } else {
        (1u64 << offset) - 1
    }
}

fn prefix_popcount(mask: u64, offset: u16) -> u16 {
    (mask & lower_bits_mask(offset)).count_ones() as u16
}

#[cfg(target_arch = "x86_64")]
mod bmi2 {
    #[inline]
    pub fn pext(value: u64, mask: u64) -> Option<u64> {
        if mask == 0 {
            return Some(0);
        }
        if std::arch::is_x86_feature_detected!("bmi2") {
            unsafe { Some(core::arch::x86_64::_pext_u64(value, mask)) }
        } else {
            None
        }
    }

    #[inline]
    pub fn pdep(value: u64, mask: u64) -> Option<u64> {
        if mask == 0 {
            return Some(0);
        }
        if std::arch::is_x86_feature_detected!("bmi2") {
            unsafe { Some(core::arch::x86_64::_pdep_u64(value, mask)) }
        } else {
            None
        }
    }
}

#[cfg(not(target_arch = "x86_64"))]
mod bmi2 {
    pub fn pext(_: u64, _: u64) -> Option<u64> {
        None
    }

    pub fn pdep(_: u64, _: u64) -> Option<u64> {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BitSlice {
    pub offset: u16,
    pub width: u8,
    pub mask: u64,
}

impl BitSlice {
    pub fn new(offset: u16, width: u16) -> Result<Self, BitFieldError> {
        if width == 0 {
            return Err(BitFieldError::ZeroWidthSlice);
        }
        if width > MAX_BITFIELD_BITS {
            return Err(BitFieldError::SliceTooWide { width });
        }
        if offset >= MAX_BITFIELD_BITS || offset + width > MAX_BITFIELD_BITS {
            return Err(BitFieldError::SliceOutOfRange { offset, width });
        }
        Ok(Self {
            offset,
            width: width as u8,
            mask: mask_for_width(width) << offset,
        })
    }
}

/// Ordered segments that contribute bits to the final bitfield value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BitFieldSegment {
    /// Extracts bits from the container using a precomputed mask/shift pair.
    Slice(BitSlice),
    /// Appends a literal value with the provided bit width.
    Literal { value: u64, width: u8 },
}

/// Indicates how many bits should be prepended after the explicit segments are evaluated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadKind {
    Zero,
    Sign,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PadSpec {
    pub kind: PadKind,
    pub width: u16,
}

impl PadSpec {
    pub fn new(kind: PadKind, width: u16) -> Self {
        Self { kind, width }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BitFieldSpec {
    storage: ScalarStorage,
    pub segments: SmallVec<[BitFieldSegment; 4]>,
    pub pad: Option<PadSpec>,
    pub signed: bool,
    pub mask: u64,
    slice_meta: SmallVec<[Option<SliceMeta>; 4]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SliceMeta {
    rank_start: u16,
    width: u8,
}

impl BitFieldSpec {
    pub fn builder(store_bitw: u16) -> BitFieldSpecBuilder {
        BitFieldSpecBuilder::new(ScalarStorage::for_bits(store_bitw as usize))
    }

    pub fn from_range(store_bitw: u16, offset: u16, range_bitw: u16) -> Self {
        BitFieldSpec::builder(store_bitw)
            .range(offset, range_bitw)
            .finish()
    }

    /// Parses an ISA-style bit spec string (e.g. `@(16..29|0b00)`), assuming MSB-zero numbering
    /// for ranges as described in the language specification.
    pub fn from_spec_str(store_bitw: u16, spec: &str) -> Result<Self, BitFieldError> {
        if store_bitw == 0 || store_bitw > MAX_BITFIELD_BITS {
            return Err(BitFieldError::ContainerTooWide { bits: store_bitw });
        }
        let storage = ScalarStorage::for_bits(store_bitw as usize);
        let body = extract_spec_body(spec)?;
        let mut pad_kind = None;
        let mut raw_segments = Vec::new();
        for token in body.split('|') {
            let token = token.trim();
            if token.is_empty() {
                continue;
            }
            if let Some(kind) = parse_pad(token)? {
                if pad_kind.replace(kind).is_some() {
                    return Err(BitFieldError::DuplicatePad);
                }
                continue;
            }
            if let Some((value, width)) = parse_literal(token)? {
                raw_segments.push(SegmentToken::Literal { value, width });
                continue;
            }
            let (start, end) = parse_range(token)?;
            raw_segments.push(SegmentToken::Range { start, end });
        }
        if raw_segments.is_empty() {
            return Err(BitFieldError::MissingSegments);
        }
        let mut segments = SmallVec::new();
        for token in raw_segments.into_iter() {
            match token {
                SegmentToken::Literal { value, width } => {
                    if width == 0 || width as u16 > MAX_BITFIELD_BITS {
                        return Err(BitFieldError::LiteralTooWide {
                            width: width as u16,
                        });
                    }
                    segments.push(BitFieldSegment::Literal { value, width });
                }
                SegmentToken::Range { start, end } => {
                    let (offset, width) = msb_range_to_lsb_offset(start, end, store_bitw)?;
                    let slice = BitSlice::new(offset, width)?;
                    segments.push(BitFieldSegment::Slice(slice));
                }
            }
        }
        let mut result = BitFieldSpec {
            storage,
            segments,
            pad: None,
            signed: false,
            mask: 0,
            slice_meta: SmallVec::new(),
        };
        if let Some(kind) = pad_kind {
            let data_bits = result.data_width();
            if store_bitw < data_bits {
                return Err(BitFieldError::PadExceedsContainer {
                    container_bits: store_bitw,
                    data_bits,
                });
            }
            let pad_width = store_bitw - data_bits;
            if pad_width > 0 {
                result.pad = Some(PadSpec::new(kind, pad_width));
            }
        }
        if result.total_width() > MAX_BITFIELD_BITS {
            return Err(BitFieldError::TotalWidthExceeded {
                bits: result.total_width(),
            });
        }
        result.rebuild_cache();
        Ok(result)
    }

    fn rebuild_cache(&mut self) {
        self.mask = 0;
        self.slice_meta.clear();
        self.slice_meta.reserve(self.segments.len());
        for segment in &self.segments {
            if let BitFieldSegment::Slice(slice) = segment {
                self.mask |= slice.mask;
            }
        }
        for segment in &self.segments {
            if let BitFieldSegment::Slice(slice) = segment {
                let rank = prefix_popcount(self.mask, slice.offset);
                self.slice_meta.push(Some(SliceMeta {
                    rank_start: rank,
                    width: slice.width,
                }));
            } else {
                self.slice_meta.push(None);
            }
        }
    }

    pub fn total_width(&self) -> u16 {
        self.data_width() + self.pad.map(|pad| pad.width).unwrap_or(0)
    }

    pub fn data_width(&self) -> u16 {
        self.segments
            .iter()
            .map(|segment| match segment {
                BitFieldSegment::Slice(slice) => slice.width as u16,
                BitFieldSegment::Literal { width, .. } => *width as u16,
            })
            .sum()
    }

    pub fn is_signed(&self) -> bool {
        self.signed
            || matches!(
                self.pad,
                Some(PadSpec {
                    kind: PadKind::Sign,
                    ..
                })
            )
    }

    /// Returns the inclusive bit span covered by container slices (if any).
    pub fn bit_span(&self) -> Option<(u16, u16)> {
        let mut min_bit: Option<u16> = None;
        let mut max_bit: Option<u16> = None;
        for segment in &self.segments {
            if let BitFieldSegment::Slice(slice) = segment {
                let slice_min = slice.offset;
                let slice_max = slice.offset + slice.width as u16;
                min_bit = Some(match min_bit {
                    Some(min) => min.min(slice_min),
                    None => slice_min,
                });
                max_bit = Some(match max_bit {
                    Some(max) => max.max(slice_max),
                    None => slice_max,
                });
            }
        }
        match (min_bit, max_bit) {
            (Some(min), Some(max)) => Some((min, max)),
            _ => None,
        }
    }

    /// ======================= READ ========================
    /// Reads the bitfield and interprets the result as a signed value when the spec demands it.
    ///
    /// Consumers like the semantics runtime can rely on this helper to get properly sign-extended
    /// operands without having to duplicate the padding logic baked into the spec definition.
    pub fn read_from(&self, bits: u64) -> u64 {
        let (value, width) = self.read_bits(bits);
        if width == 0 {
            return 0;
        }
        if self.is_signed() {
            let effective = width.min(64);
            let shift = 64 - effective as u32;
            // Perform sign extension
            (((value << shift) as i64) >> shift) as u64
        } else {
            value
        }
    }

    /// Extracts the logical field value from the provided container bits. Returns the value and
    /// its effective width after padding.
    pub fn read_bits(&self, bits: u64) -> (u64, u16) {
        if let Some((value, width)) = self.extract_data_parallel(bits) {
            return self.apply_pad(value, width);
        }
        let (value, width) = self.extract_data(bits);
        self.apply_pad(value, width)
    }

    fn extract_data(&self, bits: u64) -> (u64, u16) {
        let mut acc = 0u128;
        let mut acc_width: u16 = 0;
        for segment in &self.segments {
            match segment {
                BitFieldSegment::Slice(slice) => {
                    let part = ((bits & slice.mask) >> slice.offset) as u128;
                    acc = (acc << slice.width) | part;
                    acc_width += slice.width as u16;
                }
                BitFieldSegment::Literal { value, width } => {
                    let mask = mask_for_width(*width as u16) as u128;
                    acc = (acc << *width as u32) | ((*value as u128) & mask);
                    acc_width += *width as u16;
                }
            }
        }
        (acc as u64, acc_width)
    }

    fn apply_pad(&self, value: u64, width: u16) -> (u64, u16) {
        if let Some(pad) = self.pad {
            if matches!(pad.kind, PadKind::Sign) && width > 0 {
                let sign_bit = (value >> (width - 1)) & 1;
                if sign_bit == 1 {
                    let pad_mask = mask_for_width(pad.width) << width;
                    return (value | pad_mask, width + pad.width);
                }
            }
            (value, width + pad.width)
        } else {
            (value, width)
        }
    }

    /// ======================= WRITE ========================
    /// Writes the logical field value back into the container bits, returning the updated value.
    pub fn write_to(&self, mut container: u64, mut value: u64) -> Result<u64, BitFieldError> {
        let total = self.total_width();
        if total == 0 {
            return Ok(container);
        }
        if total < 64 && (value >> total) != 0 {
            return Err(BitFieldError::ValueTooWide { bits: total, total });
        }
        let data_width = self.data_width();
        if let Some(pad) = self.pad {
            let pad_mask = mask_for_width(pad.width);
            let pad_bits = value >> data_width;
            match pad.kind {
                PadKind::Zero => {
                    if pad_bits != 0 {
                        return Err(BitFieldError::PadBitsMismatch);
                    }
                }
                PadKind::Sign => {
                    if data_width > 0 {
                        let sign_bit = (value >> (data_width - 1)) & 1;
                        let expected = if sign_bit == 1 { pad_mask } else { 0 };
                        if pad_bits != expected {
                            return Err(BitFieldError::PadBitsMismatch);
                        }
                    }
                }
            }
            value &= mask_for_width(data_width);
        }
        if let Some(result) = self.try_write_parallel(container, value, data_width, total) {
            return result;
        }
        let mut remaining = data_width;
        for segment in &self.segments {
            match segment {
                BitFieldSegment::Slice(slice) => {
                    let width = slice.width as u16;
                    remaining =
                        remaining
                            .checked_sub(width)
                            .ok_or(BitFieldError::ValueTooWide {
                                bits: data_width,
                                total,
                            })?;
                    let part_mask = mask_for_width(width);
                    let part = (value >> remaining) & part_mask;
                    let cleared = container & !slice.mask;
                    let shifted = (part << slice.offset) & slice.mask;
                    container = cleared | shifted;
                }
                BitFieldSegment::Literal {
                    value: literal,
                    width: seg_width,
                } => {
                    let width = *seg_width as u16;
                    remaining =
                        remaining
                            .checked_sub(width)
                            .ok_or(BitFieldError::ValueTooWide {
                                bits: data_width,
                                total,
                            })?;
                    let mask = mask_for_width(width);
                    let part = (value >> remaining) & mask;
                    if part != (*literal & mask) {
                        return Err(BitFieldError::LiteralMismatch {
                            expected: *literal & mask,
                            actual: part,
                            width: *seg_width,
                        });
                    }
                }
            }
        }
        if remaining != 0 {
            return Err(BitFieldError::ValueTooWide {
                bits: data_width,
                total,
            });
        }
        Ok(container)
    }

    fn extract_data_parallel(&self, bits: u64) -> Option<(u64, u16)> {
        if self.mask == 0 {
            return None;
        }
        let extracted = bmi2::pext(bits, self.mask)?;
        let mut acc = 0u128;
        let mut acc_width: u16 = 0;
        for (segment, meta) in self.segments.iter().zip(self.slice_meta.iter()) {
            match (segment, meta) {
                (BitFieldSegment::Slice(_), Some(slice_meta)) => {
                    let width = slice_meta.width as u16;
                    let part_mask = mask_for_width(width);
                    let part = (extracted >> slice_meta.rank_start) & part_mask;
                    acc = (acc << width) | u128::from(part);
                    acc_width += width;
                }
                (BitFieldSegment::Literal { value, width }, _) => {
                    let width_u16 = *width as u16;
                    let mask = mask_for_width(width_u16);
                    acc = (acc << width_u16) | u128::from(*value & mask);
                    acc_width += width_u16;
                }
                (BitFieldSegment::Slice(_), None) => return None,
            }
        }
        Some((acc as u64, acc_width))
    }

    fn try_write_parallel(
        &self,
        container: u64,
        value: u64,
        data_width: u16,
        total: u16,
    ) -> Option<Result<u64, BitFieldError>> {
        if self.mask == 0 {
            return None;
        }
        let mut remaining = data_width;
        let mut deposit_bits = 0u64;
        for (segment, meta) in self.segments.iter().zip(self.slice_meta.iter()) {
            match (segment, meta) {
                (BitFieldSegment::Slice(_), Some(slice_meta)) => {
                    let width = slice_meta.width as u16;
                    remaining = remaining
                        .checked_sub(width)
                        .ok_or(BitFieldError::ValueTooWide {
                            bits: data_width,
                            total,
                        })
                        .ok()?;
                    let part_mask = mask_for_width(width);
                    let part = (value >> remaining) & part_mask;
                    deposit_bits |= part << slice_meta.rank_start;
                }
                (
                    BitFieldSegment::Literal {
                        value: literal,
                        width,
                    },
                    _,
                ) => {
                    let width_u16 = *width as u16;
                    remaining = remaining
                        .checked_sub(width_u16)
                        .ok_or(BitFieldError::ValueTooWide {
                            bits: data_width,
                            total,
                        })
                        .ok()?;
                    let mask = mask_for_width(width_u16);
                    let part = (value >> remaining) & mask;
                    if part != (*literal & mask) {
                        return Some(Err(BitFieldError::LiteralMismatch {
                            expected: *literal & mask,
                            actual: part,
                            width: *width,
                        }));
                    }
                }
                (BitFieldSegment::Slice(_), None) => return None,
            }
        }
        if remaining != 0 {
            return Some(Err(BitFieldError::ValueTooWide {
                bits: data_width,
                total,
            }));
        }
        let scatter = bmi2::pdep(deposit_bits, self.mask)?;
        let cleared = container & !self.mask;
        Some(Ok(cleared | scatter))
    }

    pub fn storage_bits(&self) -> u16 {
        self.storage.bit_size()
    }

    pub fn storage_bytes(&self) -> usize {
        self.storage.byte_size()
    }
}

pub struct BitFieldSpecBuilder {
    storage: ScalarStorage,
    segments: SmallVec<[BitFieldSegment; 4]>,
    pad: Option<PadSpec>,
    signed: bool,
}

impl BitFieldSpecBuilder {
    fn new(storage: ScalarStorage) -> Self {
        Self {
            storage,
            segments: SmallVec::new(),
            pad: None,
            signed: false,
        }
    }

    pub fn range(mut self, offset: u16, width: u16) -> Self {
        let slice = BitSlice::new(offset, width).expect("range should fit within 64 bits");
        self.segments.push(BitFieldSegment::Slice(slice));
        self
    }

    pub fn literal(mut self, value: u64, width: u8) -> Self {
        self.segments
            .push(BitFieldSegment::Literal { value, width });
        self
    }

    pub fn pad(mut self, pad: PadSpec) -> Self {
        self.pad = Some(pad);
        self
    }

    pub fn signed(mut self, signed: bool) -> Self {
        self.signed = signed;
        self
    }

    pub fn finish(mut self) -> BitFieldSpec {
        // If empty, default to a full-width slice
        if self.segments.is_empty() {
            self.segments
                .push(BitFieldSegment::Slice(BitSlice::new(0, self.storage.bit_size()).unwrap()));
        }
        let mut spec = BitFieldSpec {
            storage: self.storage,
            segments: self.segments,
            pad: self.pad,
            signed: self.signed,
            mask: 0,
            slice_meta: SmallVec::new(),
        };
        spec.rebuild_cache();
        spec
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum BitFieldError {
    EmptySpec,
    InvalidToken(String),
    InvalidNumber(String),
    InvalidLiteral(String),
    LiteralTooWide {
        width: u16,
    },
    ZeroWidthSlice,
    SliceTooWide {
        width: u16,
    },
    SliceOutOfRange {
        offset: u16,
        width: u16,
    },
    DuplicatePad,
    PadExceedsContainer {
        container_bits: u16,
        data_bits: u16,
    },
    ContainerTooWide {
        bits: u16,
    },
    TotalWidthExceeded {
        bits: u16,
    },
    MissingSegments,
    PadBitsMismatch,
    LiteralMismatch {
        expected: u64,
        actual: u64,
        width: u8,
    },
    ValueTooWide {
        bits: u16,
        total: u16,
    },
}

impl fmt::Display for BitFieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BitFieldError::EmptySpec => write!(f, "bitfield spec is empty"),
            BitFieldError::InvalidToken(tok) => write!(f, "invalid token '{tok}' in bitfield spec"),
            BitFieldError::InvalidNumber(tok) => {
                write!(f, "invalid number '{tok}' in bitfield spec")
            }
            BitFieldError::InvalidLiteral(tok) => {
                write!(f, "invalid literal '{tok}' in bitfield spec")
            }
            BitFieldError::LiteralTooWide { width } => {
                write!(f, "literal width {width} exceeds u64")
            }
            BitFieldError::ZeroWidthSlice => write!(f, "slice width must be non-zero"),
            BitFieldError::SliceTooWide { width } => write!(f, "slice width {width} exceeds limit"),
            BitFieldError::SliceOutOfRange { offset, width } => {
                write!(
                    f,
                    "slice at offset {offset} width {width} exceeds 64-bit container"
                )
            }
            BitFieldError::DuplicatePad => write!(f, "multiple pad directives in bitfield spec"),
            BitFieldError::PadExceedsContainer {
                container_bits,
                data_bits,
            } => write!(
                f,
                "pad directive exceeds container size (container={container_bits}, data={data_bits})"
            ),
            BitFieldError::ContainerTooWide { bits } => {
                write!(f, "container width {bits} exceeds supported 64-bit limit")
            }
            BitFieldError::TotalWidthExceeded { bits } => {
                write!(f, "bitfield total width {bits} exceeds 64-bit accumulator")
            }
            BitFieldError::MissingSegments => {
                write!(f, "bitfield spec does not contain any segments")
            }
            BitFieldError::PadBitsMismatch => {
                write!(f, "pad bits do not match the requested padding")
            }
            BitFieldError::LiteralMismatch {
                expected,
                actual,
                width,
            } => write!(
                f,
                "literal segment mismatch: expected {expected:#x} != provided {actual:#x} (width {width})"
            ),
            BitFieldError::ValueTooWide { bits, total } => write!(
                f,
                "value does not fit within {total} bits (requires {bits} bits)"
            ),
        }
    }
}

impl std::error::Error for BitFieldError {}

#[derive(Debug)]
enum SegmentToken {
    Literal { value: u64, width: u8 },
    Range { start: u16, end: u16 },
}

fn extract_spec_body(spec: &str) -> Result<&str, BitFieldError> {
    let trimmed = spec.trim();
    if let Some(rest) = trimmed.strip_prefix("@(") {
        return rest
            .strip_suffix(')')
            .ok_or_else(|| BitFieldError::InvalidToken(trimmed.to_string()));
    }
    if let Some(rest) = trimmed.strip_prefix('(') {
        return rest
            .strip_suffix(')')
            .ok_or_else(|| BitFieldError::InvalidToken(trimmed.to_string()));
    }
    Ok(trimmed)
}

fn parse_pad(token: &str) -> Result<Option<PadKind>, BitFieldError> {
    if let Some(rest) = token.strip_prefix('?') {
        return match rest.trim() {
            "0" => Ok(Some(PadKind::Zero)),
            "1" => Ok(Some(PadKind::Sign)),
            _ => Err(BitFieldError::InvalidToken(token.to_string())),
        };
    }
    Ok(None)
}

fn parse_literal(token: &str) -> Result<Option<(u64, u8)>, BitFieldError> {
    if let Some(rest) = token.strip_prefix("0b") {
        let bits = rest.trim();
        if bits.is_empty() {
            return Err(BitFieldError::InvalidLiteral(token.to_string()));
        }
        let width = bits.len();
        if width > 64 {
            return Err(BitFieldError::LiteralTooWide {
                width: width as u16,
            });
        }
        let value = u64::from_str_radix(bits, 2)
            .map_err(|_| BitFieldError::InvalidLiteral(token.to_string()))?;
        return Ok(Some((value, width as u8)));
    }
    Ok(None)
}

fn parse_range(token: &str) -> Result<(u16, u16), BitFieldError> {
    let trimmed = token.trim();
    if let Some((start, end)) = trimmed.split_once("..") {
        if start.trim().is_empty() || end.trim().is_empty() {
            return Err(BitFieldError::InvalidToken(trimmed.to_string()));
        }
        let start = parse_number(start.trim())?;
        let end = parse_number(end.trim())?;
        if end < start {
            return Err(BitFieldError::InvalidToken(trimmed.to_string()));
        }
        Ok((start, end))
    } else {
        let bit = parse_number(trimmed)?;
        Ok((bit, bit))
    }
}

fn parse_number(token: &str) -> Result<u16, BitFieldError> {
    if token.is_empty() {
        return Err(BitFieldError::InvalidNumber(token.to_string()));
    }
    let value = if let Some(rest) = token.strip_prefix("0x") {
        u16::from_str_radix(rest, 16)
    } else if let Some(rest) = token.strip_prefix("0o") {
        u16::from_str_radix(rest, 8)
    } else if let Some(rest) = token.strip_prefix("0b") {
        u16::from_str_radix(rest, 2)
    } else {
        token.parse::<u16>()
    };
    value.map_err(|_| BitFieldError::InvalidNumber(token.to_string()))
}

fn msb_range_to_lsb_offset(
    start: u16,
    end: u16,
    container_bits: u16,
) -> Result<(u16, u16), BitFieldError> {
    if start >= container_bits || end >= container_bits {
        return Err(BitFieldError::SliceOutOfRange {
            offset: start,
            width: end - start + 1,
        });
    }
    let width = end - start + 1;
    let lsb_offset = container_bits - 1 - end;
    Ok((lsb_offset, width))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_range_creates_single_segment() {
        let spec = BitFieldSpec::from_range(8, 4, 5);
        assert_eq!(
            spec.storage_bytes(),
            1,
            "bitfield should remember storage selection"
        );
        assert_eq!(
            spec.total_width(),
            5,
            "range width should propagate to total width"
        );
        assert_eq!(
            spec.segments.len(),
            1,
            "from_range should create exactly one segment"
        );
        assert!(
            matches!(
                spec.segments[0],
                BitFieldSegment::Slice(BitSlice {
                    offset: 4,
                    width: 5,
                    ..
                })
            ),
            "builder should wrap range as slice"
        );
        assert!(!spec.is_signed(), "from_range defaults to unsigned result");
    }

    #[test]
    fn builder_accumulates_literals_and_padding() {
        let spec = BitFieldSpec::builder(16)
            .range(0, 4)
            .literal(0b101, 3)
            .pad(PadSpec::new(PadKind::Zero, 2))
            .signed(true)
            .finish();
        assert_eq!(
            spec.segments.len(),
            2,
            "builder should record both range and literal segments"
        );
        assert_eq!(spec.total_width(), 9, "total width includes padding bits");
        assert!(
            spec.is_signed(),
            "explicit signed flag should mark spec as signed"
        );
        assert_eq!(spec.storage_bytes(), 2, "builder should retain storage");
    }

    #[test]
    fn sign_padding_marks_spec_signed() {
        let spec = BitFieldSpec::builder(16)
            .range(3, 4)
            .pad(PadSpec::new(PadKind::Sign, 4))
            .finish();
        assert!(
            spec.is_signed(),
            "sign padding should imply signed result even without explicit flag"
        );
        assert_eq!(spec.total_width(), 8, "padding contributes to total width");
    }

    #[test]
    fn parses_spec_with_literals_and_pad() {
        let spec = BitFieldSpec::from_spec_str(32, "@(16..29|0b00)").expect("spec parse");
        assert_eq!(
            spec.data_width(),
            16,
            "range and literal widths should be accumulated"
        );
        assert!(
            spec.pad.is_none(),
            "spec without pad directive should not infer pad"
        );
    }

    #[test]
    fn parses_sign_pad_spec() {
        let spec = BitFieldSpec::from_spec_str(32, "@(?1|16..29|0b00)").expect("spec parse");
        assert!(
            matches!(
                spec.pad,
                Some(PadSpec {
                    kind: PadKind::Sign,
                    width: 16
                })
            ),
            "sign pad should consume remaining bits"
        );
        assert!(
            spec.is_signed(),
            "sign pad should imply signed interpretation"
        );
    }

    #[test]
    fn read_and_write_round_trip() {
        let spec = BitFieldSpec::builder(16)
            .range(0, 3)
            .literal(0b01, 2)
            .finish();
        assert_eq!(
            spec.segments.len(),
            2,
            "spec should contain range and literal segments"
        );
        let bits = 0b111101u64;
        let (value, width) = spec.read_bits(bits);
        assert_eq!(value, 0b10101, "Should be interpreted as @(0..2|0b01");
        assert_eq!(width, 5, "total width should include literal segment");
        let updated = spec.write_to(0, value).expect("write ok");
        assert_eq!(
            updated,
            bits & mask_for_width(3),
            "only range bits should be written back"
        );
    }

    #[test]
    fn read_zero_extends_unsigned_specs() {
        let spec = BitFieldSpec::builder(16).range(4, 4).finish();
        let bits = 0x00F0u64; // field is 0xF -> 15
        let signed = spec.read_from(bits);
        assert_eq!(signed, 15, "unsigned specs should zero-extend their values");
    }

    #[test]
    fn read_signed_sign_extends_when_requested() {
        let spec = BitFieldSpec::builder(16)
            .range(4, 4)
            .pad(PadSpec::new(PadKind::Sign, 60))
            .finish();
        let bits = 0x00F0u64; // field is 0xF -> -1 when sign extended
        let signed = spec.read_from(bits) as i64;
        assert_eq!(
            signed, -1,
            "sign-padded specs should return negative values"
        );
    }

    #[test]
    fn spec_reports_slice_mask() {
        let spec = BitFieldSpec::builder(16).range(0, 4).range(8, 4).finish();
        assert_eq!(spec.mask, 0x0F0F, "mask should union all slice bits");
    }
}
