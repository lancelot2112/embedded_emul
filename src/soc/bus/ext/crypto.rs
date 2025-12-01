//! Lightweight helpers for reading simple cryptographic primitives.

use crate::soc::bus::{BusResult, DataTxn, data::DataView, handle::CursorBehavior};
use sha2::{Digest, Sha256};

pub trait CryptoDataViewExt {
    fn calc_sha256(&mut self, len: usize) -> BusResult<[u8; 32]>;
}

impl<C:CursorBehavior> CryptoDataViewExt for DataView<C> {
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
    use crate::soc::device::{Device, Endianness, RamMemory};
    use hex_literal::hex;
    use std::sync::Arc;

    fn make_handle(bytes: &[u8]) -> AddressHandle {
        let bus = Arc::new(DeviceBus::new(8));
        let memory = Arc::new(RamMemory::new("rom", 0x40, Endianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();
        memory.write(0, bytes).unwrap();
        let mut addr = AddressHandle::new(bus);
        addr.jump(0).unwrap();
        addr
    }

    #[test]
    fn sha256_matches_known_vector() {
        let mut addr = make_handle(b"abc");
        let digest = addr
            .data_handle(3)
            .expect("handle")
            .calc_sha256()
            .expect("hash");
        assert_eq!(
            digest,
            hex!("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }
}
