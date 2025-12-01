//! Floating point helpers layered on top of `DataHandle`.

use crate::soc::bus::{BusResult, data::DataView, handle::CursorBehavior};

pub trait FloatDataViewExt {
    fn read_f32(&mut self) -> BusResult<f32>;
    fn read_f64(&mut self) -> BusResult<f64>;
}

impl<C: CursorBehavior> FloatDataViewExt for DataView<C> {
    fn read_f32(&mut self) -> BusResult<f32> {
        let bits = self.read_u32()?;
        Ok(f32::from_bits(bits))
    }

    fn read_f64(&mut self) -> BusResult<f64> {
        let bits = self.read_u64()?;
        Ok(f64::from_bits(bits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::bus::DeviceBus;
    use crate::soc::device::{Device, Endianness as DeviceEndianness, RamMemory};
    use std::sync::Arc;

    fn make_handle(bytes: &[u8]) -> DataTxn {
        let bus = Arc::new(DeviceBus::new(8));
        let memory = Arc::new(RamMemory::new("ram", 0x20, DeviceEndianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();
        memory.write(0, bytes).unwrap();
        let mut handle = DataTxn::new(bus);
        handle.address_mut().jump(0).unwrap();
        handle
    }

    #[test]
    fn read_f32_round_trips() {
        let mut handle = make_handle(&f32::to_le_bytes(3.5));
        let value = handle.read_f32().expect("f32 read");
        assert!(
            (value - 3.5).abs() < f32::EPSILON,
            "decoded value should match original literal"
        );
    }

    #[test]
    fn read_f64_round_trips() {
        let mut handle = make_handle(&f64::to_le_bytes(-12.25));
        let value = handle.read_f64().expect("f64 read");
        assert!(
            (value + 12.25).abs() < f64::EPSILON,
            "decoded value should match original literal"
        );
    }
}
