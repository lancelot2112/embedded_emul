#[path = "device.rs"]
mod device_trait;
pub mod endianness;
pub mod error;
pub mod memory;

pub use device_trait::Device;
pub use endianness::Endianness;
pub use error::{DeviceError, DeviceResult};
pub use memory::BasicMemory;
