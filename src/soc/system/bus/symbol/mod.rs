//! Symbol-aware handle that bridges the program `SymbolTable` with device bus access so tools can
//! read memory through symbolic context.

mod cursor;
mod handle;
mod read;
mod size;
mod value;

pub use cursor::{SymbolValueCursor, SymbolWalkRead};
pub use handle::SymbolHandle;
pub use value::{SymbolAccessError, SymbolValue};

#[cfg(test)]
mod tests;
