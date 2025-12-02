//! Collection of optional helpers layered on top of `DataHandle` so consumers can opt-in to higher level bus semantics.

pub mod crypto;
pub mod float;
pub mod signed;
pub mod leb128;
pub mod string;
pub mod string_repr;

pub use crypto::CryptoDataViewExt;
pub use float::FloatDataViewExt;
pub use signed::SignedDataViewExt;
pub use leb128::Leb128DataViewExt;
pub use string::StringDataViewExt;
pub use string_repr::StringReprDataViewExt;