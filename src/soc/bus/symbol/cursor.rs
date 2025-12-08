//! Iterative cursor that walks primitive symbol values and marshals pointer/bitfield helpers.

use crate::soc::bus::BusError;
use crate::soc::prog::symbols::walker::{SymbolWalkEntry, SymbolWalker, ValueKind};
use crate::soc::prog::types::arena::TypeArena;
use crate::soc::prog::types::arena_record::TypeRecord;

use super::handle::{Snapshot, SymbolBus};
use super::read::{ReadContext, read_type_record};
use super::value::{SymbolAccessError, SymbolValue};

/// Streaming view that materialises values discovered by the `SymbolWalker` and exposes typed
/// reads/writes at each primitive leaf.
pub struct SymbolValueCursor<'handle, 'arena> {
    pub(super) handle: &'handle mut SymbolBus<'arena>,
    pub(super) snapshot: Snapshot,
    pub(super) walker: SymbolWalker<'arena>,
    pub(super) arena: &'arena TypeArena,
}

pub struct SymbolWalkRead {
    pub entry: SymbolWalkEntry,
    pub value: SymbolValue,
    pub address: usize,
}

impl<'handle, 'arena> SymbolValueCursor<'handle, 'arena> {
    /// Returns the next primitive value in declaration order along with its formatted path.
    pub fn try_next(&mut self) -> Result<Option<SymbolWalkRead>, SymbolAccessError> {
        for entry in &mut self.walker {
            if entry.offset_bits % 8 != 0 {
                let is_bitfield = matches!(self.arena.get(entry.ty), TypeRecord::BitField(_));
                if !is_bitfield {
                    continue;
                }
            }
            let address = self.snapshot.address + (entry.offset_bits / 8);
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
        let arena = self.handle.table.type_arena();
        SymbolBus::interpret_type_at(
            &mut *self.handle.cursor,
            arena.as_ref(),
            target,
            address as usize,
            None,
        )
    }

    /// Writes a raw byte slice into the location described by the walk entry.
    pub fn write_bytes(
        &mut self,
        entry: &SymbolWalkEntry,
        data: &[u8],
    ) -> Result<(), SymbolAccessError> {
        let expected = entry.byte_len();
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
        self.handle.cursor.goto(address)?.write_ram(data)?;
        Ok(())
    }

    fn read_entry_value(
        &mut self,
        entry: &SymbolWalkEntry,
        address: usize,
    ) -> Result<SymbolValue, SymbolAccessError> {
        let label = self.symbol_label();
        {
            let record = self.arena.get(entry.ty);
            let mut ctx = ReadContext::new(
                &mut *self.handle.cursor,
                self.arena,
                address,
                Some(entry.byte_len()),
                (entry.offset_bits % 8) as u8,
            );
            if let Some(value) = read_type_record(record, &mut ctx)? {
                return Ok(value);
            }
        }
        Err(SymbolAccessError::UnsupportedTraversal { label })
    }

    fn symbol_label(&self) -> String {
        self.handle
            .table
            .resolve_label(self.snapshot.record.label)
            .to_string()
    }
}
