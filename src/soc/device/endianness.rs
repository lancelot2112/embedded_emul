//! Endianness handling.
#[cfg_attr(not(test), allow(dead_code))]
pub const MAX_ENDIAN_BYTES: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Endianness {
    Little,
    Big,
}

impl Endianness {
    #[inline(always)]
    pub const fn native() -> Self {
        if cfg!(target_endian = "little") {
            Endianness::Little
        } else {
            Endianness::Big
        }
    }

    #[inline(always)]
    pub fn to_native_mut(self, bytes: &mut [u8]) {
        assert!(bytes.len() <= MAX_ENDIAN_BYTES, "value exceeds 128 bits");
        if bytes.len() <= 1{
            return;
        }
        match self {
            Endianness::Little => {
                if cfg!(target_endian = "big") {
                    bytes.reverse();
                }
            }
            Endianness::Big => {
                if cfg!(target_endian = "little") {
                    bytes.reverse();
                } 
            }
        }
    }

    #[inline(always)]
    pub fn from_native_mut(self, bytes: &mut [u8]) {
        assert!(bytes.len() <= MAX_ENDIAN_BYTES, "value exceeds 128 bits");
        if bytes.len() <= 1 {
            return;
        }
        match self {
            Endianness::Little => {
                if cfg!(target_endian = "big") {
                    bytes.reverse();
                }
            }
            Endianness::Big => {
                if cfg!(target_endian = "little") {
                    bytes.reverse();
                } 
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Endianness;
    #[test]
    fn endianness_conversion_round_trip() {
        let mut data: [u8; 8] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let little = Endianness::Little;
        let big = Endianness::Big;
        little.to_native_mut(&mut data);
        little.from_native_mut(&mut data);
        assert_eq!(data, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08], "little endian round trip failed");

        big.to_native_mut(&mut data);
        big.from_native_mut(&mut data);
        assert_eq!(data, [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08], "big endian round trip failed");

        let mut data: [u8; 3] = [0x01, 0x02, 0x03];
        little.to_native_mut(&mut data);
        little.from_native_mut(&mut data);
        assert_eq!(data, [0x01, 0x02, 0x03], "little endian round trip failed");

        big.to_native_mut(&mut data);
        big.from_native_mut(&mut data);
        assert_eq!(data, [0x01, 0x02, 0x03], "big endian round trip failed");
    }

    #[test]
    fn endianness_conversion_different() {
        let mut data: [u8; 4] = [0x0A, 0x0B, 0x0C, 0x0D];
        let endian = if cfg!(target_endian = "little") {
            Endianness::Big
        } else {
            Endianness::Little
        };

        endian.to_native_mut(&mut data);
        assert_eq!(data, [0x0D, 0x0C, 0x0B, 0x0A], "endianness conversion failed");

        endian.from_native_mut(&mut data);
        assert_eq!(data, [0x0A, 0x0B, 0x0C, 0x0D], "endianness reverse conversion failed");
    }

    #[test]
    fn endianness_conversion_same() {
        let mut data: [u8; 4] = [0x0A, 0x0B, 0x0C, 0x0D];
        let endian = if cfg!(target_endian = "little") {
            Endianness::Little
        } else {
            Endianness::Big
        };

        endian.to_native_mut(&mut data);
        assert_eq!(data, [0x0A, 0x0B, 0x0C, 0x0D], "endianness conversion failed");

        endian.from_native_mut(&mut data);
        assert_eq!(data, [0x0A, 0x0B, 0x0C, 0x0D], "endianness reverse conversion failed");
    }

}