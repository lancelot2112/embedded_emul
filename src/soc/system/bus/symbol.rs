//! Symbol-aware handle that bridges the program `SymbolTable` with device bus access so tools can
//! read memory through symbolic context.

use std::sync::Arc;

use crate::soc::prog::symbols::walker::{SymbolWalkEntry, SymbolWalker, ValueKind};
use crate::soc::prog::symbols::{
    SymbolHandle as TableSymbolHandle, SymbolId, SymbolRecord, SymbolTable,
};
use crate::soc::prog::types::arena::{TypeArena, TypeId};
use crate::soc::prog::types::scalar::{EnumType, FixedScalar, ScalarEncoding, ScalarType};
use crate::soc::prog::types::sequence::{SequenceCount, SequenceType};
use crate::soc::system::bus::ext::{
    FloatDataHandleExt, ArbSizeDataHandleExt, StringDataHandleExt,
};
use crate::soc::system::bus::{BusError, BusResult, DataHandle, DeviceBus};

/// Computes typed values for symbols by combining the symbol table with a live bus view.
pub struct SymbolHandle<'a> {
    table: &'a SymbolTable,
    data: DataHandle,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SymbolValue {
    Unsigned(u64),
    Signed(i64),
    Float(f64),
    Utf8(String),
    Enum { label: Option<String>, value: i64 },
    Bytes(Vec<u8>),
}

#[derive(Debug)]
pub enum SymbolAccessError {
    MissingAddress { label: String },
    MissingSize { label: String },
    Bus(BusError),
    UnsupportedTraversal { label: String },
}

impl From<BusError> for SymbolAccessError {
    fn from(value: BusError) -> Self {
        SymbolAccessError::Bus(value)
    }
}

impl std::fmt::Display for SymbolAccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SymbolAccessError::MissingAddress { label } => {
                write!(f, "symbol '{label}' has no runtime or file address")
            }
            SymbolAccessError::MissingSize { label } => {
                write!(f, "symbol '{label}' has no byte size or sized type metadata")
            }
            SymbolAccessError::Bus(err) => err.fmt(f),
            SymbolAccessError::UnsupportedTraversal { label } => {
                write!(f, "symbol '{label}' has no type metadata to drive traversal")
            }
        }
    }
}

impl std::error::Error for SymbolAccessError {}

impl<'a> SymbolHandle<'a> {
    pub fn new(table: &'a SymbolTable, bus: Arc<DeviceBus>) -> Self {
        Self {
            table,
            data: DataHandle::new(bus),
        }
    }

    pub fn resolve_label(&self, label: &str) -> Option<TableSymbolHandle> {
        self.table
            .lookup_label(label)
            .and_then(|id| self.table.handles_by_label(id))
            .and_then(|handles| handles.first().copied())
    }

    pub fn resolve_symbol_id(&self, id: SymbolId) -> Option<TableSymbolHandle> {
        self.table.handle_by_symbol_id(id)
    }

