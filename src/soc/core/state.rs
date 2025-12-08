use std::{collections::HashMap, sync::Arc};

use crate::soc::bus::ext::BitsCursorExt;
use crate::soc::bus::{BusCursor, BusError, DeviceBus};
use crate::soc::core::specification::{CoreSpec, RegisterSpec};
use crate::soc::device::{AccessContext, RamMemory};
/// Comprehensive processor snapshot referencing a local device bus so higher
/// layers can reuse the existing data-handle abstractions for bitfield access.
pub struct CoreState {
    spec: Arc<CoreSpec>,
    register_layout: HashMap<String, RegisterLayout>,
    cursor: BusCursor,
}

impl CoreState {
    pub fn new(spec: Arc<CoreSpec>) -> StateResult<Self> {
        let byte_len = align_byte_len(spec.byte_len());
        // Registers are modeled as bit slices with LSB-indexed offsets regardless of the
        // architecture's external byte order, so keep their backing store little endian to
        // make bitfield access predictable.
        let memory = RamMemory::new(
            format!("{}_state", spec.name()),
            byte_len,
            crate::soc::device::Endianness::native(),
        );
        let mut bus = DeviceBus::new(32);
        bus.map_device(memory, 0, 0)?;
        let cursor = BusCursor::attach_to_bus(Arc::new(bus), 0, AccessContext::CPU);
        let registers = spec
            .registers()
            .iter()
            .map(|spec| (spec.name.clone(), RegisterLayout::from_spec(spec)))
            .collect();
        Ok(Self {
            spec,
            register_layout: registers,
            cursor,
        })
    }

    pub fn specification(&self) -> &CoreSpec {
        &self.spec
    }

    pub fn register_layout(&self, name: &str) -> Option<RegisterLayout> {
        self.register_layout.get(name).copied()
    }

    pub fn read_register(&mut self, name: &str) -> StateResult<u64> {
        let layout = self
            .register_layout
            .get(name)
            .copied()
            .ok_or_else(|| StateError::UnknownRegister(name.to_string()))?;
        let bit_len = narrow_bit_len(name, layout.bit_len as u32)?;
        self.read_bits_at(layout.byte_offset, layout.bit_offset, bit_len)
    }

    pub fn write_register(&mut self, name: &str, value: u64) -> StateResult<()> {
        let layout = self
            .register_layout
            .get(name)
            .copied()
            .ok_or_else(|| StateError::UnknownRegister(name.to_string()))?;
        let bit_len = narrow_bit_len(name, layout.bit_len as u32)?;
        self.write_bits_at(layout.byte_offset, layout.bit_offset, bit_len, value)
    }

    pub fn read_bits_at(
        &mut self,
        byte_offset: usize,
        bit_offset: u8,
        bit_len: u16,
    ) -> StateResult<u64> {
        self.cursor.goto(byte_offset)?;
        let value = self.cursor.read_bits(bit_offset, bit_len as usize)?;
        Ok(value as u64)
    }

    pub fn write_bits_at(
        &mut self,
        byte_offset: usize,
        bit_offset: u8,
        bit_len: u16,
        value: u64,
    ) -> StateResult<()> {
        self.cursor.goto(byte_offset)?;
        let masked = mask_to_width(value, bit_len);
        self.cursor
            .write_bits(bit_offset, bit_len as usize, masked as u128)?;
        Ok(())
    }

    pub fn zeroize(&mut self) -> StateResult<()> {
        self.cursor.goto(0)?;
        let buffer = vec![0u8; self.spec.byte_len()];
        self.cursor.write_ram(&buffer)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisterLayout {
    pub byte_offset: usize,
    pub bit_offset: u8,
    pub bit_len: u16,
}

impl RegisterLayout {
    fn from_spec(spec: &RegisterSpec) -> Self {
        Self {
            byte_offset: (spec.bit_offset / 8) as usize,
            bit_offset: (spec.bit_offset % 8) as u8,
            bit_len: spec.bit_len as u16,
        }
    }
}

#[derive(Debug)]
pub enum StateError {
    Bus(BusError),
    UnknownRegister(String),
    RegisterWidthOverflow { register: String, bits: u32 },
}

pub type StateResult<T> = Result<T, StateError>;

impl std::fmt::Display for StateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateError::Bus(err) => write!(f, "bus error: {err}"),
            StateError::UnknownRegister(name) => write!(f, "unknown register '{name}'"),
            StateError::RegisterWidthOverflow { register, bits } => {
                write!(
                    f,
                    "register '{register}' width {bits} exceeds bus slice limit"
                )
            }
        }
    }
}

