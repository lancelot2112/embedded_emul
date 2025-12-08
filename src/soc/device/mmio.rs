pub struct BitFieldSpec {
    pub name: String,
    pub bit_offset: u8, // Bit offset within the register
    pub bit_len: u8,    // Bit length of the sub-field
}

impl BitFieldSpec {
    pub fn from_msb0_range(
        name: impl Into<String>,
        register_bit_len: u8,
        msb0_range: std::ops::Range<u8>,
    ) -> Self {
        let name = name.into();
        let bit_offset = register_bit_len - msb0_range.end;
        let bit_len = msb0_range.end - msb0_range.start;
        Self {
            name,
            bit_offset,
            bit_len,
        }
    }

    pub fn from_lsb0_range(name: impl Into<String>, lsb0_range: std::ops::Range<u8>) -> Self {
        let name = name.into();
        let bit_offset = lsb0_range.start;
        let bit_len = lsb0_range.end - lsb0_range.start;
        Self {
            name,
            bit_offset,
            bit_len,
        }
    }

    #[inline(always)]
    pub fn mask(&self) -> u64 {
        if self.bit_len >= 64 {
            u64::MAX
        } else {
            (1u64 << self.bit_len) - 1
        }
    }

    #[inline(always)]
    pub fn read_from(&self, value: u64) -> u64 {
        (value >> self.bit_offset) & self.mask()
    }

    #[inline(always)]
    pub fn write_to(&self, original: u64, field_value: u64) -> u64 {
        let cleared = original & !(self.mask() << self.bit_offset);
        let shifted = (field_value & self.mask()) << self.bit_offset;
        cleared | shifted
    }
}
pub struct RegSpec {
    pub name: String,
    pub offset: usize,        // Byte offset within MMIO region
    pub count: Option<usize>, // Number of consecutive registers (for arrays)
    pub bit_len: u8,          //Total bit length of each register
}