    /// Creates a cursor that walks all primitive values reachable from the symbol's type tree.
    pub fn value_cursor<'handle>(
        &'handle mut self,
        symbol: TableSymbolHandle,
    ) -> Result<SymbolValueCursor<'handle, 'a>, SymbolAccessError> {
        let snapshot = self.prepare(symbol)?;
        let Some(type_id) = snapshot.record.type_id else {
            let label = self.table.resolve_label(snapshot.record.label).to_string();
            return Err(SymbolAccessError::UnsupportedTraversal { label });
        };
        let arena = self.table.type_arena();
        let walker = SymbolWalker::new(arena.as_ref(), type_id);
        Ok(SymbolValueCursor {
            handle: self,
            snapshot,
            walker,
            arena: arena.as_ref(),
        })
    }

    pub fn read_value(
        &mut self,
        symbol: TableSymbolHandle,
    ) -> Result<SymbolValue, SymbolAccessError> {
        let snapshot = self.prepare(symbol)?;
        let arena = self.table.type_arena();
        if let Some(value) = self.interpret_value(arena.as_ref(), &snapshot)? {
            return Ok(value);
        }
        let bytes = self.read_bytes(&snapshot)?;
        Ok(SymbolValue::Bytes(bytes))
    }

    pub fn read_raw_bytes(
        &mut self,
        symbol: TableSymbolHandle,
    ) -> Result<Vec<u8>, SymbolAccessError> {
        let snapshot = self.prepare(symbol)?;
        self.read_bytes(&snapshot)
    }

    fn prepare(&self, symbol: TableSymbolHandle) -> Result<Snapshot, SymbolAccessError> {
        let record = self.table.get(symbol).clone();
        let label = self.table.resolve_label(record.label).to_string();
        let address = record
            .runtime_addr
            .or(record.file_addr)
            .ok_or(SymbolAccessError::MissingAddress { label: label.clone() })?;
        let size = record
            .size
            .or_else(|| record.type_id.and_then(|ty| type_size(self.table.type_arena().as_ref(), ty)))
            .ok_or(SymbolAccessError::MissingSize { label })?;
        Ok(Snapshot {
            record,
            address,
            size,
        })
    }

    fn read_bytes(&mut self, snapshot: &Snapshot) -> Result<Vec<u8>, SymbolAccessError> {
        self.data.address_mut().jump(snapshot.address)?;
        let mut buf = vec![0u8; snapshot.size as usize];
        if snapshot.size > 0 {
            self.data.read_bytes(&mut buf)?;
        }
        Ok(buf)
    }

    fn interpret_value(
        &mut self,
        arena: &TypeArena,
        snapshot: &Snapshot,
    ) -> Result<Option<SymbolValue>, SymbolAccessError> {
        let Some(type_id) = snapshot.record.type_id else {
            return Ok(None);
        };
        self.interpret_type_at(
            arena,
            type_id,
            snapshot.address,
            Some(snapshot.size),
        )
    }

    fn interpret_type_at(
        &mut self,
        arena: &TypeArena,
        type_id: TypeId,
        address: u64,
        size_hint: Option<u32>,
    ) -> Result<Option<SymbolValue>, SymbolAccessError> {
        self.data.address_mut().jump(address)?;
        let record = arena.get(type_id);
        let value = match record {
            crate::soc::prog::types::record::TypeRecord::Scalar(scalar) => {
                interpret_scalar(&mut self.data, scalar)?
            }
            crate::soc::prog::types::record::TypeRecord::Enum(enum_type) => {
                Some(interpret_enum(&mut self.data, arena, enum_type)?)
            }
            crate::soc::prog::types::record::TypeRecord::Fixed(fixed) => {
                interpret_fixed(&mut self.data, fixed)?
            }
            crate::soc::prog::types::record::TypeRecord::Pointer(pointer) => {
                let width = pointer.byte_size.max(size_hint.unwrap_or(pointer.byte_size));
                interpret_pointer(&mut self.data, width as usize)?
            }
            _ => None,
        };
        Ok(value)
    }
}

struct Snapshot {
    record: SymbolRecord,
    address: u64,
    size: u32,
}

/// Streaming view that materialises values discovered by the `SymbolWalker` and exposes typed
/// reads/writes at each primitive leaf.
pub struct SymbolValueCursor<'handle, 'arena> {
    handle: &'handle mut SymbolHandle<'arena>,
    snapshot: Snapshot,
    walker: SymbolWalker<'arena>,
    arena: &'arena TypeArena,
}

pub struct SymbolWalkRead {
    pub entry: SymbolWalkEntry,
    pub value: SymbolValue,
    pub address: u64,
}

