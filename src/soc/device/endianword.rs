use crate::soc::device::Endianness;

pub trait EndianWord: Copy {
    fn to_host(self, source: Endianness) -> Self;
    fn from_host(self, target: Endianness) -> Self;
}

macro_rules! impl_word {
    ($t:ty) => {
        impl EndianWord for $t {
            #[inline(always)]
            fn to_host(self, source: Endianness) -> Self {
                match source {
                    Endianness::Little => Self::from_le(self),
                    Endianness::Big => Self::from_be(self),
                }
            }

            #[inline(always)]
            fn from_host(self, target: Endianness) -> Self {
                match target {
                    Endianness::Little => Self::to_le(self),
                    Endianness::Big => Self::to_be(self),
                }
            }
        }
    };
}

impl_word!(u8);
impl_word!(u16);
impl_word!(u32);
impl_word!(u64);
impl_word!(u128);