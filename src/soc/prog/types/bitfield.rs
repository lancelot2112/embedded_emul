//! Shared bitfield metadata that captures concatenated segment descriptions used by both
//! runtime symbol traversal and instruction decoding.

use smallvec::SmallVec;

use super::arena::TypeId;

/// Ordered segments that contribute bits to the final bitfield value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BitFieldSegment {
    /// Extracts `width` bits starting at the given LSB offset inside the container.
    Range { offset: u16, width: u16 },
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
    pub container: TypeId,
    pub segments: SmallVec<[BitFieldSegment; 4]>,
    pub pad: Option<PadSpec>,
    pub signed: bool,
}

impl BitFieldSpec {
    pub fn builder(container: TypeId) -> BitFieldSpecBuilder {
        BitFieldSpecBuilder::new(container)
    }

    pub fn from_range(container: TypeId, offset: u16, width: u16) -> Self {
        let mut segments = SmallVec::new();
        segments.push(BitFieldSegment::Range { offset, width });
        Self {
            container,
            segments,
            pad: None,
            signed: false,
        }
    }

    pub fn total_width(&self) -> u16 {
        let mut width = 0u16;
        for segment in &self.segments {
            width += match segment {
                BitFieldSegment::Range { width, .. } => *width,
                BitFieldSegment::Literal { width, .. } => *width as u16,
            };
        }
        if let Some(pad) = self.pad {
            width += pad.width;
        }
        width
    }

    pub fn is_signed(&self) -> bool {
        self.signed || matches!(self.pad, Some(PadSpec { kind: PadKind::Sign, .. }))
    }
}

pub struct BitFieldSpecBuilder {
    container: TypeId,
    segments: SmallVec<[BitFieldSegment; 4]>,
    pad: Option<PadSpec>,
    signed: bool,
}

impl BitFieldSpecBuilder {
    fn new(container: TypeId) -> Self {
        Self {
            container,
            segments: SmallVec::new(),
            pad: None,
            signed: false,
        }
    }

    pub fn range(mut self, offset: u16, width: u16) -> Self {
        self.segments.push(BitFieldSegment::Range { offset, width });
        self
    }

    pub fn literal(mut self, value: u64, width: u8) -> Self {
        self.segments.push(BitFieldSegment::Literal { value, width });
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

    pub fn finish(self) -> BitFieldSpec {
        BitFieldSpec {
            container: self.container,
            segments: self.segments,
            pad: self.pad,
            signed: self.signed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_container(index: usize) -> TypeId {
        TypeId::from_index(index)
    }

    #[test]
    fn from_range_creates_single_segment() {
        let container = dummy_container(0);
        let spec = BitFieldSpec::from_range(container, 4, 5);
        assert_eq!(spec.container, container, "bitfield should remember container id");
        assert_eq!(spec.total_width(), 5, "range width should propagate to total width");
        assert_eq!(spec.segments.len(), 1, "from_range should create exactly one segment");
        assert!(matches!(spec.segments[0], BitFieldSegment::Range { offset: 4, width: 5 }));
        assert!(!spec.is_signed(), "from_range defaults to unsigned result");
    }

    #[test]
    fn builder_accumulates_literals_and_padding() {
        let container = dummy_container(1);
        let spec = BitFieldSpec::builder(container)
            .range(0, 4)
            .literal(0b101, 3)
            .pad(PadSpec::new(PadKind::Zero, 2))
            .signed(true)
            .finish();
        assert_eq!(spec.segments.len(), 2, "builder should record both range and literal segments");
        assert_eq!(spec.total_width(), 9, "total width includes padding bits");
        assert!(spec.is_signed(), "explicit signed flag should mark spec as signed");
        assert_eq!(spec.container, container, "builder should retain provided container id");
    }

    #[test]
    fn sign_padding_marks_spec_signed() {
        let container = dummy_container(2);
        let spec = BitFieldSpec::builder(container)
            .range(3, 4)
            .pad(PadSpec::new(PadKind::Sign, 4))
            .finish();
        assert!(spec.is_signed(), "sign padding should imply signed result even without explicit flag");
        assert_eq!(spec.total_width(), 8, "padding contributes to total width");
    }
}