impl<'handle, 'arena> SymbolValueCursor<'handle, 'arena> {
    /// Returns the next primitive value in declaration order along with its formatted path.
    pub fn next(&mut self) -> Result<Option<SymbolWalkRead>, SymbolAccessError> {
        while let Some(entry) = self.walker.next() {
            if entry.offset_bits % 8 != 0 {
                continue;
            }
            let address = self.snapshot.address + (entry.offset_bits / 8) as u64;
            let value = self.read_entry_value(&entry, address)?;
            return Ok(Some(SymbolWalkRead { entry, value, address }));
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
            .interpret_type_at(
                self.arena,
                target,
                address,
                None,
            )
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
        let address = self.snapshot.address + (entry.offset_bits / 8) as u64;
        self.handle.data.address_mut().jump(address)?;
        self.handle.data.write_bytes(data)?;
        Ok(())
    }

    fn read_entry_value(
        &mut self,
        entry: &SymbolWalkEntry,
        address: u64,
    ) -> Result<SymbolValue, SymbolAccessError> {
        self.handle.data.address_mut().jump(address)?;
        let value = match entry.kind {
            ValueKind::Unsigned { bytes } => {
                let val = self.handle.data.read_unsigned(bytes as usize)?;
                SymbolValue::Unsigned(val)
            }
            ValueKind::Signed { bytes } => {
                let val = self.handle.data.read_signed(bytes as usize)?;
                SymbolValue::Signed(val)
            }
            ValueKind::Float32 => {
                let val = self.handle.data.read_f32()?;
                SymbolValue::Float(val as f64)
            }
            ValueKind::Float64 => {
                let val = self.handle.data.read_f64()?;
                SymbolValue::Float(val)
            }
            ValueKind::Utf8 { bytes } => {
                let text = self.handle.data.read_utf8(bytes as usize)?;
                SymbolValue::Utf8(text)
            }
            ValueKind::Enum => {
                let record = self.arena.get(entry.ty);
                if let crate::soc::prog::types::record::TypeRecord::Enum(enum_type) = record {
                    interpret_enum(&mut self.handle.data, self.arena, enum_type)?
                } else {
                    return Err(SymbolAccessError::UnsupportedTraversal {
                        label: self
                            .handle
                            .table
                            .resolve_label(self.snapshot.record.label)
                            .to_string(),
                    });
                }
            }
            ValueKind::Fixed => {
                let record = self.arena.get(entry.ty);
                if let crate::soc::prog::types::record::TypeRecord::Fixed(fixed) = record {
                    interpret_fixed(&mut self.handle.data, fixed)?
                        .unwrap_or(SymbolValue::Signed(0))
                } else {
                    return Err(SymbolAccessError::UnsupportedTraversal {
                        label: self
                            .handle
                            .table
                            .resolve_label(self.snapshot.record.label)
                            .to_string(),
                    });
                }
            }
            ValueKind::Pointer { bytes, .. } => {
                let val = self.handle.data.read_unsigned(bytes as usize)?;
                SymbolValue::Unsigned(val)
            }
        };
        Ok(value)
    }
}

fn interpret_scalar(
    handle: &mut DataHandle,
    scalar: &ScalarType,
) -> BusResult<Option<SymbolValue>> {
    let width = scalar.byte_size as usize;
    let value = match scalar.encoding {
        ScalarEncoding::Unsigned => {
            let value = if width == 0 {
                0
            } else {
                handle.read_unsigned(width)?
            };
            Some(SymbolValue::Unsigned(value))
        }
        ScalarEncoding::Signed => {
            let value = if width == 0 {
                0
            } else {
                handle.read_signed(width)?
            };
            Some(SymbolValue::Signed(value))
        }
        ScalarEncoding::Floating => match width {
            4 => {
                let value = handle.read_f32()?;
                Some(SymbolValue::Float(value as f64))
            }
            8 => {
                let value = handle.read_f64()?;
                Some(SymbolValue::Float(value))
            }
            _ => None,
        },
        ScalarEncoding::Utf8String => {
            if width == 0 {
                return Ok(Some(SymbolValue::Utf8(String::new())));
            }
            let value = handle.read_utf8(width)?;
            Some(SymbolValue::Utf8(value))
        }
    };
    Ok(value)
}

fn interpret_enum(
    handle: &mut DataHandle,
    arena: &TypeArena,
    enum_type: &EnumType,
) -> BusResult<SymbolValue> {
    let width = enum_type.base.byte_size as usize;
    let value = if width == 0 {
        0
    } else {
        handle.read_signed(width,)?
    };
    let label = enum_type
        .label_for(value)
        .map(|id| arena.resolve_string(id).to_string());
    Ok(SymbolValue::Enum { label, value })
}

fn interpret_fixed(
    handle: &mut DataHandle,
    fixed: &FixedScalar,
) -> BusResult<Option<SymbolValue>> {
    let width = fixed.base.byte_size as usize;
    if width == 0 {
        return Ok(Some(SymbolValue::Float(fixed.apply(0))));
    }
    let raw = handle.read_signed(width)?;
    Ok(Some(SymbolValue::Float(fixed.apply(raw))))
}

fn interpret_pointer(
    handle: &mut DataHandle,
    width: usize,
) -> BusResult<Option<SymbolValue>> {
    if width > 8 {
        return Ok(None);
    }
    let value = if width == 0 {
        0
    } else {
        handle.read_unsigned(width)?
    };
    Ok(Some(SymbolValue::Unsigned(value)))
}

fn type_size(arena: &TypeArena, ty: TypeId) -> Option<u32> {
    match arena.get(ty) {
        crate::soc::prog::types::record::TypeRecord::Scalar(scalar) => Some(scalar.byte_size),
        crate::soc::prog::types::record::TypeRecord::Enum(enum_type) => {
            Some(enum_type.base.byte_size)
        }
        crate::soc::prog::types::record::TypeRecord::Fixed(fixed) => {
            Some(fixed.base.byte_size)
        }
        crate::soc::prog::types::record::TypeRecord::Sequence(seq) => sequence_size(seq),
        crate::soc::prog::types::record::TypeRecord::Aggregate(agg) => Some(agg.byte_size.bytes),
        crate::soc::prog::types::record::TypeRecord::Opaque(opaque) => Some(opaque.byte_size),
        crate::soc::prog::types::record::TypeRecord::Pointer(pointer) => Some(pointer.byte_size),
        _ => None,
    }
}

fn sequence_size(seq: &SequenceType) -> Option<u32> {
    match seq.count {
        SequenceCount::Static(count) => count.checked_mul(seq.stride_bytes),
        SequenceCount::Dynamic(_) => None,
    }
}

#[cfg(test)]
mod tests {
    //! Targeted tests verifying symbol-backed reads and traversal behaviour.
    use super::*;
    use crate::soc::device::{BasicMemory, Device, Endianness as DeviceEndianness};
    use crate::soc::prog::symbols::symbol::SymbolState;
    use crate::soc::prog::types::aggregate::AggregateKind;
    use crate::soc::prog::types::arena::TypeArena;
    use crate::soc::prog::types::builder::TypeBuilder;
    use crate::soc::prog::types::record::TypeRecord;
    use crate::soc::prog::types::scalar::{BitFieldType, DisplayFormat, EnumVariant};

