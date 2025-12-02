//! Lightweight helpers for reading simple cryptographic primitives.

use crate::soc::bus::{BusResult, DataView};
use sha2::{Digest, Sha256};

pub trait CryptoDataViewExt {
    fn calc_sha256(&mut self, len: usize) -> BusResult<[u8; 32]>;
}

impl CryptoDataViewExt for DataView {
    fn calc_sha256(&mut self, len: usize) -> BusResult<[u8; 32]> {
        let mut buffer = vec![0u8; len];
        self.read(&mut buffer)?;
        let digest = Sha256::digest(&buffer);
        let mut array = [0u8; 32];
        array.copy_from_slice(&digest);
        Ok(array)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::bus::{DeviceBus};
    use crate::soc::device::{AccessContext, Device, Endianness, RamMemory};
    use hex_literal::hex;

    fn make_handle(bytes: &[u8]) -> DataView {
        let mut bus = DeviceBus::new();
        let mut memory = RamMemory::new("rom", 0x40, Endianness::Little);
        memory.write(0, bytes, AccessContext::DEBUG).unwrap();
        bus.map_device(memory, 0, 0).unwrap();
        
        DataView::new(bus.resolve(0).unwrap(), AccessContext::CPU)
    }

    #[test]
    fn sha256_matches_known_vector() {
        let mut view = make_handle(b"abc");
        let digest = view.calc_sha256(3).expect("hash");
        assert_eq!(
            digest,
            hex!("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }
}
