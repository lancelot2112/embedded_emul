//! Iterative cursor that walks primitive symbol values and marshals pointer/bitfield helpers.

use crate::soc::prog::symbols::walker::{SymbolWalkEntry, SymbolWalker, ValueKind};
use crate::soc::prog::types::arena::TypeArena;
use crate::soc::system::bus::{BusError, ext::stream::ByteDataHandleExt};

use super::handle::{Snapshot, SymbolHandle};
use super::read::{ReadContext, read_type_record};
use super::value::{SymbolAccessError, SymbolValue};

/// Streaming view that materialises values discovered by the `SymbolWalker` and exposes typed
/// reads/writes at each primitive leaf.
pub struct SymbolValueCursor<'handle, 'arena> {
    pub(super) handle: &'handle mut SymbolHandle<'arena>,
    pub(super) snapshot: Snapshot,
    pub(super) walker: SymbolWalker<'arena>,
    pub(super) arena: &'arena TypeArena,
}

pub struct SymbolWalkRead {
    pub entry: SymbolWalkEntry,
    pub value: SymbolValue,
    pub address: u64,
}

impl<'handle, 'arena> SymbolValueCursor<'handle, 'arena> {
    /// Returns the next primitive value in declaration order along with its formatted path.
    pub fn try_next(&mut self) -> Result<Option<SymbolWalkRead>, SymbolAccessError> {
        for entry in &mut self.walker {
            if entry.offset_bits % 8 != 0 {
                let is_bitfield = matches!(
                    self.arena.get(entry.ty),
                    crate::soc::prog::types::record::TypeRecord::BitField(_)
                );
                if !is_bitfield {
                    continue;
                }
            }
            let mut address = self.snapshot.address + (entry.offset_bits / 8);
            if let crate::soc::prog::types::record::TypeRecord::BitField(bitfield) =
                self.arena.get(entry.ty)
            {
                if let Some((min_offset, _)) = bitfield.bit_span() {
                    let total_bits = entry.offset_bits + min_offset as u64;
                    address = self.snapshot.address + (total_bits / 8);
                }
            }
            let value = self.read_entry_value(&entry, address)?;
            return Ok(Some(SymbolWalkRead {
                entry,
                value,
                address,
            }));
        }
        Ok(None)
    }

    /// Reads the pointed-to value using the metadata encoded on the pointer walk entry.
    pub fn deref(
        &mut self,
        pointer: &SymbolWalkRead,
    ) -> Result<Option<SymbolValue>, SymbolAccessError> {
        let ValueKind::Pointer { target, .. } = pointer.entry.kind else {
            return Err(SymbolAccessError::UnsupportedTraversal {
                label: self
                    .handle
                    .table
                    .resolve_label(self.snapshot.record.label)
                    .to_string(),
            });
        };
        let SymbolValue::Unsigned(address) = pointer.value else {
            return Err(SymbolAccessError::UnsupportedTraversal {
                label: self
                    .handle
                    .table
                    .resolve_label(self.snapshot.record.label)
                    .to_string(),
            });
        };
        self.handle
            .interpret_type_at(self.arena, target, address, None)
    }

    /// Writes a raw byte slice into the location described by the walk entry.
    pub fn write_bytes(
        &mut self,
        entry: &SymbolWalkEntry,
        data: &[u8],
    ) -> Result<(), SymbolAccessError> {
        let expected = entry.byte_len() as usize;
        if expected != data.len() {
            return Err(SymbolAccessError::Bus(BusError::DeviceFault {
                device: "symbol".into(),
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "byte slice length does not match field width",
                )),
            }));
        }
        let address = self.snapshot.address + (entry.offset_bits / 8);
        self.handle.data.address_mut().jump(address)?;
        self.handle.data.write_bytes(data)?;
        Ok(())
    }

    fn read_entry_value(
        &mut self,
        entry: &SymbolWalkEntry,
        address: u64,
    ) -> Result<SymbolValue, SymbolAccessError> {
        let record = self.arena.get(entry.ty);
        let mut ctx = ReadContext::new(
            &mut self.handle.data,
            self.arena,
            Some(entry),
            address,
            self.snapshot.address,
            Some(entry.byte_len()),
        );
        if let Some(value) = read_type_record(record, &mut ctx)? {
            Ok(value)
        } else {
            Err(SymbolAccessError::UnsupportedTraversal {
                label: self.symbol_label(),
            })
        }
    }

    fn symbol_label(&self) -> String {
        self.handle
            .table
            .resolve_label(self.snapshot.record.label)
            .to_string()
    }
}