    fn make_bus(size: usize) -> (Arc<DeviceBus>, Arc<BasicMemory>) {
        let bus = Arc::new(DeviceBus::new(8));
        let memory = Arc::new(BasicMemory::new("ram", size, DeviceEndianness::Little));
        bus.register_device(memory.clone(), 0).unwrap();
        (bus, memory)
    }

    #[test]
    fn reads_unsigned_scalar_value() {
        let mut arena = TypeArena::new();
        let scalar = ScalarType::new(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Hex);
        let scalar_id = arena.push_record(TypeRecord::Scalar(scalar));
        let arena = Arc::new(arena);
        let mut table = SymbolTable::new(Arc::clone(&arena));
        let handle = table
            .builder()
            .label("RPM")
            .type_id(scalar_id)
            .runtime_addr(0x20)
            .size(4)
            .state(SymbolState::Defined)
            .finish();

        let (bus, memory) = make_bus(0x100);
        memory
            .write(0x20, &u32::to_le_bytes(0xDEAD_BEEF))
            .unwrap();

        let mut symbol_handle = SymbolHandle::new(&table, bus);
        let value = symbol_handle.read_value(handle).expect("value read");
        assert_eq!(value, SymbolValue::Unsigned(0xDEAD_BEEF), "scalar should decode as unsigned");
    }

    #[test]
    fn enum_value_reports_label() {
        let mut arena = TypeArena::new();
        let ready_label = arena.intern_string("Ready");
        let base = ScalarType::new(None, 1, ScalarEncoding::Signed, DisplayFormat::Default);
        let mut enum_type = EnumType::new(base);
        enum_type.push_variant(EnumVariant {
            label: ready_label,
            value: 1,
        });
        let enum_id = arena.push_record(TypeRecord::Enum(enum_type));
        let arena = Arc::new(arena);
        let mut table = SymbolTable::new(Arc::clone(&arena));
        let handle = table
            .builder()
            .label("STATE")
            .type_id(enum_id)
            .runtime_addr(0x10)
            .size(1)
            .finish();

        let (bus, memory) = make_bus(0x40);
        memory.write(0x10, &[1]).unwrap();

        let mut symbol_handle = SymbolHandle::new(&table, bus);
        let value = symbol_handle.read_value(handle).expect("enum value");
        assert_eq!(
            value,
            SymbolValue::Enum {
                label: Some("Ready".into()),
                value: 1,
            },
            "enum helper should expose the matched label"
        );
    }

