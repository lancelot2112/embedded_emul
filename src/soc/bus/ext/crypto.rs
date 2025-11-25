//! Lightweight helpers for reading simple cryptographic primitives.

use crate::soc::bus::{BusResult, DataHandle, ext::stream::ByteDataHandleExt};
use sha2::{Digest, Sha256};

pub trait CryptoDataHandleExt {
    fn read_sha256(&mut self, length: usize) -> BusResult<[u8; 32]>;
}

impl CryptoDataHandleExt for DataHandle {
    fn read_sha256(&mut self, length: usize) -> BusResult<[u8; 32]> {
        let mut buffer = vec![0u8; length];
        self.stream_out(&mut buffer)?;
        let digest = Sha256::digest(&buffer);
        let mut array = [0u8; 32];
        array.copy_from_slice(&digest);
        Ok(array)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::{BasicMemory, Device, Endianness};
    use crate::soc::bus::DeviceBus;
    use hex_literal::hex;
    use std::sync::Arc;

    fn make_handle(bytes: &[u8]) -> DataHandle {
        let bus = Arc::new(DeviceBus::new(8));
        let memory = Arc::new(BasicMemory::new("rom", 0x40, Endianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();
        memory.write(0, bytes).unwrap();
        let mut handle = DataHandle::new(bus);
        handle.address_mut().jump(0).unwrap();
        handle
    }

    #[test]
    fn sha256_matches_known_vector() {
        let mut handle = make_handle(b"abc");
        let digest = handle.read_sha256(3).expect("hash");
        assert_eq!(
            digest,
            hex!("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }
}
