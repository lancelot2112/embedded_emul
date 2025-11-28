#[path = "device.rs"]
mod device_trait;
pub mod endianness;
pub mod error;
pub mod ram;
pub mod mmio;

pub use device_trait::Device;
pub use endianness::Endianness;
pub use error::{DeviceError, DeviceResult};
pub use ram::RamMemory;
