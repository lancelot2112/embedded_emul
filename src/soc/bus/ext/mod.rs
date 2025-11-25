//! Collection of optional helpers layered on top of `DataHandle` so consumers can opt-in to higher level bus semantics.

pub mod crypto;
pub mod float;
pub mod int;
pub mod bits;
pub mod leb128;
pub mod stream;
pub mod string;
pub mod string_repr;

pub use crypto::CryptoDataHandleExt;
pub use float::FloatDataHandleExt;
pub use int::IntDataHandleExt;
pub use bits::BitDataHandleExt;
pub use leb128::Leb128DataHandleExt;
pub use stream::{ByteDataHandleExt, DataStream};
pub use string::StringDataHandleExt;
pub use string_repr::StringReprDataHandleExt;