    #[test]
    fn missing_address_reports_error() {
        let mut arena = TypeArena::new();
        let scalar = ScalarType::new(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Decimal);
        let scalar_id = arena.push_record(TypeRecord::Scalar(scalar));
        let arena = Arc::new(arena);
        let mut table = SymbolTable::new(Arc::clone(&arena));
        let handle = table
            .builder()
            .label("BROKEN")
            .type_id(scalar_id)
            .size(4)
            .finish();

        let (bus, _) = make_bus(0x10);
        let mut symbol_handle = SymbolHandle::new(&table, bus);
        let err = symbol_handle.read_value(handle).expect_err("missing address");
        assert!(matches!(err, SymbolAccessError::MissingAddress { .. }), "missing address should surface as error");
    }

    #[test]
    fn pointer_deref_reads_target_value() {
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let u32_id = builder.scalar(None, 4, ScalarEncoding::Unsigned, DisplayFormat::Hex);
        let ptr_id = builder.pointer(
            u32_id,
            crate::soc::prog::types::pointer::PointerKind::Data,
            8,
        );
        let arena = Arc::new(arena);
        let mut table = SymbolTable::new(Arc::clone(&arena));
        let handle = table
            .builder()
            .label("PTR")
            .type_id(ptr_id)
            .runtime_addr(0x00)
            .size(8)
            .finish();

        let (bus, memory) = make_bus(0x100);
        memory.write(0x10, &u32::to_le_bytes(0xAABB_CCDD)).unwrap();
        memory.write(0x00, &u64::to_le_bytes(0x10)).unwrap();

        let mut symbol_handle = SymbolHandle::new(&table, bus);
        let mut cursor = symbol_handle.value_cursor(handle).expect("cursor");
        let entry = cursor.next().expect("pointer entry").expect("value");
        assert!(matches!(entry.entry.kind, ValueKind::Pointer { .. }), "walker should report pointer kind");
        let pointee = cursor.deref(&entry).expect("deref").expect("pointee value");
        assert_eq!(
            pointee,
            SymbolValue::Unsigned(0xAABB_CCDD),
            "dereferenced pointer should read the target value"
        );
    }

    #[test]
    fn walker_iterates_structured_arrays() {
        let mut arena = TypeArena::new();
        let mut builder = TypeBuilder::new(&mut arena);
        let u16_id = builder.scalar(Some("word"), 2, ScalarEncoding::Unsigned, DisplayFormat::Hex);
        let seq_id = builder.sequence_static(u16_id, 2, 3);
        let agg_id = builder
            .aggregate(AggregateKind::Struct)
            .layout(6, 0)
            .member("data", seq_id, 0)
            .finish();
        let arena = Arc::new(arena);
        let mut table = SymbolTable::new(Arc::clone(&arena));
        let handle = table
            .builder()
            .label("ARRAY")
            .type_id(agg_id)
            .runtime_addr(0x40)
            .size(6)
            .finish();

        let (bus, memory) = make_bus(0x100);
        memory.write(0x40, &[0x01, 0x00, 0x02, 0x00, 0x03, 0x00]).unwrap();

        let mut symbol_handle = SymbolHandle::new(&table, bus);
        let mut cursor = symbol_handle.value_cursor(handle).expect("cursor");
        let mut seen = Vec::new();
        while let Some(value) = cursor.next().expect("next") {
            seen.push(value);
        }
        let paths: Vec<String> = seen
            .iter()
            .map(|entry| entry.entry.path.to_string(arena.as_ref()))
            .collect();
        assert_eq!(
            paths,
            vec!["data[0]", "data[1]", "data[2]"],
            "walker should visit array elements in order"
        );
    }