impl std::error::Error for StateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StateError::Bus(err) => Some(err),
            StateError::UnknownRegister(_) => None,
            StateError::RegisterWidthOverflow { .. } => None,
        }
    }
}

impl From<BusError> for StateError {
    fn from(err: BusError) -> Self {
        StateError::Bus(err)
    }
}

impl From<std::io::Error> for StateError {
    fn from(err: std::io::Error) -> Self {
        StateError::Bus(BusError::DeviceFault {
            device: "core_state".into(),
            source: Box::new(err),
        })
    }
}

// Pads the snapshot buffer so 64-bit chunked bus accesses never cross the
// backing allocation.
fn align_byte_len(len: usize) -> usize {
    if len == 0 {
        8
    } else {
        len.div_ceil(8) * 8
    }
}

fn narrow_bit_len(name: &str, bits: u32) -> StateResult<u16> {
    u16::try_from(bits).map_err(|_| StateError::RegisterWidthOverflow {
        register: name.to_string(),
        bits,
    })
}

fn mask_to_width(value: u64, width: u16) -> u64 {
    if width == 0 {
        0
    } else if width >= 64 {
        value
    } else {
        let mask = (1u64 << width) - 1;
        value & mask
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soc::device::Endianness;

    fn demo_spec() -> Arc<CoreSpec> {
        Arc::new(
            CoreSpec::builder("demo", Endianness::Little)
                .register("pc", 64)
                .register("sp", 64)
                .register("flags", 8)
                .build()
                .expect("descriptor"),
        )
    }

    #[test]
    fn register_round_trip() {
        let descriptor = demo_spec();
        let mut state = CoreState::new(descriptor).expect("core state");
        state.write_register("pc", 0xDEADBEEF).expect("write pc");
        let value = state.read_register("pc").expect("read pc");
        assert_eq!(value, 0xDEADBEEF);
    }

    #[test]
    fn register_layout_exposes_offsets() {
        let descriptor = demo_spec();
        let state = CoreState::new(descriptor).expect("core state");
        let pc = state.register_layout("pc").expect("pc layout");
        assert_eq!(pc.byte_offset, 0);
        assert_eq!(pc.bit_len, 64);
    }

    #[test]
    fn states_share_descriptor_without_aliasing_memory() {
        let descriptor = demo_spec();
        let mut first = CoreState::new(descriptor.clone()).expect("first");
        let mut second = CoreState::new(descriptor).expect("second");
        first.write_register("pc", 0x1).expect("write first");
        let second_value = second.read_register("pc").expect("read second");
        assert_eq!(second_value, 0, "independent states keep isolated memory");
    }

    #[test]
    fn compact_register_slices_do_not_overlap() {
        let descriptor = Arc::new(
            CoreSpec::builder("cr", Endianness::Big)
                .register("reg::CR0", 4)
                .register("reg::CR1", 4)
                .build()
                .expect("descriptor"),
        );
        let mut state = CoreState::new(descriptor).expect("core state");

        state.write_register("reg::CR0", 0xF).expect("write cr0");
        state.write_register("reg::CR1", 0x5).expect("write cr1");

        let cr0 = state.read_register("reg::CR0").expect("read cr0");
        let cr1 = state.read_register("reg::CR1").expect("read cr1");

        assert_eq!(cr0, 0xF, "cr0 retains its 4-bit value");
        assert_eq!(cr1, 0x5, "cr1 value stays isolated");
    }

    #[test]
    fn cr_register_file_round_trip() {
        let mut builder = CoreSpec::builder("cr_file", Endianness::Big);
        for idx in 0..8 {
            builder = builder.register(format!("reg::CR{idx}"), 4);
        }
        let descriptor = Arc::new(builder.build().expect("descriptor"));
        let mut state = CoreState::new(descriptor).expect("core state");

        for idx in 0..8u8 {
            let name = format!("reg::CR{idx}");
            state
                .write_register(&name, (idx as u64) & 0xF)
                .expect("write cr slice");
        }

        for idx in 0..8u8 {
            let name = format!("reg::CR{idx}");
            let value = state.read_register(&name).expect("read cr slice");
            assert_eq!(value, (idx as u64) & 0xF, "cr slice {idx} retains nibble");
        }

        let cr0 = state.register_layout("reg::CR0").expect("cr0 layout");
        let packed = state
            .read_bits_at(cr0.byte_offset, cr0.bit_offset, 32)
            .expect("read cr block");
        assert_eq!(packed, 0x7654_3210, "packed CR image matches nibble order");
    }
}
