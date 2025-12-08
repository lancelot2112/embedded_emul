//! Lightweight helpers for reading simple cryptographic primitives.

use crate::soc::bus::{BusCursor, BusResult};
use sha2::{Digest, Sha256};

pub trait CryptoCursorExt {
    fn calc_sha256(&mut self, len: usize) -> BusResult<[u8; 32]>;
}

impl CryptoCursorExt for BusCursor {
    fn calc_sha256(&mut self, len: usize) -> BusResult<[u8; 32]> {
        let buffer = self.read_ram(len)?;
        let digest = Sha256::digest(buffer);
        let mut array = [0u8; 32];
        array.copy_from_slice(&digest);
        Ok(array)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::soc::bus::DeviceBus;
    use crate::soc::device::{AccessContext, Device, Endianness, RamMemory};
    use hex_literal::hex;

    fn make_cursor(bytes: &[u8]) -> BusCursor {
        let mut bus = DeviceBus::new(32);
        let memory = RamMemory::new("rom", 0x40, Endianness::Little);
        memory.write(0, bytes, AccessContext::DEBUG).unwrap();
        bus.map_device(memory, 0, 0).unwrap();

        BusCursor::attach_to_bus(Arc::new(bus), 0, AccessContext::CPU)
    }

    #[test]
    fn sha256_matches_known_vector() {
        let mut view = make_cursor(b"abc");
        let digest = view.calc_sha256(3).expect("hash");
        assert_eq!(
            digest,
            hex!("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }
}