    #[test]
    fn mixed_data_and_bitfield_values() {
        let mut arena = TypeArena::new();
        let (header_id, container_id, tail_id) = {
            let mut builder = TypeBuilder::new(&mut arena);
            let header = builder.scalar(Some("header"), 1, ScalarEncoding::Unsigned, DisplayFormat::Hex);
            let container = builder.scalar(None, 2, ScalarEncoding::Unsigned, DisplayFormat::Hex);
            let tail = builder.scalar(Some("tail"), 2, ScalarEncoding::Signed, DisplayFormat::Decimal);
            (header, container, tail)
        };
        let bitfield_id = {
            let bitfield = BitFieldType::new(container_id, 0, 12);
            arena.push_record(TypeRecord::BitField(bitfield))
        };
        let agg_id = {
            let mut builder = TypeBuilder::new(&mut arena);
            builder
                .aggregate(AggregateKind::Struct)
                .layout(5, 0)
                .member("header", header_id, 0)
                .member("flags", bitfield_id, 1)
                .member("tail", tail_id, 3)
                .finish()
        };
        let arena = Arc::new(arena);
        let mut table = SymbolTable::new(Arc::clone(&arena));
        let handle = table
            .builder()
            .label("MIXED")
            .type_id(agg_id)
            .runtime_addr(0x30)
            .size(5)
            .finish();

        let (bus, memory) = make_bus(0x80);
        let payload = [0xAA, 0xBC, 0x0A, 0x34, 0x12];
        memory.write(0x30, &payload).unwrap();

        let mut symbol_handle = SymbolHandle::new(&table, bus);
        let mut cursor = symbol_handle.value_cursor(handle).expect("cursor");

        let first = cursor.next().expect("header step").expect("value");
        assert_eq!(first.entry.path.to_string(arena.as_ref()), "header");
        assert_eq!(first.value, SymbolValue::Unsigned(0xAA), "scalar should decode directly");

        let second = cursor.next().expect("flags step").expect("value");
        assert_eq!(second.entry.path.to_string(arena.as_ref()), "flags");
        assert_eq!(second.value, SymbolValue::Unsigned(0x0ABC), "bitfield bytes should round-trip");

        let third = cursor.next().expect("tail step").expect("value");
        assert_eq!(third.entry.path.to_string(arena.as_ref()), "tail");
        assert_eq!(third.value, SymbolValue::Signed(0x1234), "signed field should decode respecting endianness");

        assert!(cursor.next().unwrap().is_none(), "cursor should exhaust after three members");
    }

    #[test]
    fn union_members_overlay_same_bytes() {
        let mut arena = TypeArena::new();
        let union_id = {
            let mut builder = TypeBuilder::new(&mut arena);
            let as_u32 = builder.scalar(Some("as_u32"), 4, ScalarEncoding::Unsigned, DisplayFormat::Hex);
            let as_f32 = builder.scalar(Some("as_f32"), 4, ScalarEncoding::Floating, DisplayFormat::Default);
            builder
                .aggregate(AggregateKind::Union)
                .layout(4, 0)
                .member("as_u32", as_u32, 0)
                .member("as_f32", as_f32, 0)
                .finish()
        };
        let container_id = {
            let mut builder = TypeBuilder::new(&mut arena);
            builder
                .aggregate(AggregateKind::Struct)
                .layout(4, 0)
                .member("payload", union_id, 0)
                .finish()
        };
        let arena = Arc::new(arena);
        let mut table = SymbolTable::new(Arc::clone(&arena));
        let handle = table
            .builder()
            .label("UNION")
            .type_id(container_id)
            .runtime_addr(0x50)
            .size(4)
            .finish();

        let (bus, memory) = make_bus(0x80);
        let overlay = f32::to_le_bytes(1.0);
        memory.write(0x50, &overlay).unwrap();

        let mut symbol_handle = SymbolHandle::new(&table, bus);
        let mut cursor = symbol_handle.value_cursor(handle).expect("cursor");

        let first = cursor.next().expect("as_u32 step").expect("value");
        assert_eq!(first.entry.path.to_string(arena.as_ref()), "payload.as_u32");
        assert_eq!(first.value, SymbolValue::Unsigned(0x3F80_0000), "raw bytes should decode as u32");

        let second = cursor.next().expect("as_f32 step").expect("value");
        assert_eq!(second.entry.path.to_string(arena.as_ref()), "payload.as_f32");
        assert_eq!(second.value, SymbolValue::Float(1.0), "same bytes should reinterpret as float");
        assert_eq!(first.address, second.address, "union members must reference the same address");

        assert!(cursor.next().unwrap().is_none(), "union member list should be exhausted");
    }
}
