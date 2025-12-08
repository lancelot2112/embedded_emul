//! Targeted tests verifying symbol-backed reads and traversal behaviour.

use super::*;
use crate::soc::bus::{BusCursor, DeviceBus};
use crate::soc::device::{AccessContext, Endianness as DeviceEndianness, RamMemory};
use crate::soc::prog::symbols::SymbolTable;
use crate::soc::prog::symbols::symbol::SymbolState;
use crate::soc::prog::symbols::walker::ValueKind;
use crate::soc::prog::types::aggregate::AggregateKind;
use crate::soc::prog::types::arena::{TypeArena, TypeId};
use crate::soc::prog::types::arena_record::TypeRecord;
use crate::soc::prog::types::bitfield::{BitFieldSpec, PadKind, PadSpec};
use crate::soc::prog::types::builder::TypeBuilder;
use crate::soc::prog::types::enum_scalar::{EnumType, EnumVariant};
use crate::soc::prog::types::scalar::{DisplayFormat, ScalarEncoding, ScalarStorage, ScalarType};
use std::sync::Arc;

fn make_cursor(size: usize) -> BusCursor {
    let mut bus = DeviceBus::new(32);
    let memory = RamMemory::new("ram", size, DeviceEndianness::Little);
    bus.map_device(memory, 0, 0).unwrap();
    BusCursor::attach_to_bus(Arc::new(bus), 0, AccessContext::CPU)
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

    let mut cursor = make_cursor(0x100);

    cursor
        .goto(0x20)
        .unwrap()
        .write_ram(&u32::to_le_bytes(0xDEAD_BEEF))
        .unwrap();

    let mut symbol_handle = SymbolBus::new(&table, &mut cursor);
    let value = symbol_handle.read_value(handle).expect("value read");
    assert_eq!(
        value,
        SymbolValue::Unsigned(0xDEAD_BEEF),
        "scalar should decode as unsigned"
    );
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

    let mut cursor = make_cursor(0x40);
    cursor.goto(0x10).unwrap().write_ram(&[1]).unwrap();

    let mut symbol_handle = SymbolBus::new(&table, &mut cursor);
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

    let mut cursor = make_cursor(0x10);
    let mut symbol_handle = SymbolBus::new(&table, &mut cursor);
    let err = symbol_handle
        .read_value(handle)
        .expect_err("missing address");
    assert!(
        matches!(err, SymbolAccessError::MissingAddress { .. }),
        "missing address should surface as error"
    );
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

    let mut cursor = make_cursor(0x100);
    cursor
        .goto(0x10)
        .expect("absolute jump to pointer target")
        .write_ram(&u32::to_le_bytes(0xAABB_CCDD))
        .expect("write target value");
    cursor
        .goto(0x00)
        .expect("absolute jump to pointer")
        .write_ram(&u64::to_le_bytes(0x10))
        .expect("write pointer value");

    let mut symbol_handle = SymbolBus::new(&table, &mut cursor);
    let mut cursor = symbol_handle.value_cursor(handle).expect("cursor");
    let entry = cursor.try_next().expect("pointer entry").expect("value");
    assert!(
        matches!(entry.entry.kind, ValueKind::Pointer { .. }),
        "walker should report pointer kind"
    );
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
    let u16_id = builder.scalar(
        Some("word"),
        2,
        ScalarEncoding::Unsigned,
        DisplayFormat::Hex,
    );
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

    let mut cursor = make_cursor(0x100);
    cursor
        .goto(0x40)
        .expect("absolute jump to array start")
        .write_ram(&[0x01, 0x00, 0x02, 0x00, 0x03, 0x00])
        .expect("write array data");

    let mut symbol_handle = SymbolBus::new(&table, &mut cursor);
    let mut cursor = symbol_handle.value_cursor(handle).expect("cursor");
    let mut seen = Vec::new();
    while let Some(value) = cursor.try_next().expect("next") {
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
    let (header_id, tail_id) = {
        let mut builder = TypeBuilder::new(&mut arena);
        let header = builder.scalar(
            Some("header"),
            1,
            ScalarEncoding::Unsigned,
            DisplayFormat::Hex,
        );
        let tail = builder.scalar(
            Some("tail"),
            2,
            ScalarEncoding::Signed,
            DisplayFormat::Decimal,
        );
        (header, tail)
    };
    let container_storage = ScalarStorage::for_bytes(2);
    let bitfield_id = {
        let bitfield = BitFieldSpec::from_range(container_storage.bit_size(), 0, 12);
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

    let mut cursor = make_cursor(0x80);
    let payload = [0xAA, 0xBC, 0x0A, 0x34, 0x12];
    cursor
        .goto(0x30)
        .expect("absolute jump to struct start")
        .write_ram(&payload)
        .expect("write payload");

    let mut symbol_handle = SymbolBus::new(&table, &mut cursor);
    let mut cursor = symbol_handle.value_cursor(handle).expect("cursor");

    let first = cursor.try_next().expect("header step").expect("value");
    assert_eq!(first.entry.path.to_string(arena.as_ref()), "header");
    assert_eq!(
        first.value,
        SymbolValue::Unsigned(0xAA),
        "scalar should decode directly"
    );

    let second = cursor.try_next().expect("flags step").expect("value");
    assert_eq!(second.entry.path.to_string(arena.as_ref()), "flags");
    assert_eq!(
        second.value,
        SymbolValue::Unsigned(0x0ABC),
        "bitfield bytes should round-trip"
    );

    let third = cursor.try_next().expect("tail step").expect("value");
    assert_eq!(third.entry.path.to_string(arena.as_ref()), "tail");
    assert_eq!(
        third.value,
        SymbolValue::Signed(0x1234),
        "signed field should decode respecting endianness"
    );

    assert!(
        cursor.try_next().unwrap().is_none(),
        "cursor should exhaust after three members"
    );
}

#[test]
fn bitfield_members_read_individually() {
    let mut arena = TypeArena::new();
    let backing_storage = {
        let mut builder = TypeBuilder::new(&mut arena);
        builder.scalar(
            Some("register"),
            2,
            ScalarEncoding::Unsigned,
            DisplayFormat::Hex,
        );
        ScalarStorage::for_bytes(2)
    };
    let specs: [(&str, u16); 5] = [("a", 0), ("b", 3), ("c", 6), ("d", 9), ("e", 12)];
    let bitfield_ids: Vec<TypeId> = specs
        .iter()
        .map(|(_, offset)| {
            let bitfield = BitFieldSpec::from_range(backing_storage.bit_size(), *offset, 3);
            arena.push_record(TypeRecord::BitField(bitfield))
        })
        .collect();
    let agg_id = {
        let mut builder = TypeBuilder::new(&mut arena);
        let mut agg = builder.aggregate(AggregateKind::Struct).layout(2, 1);
        for ((name, _), field_id) in specs.iter().zip(bitfield_ids.iter()) {
            agg = agg.member(*name, *field_id, 0);
        }
        agg.finish()
    };
    let arena = Arc::new(arena);
    let mut table = SymbolTable::new(Arc::clone(&arena));
    let handle = table
        .builder()
        .label("BITS")
        .type_id(agg_id)
        .runtime_addr(0x60)
        .size(2)
        .finish();

    let mut cursor = make_cursor(0x80);
    let packed =
        (1 & 0x7) | ((2 & 0x7) << 3) | ((3 & 0x7) << 6) | ((4 & 0x7) << 9) | ((5 & 0x7) << 12);
    cursor
        .goto(0x60)
        .expect("absolute jump to bitfield start")
        .write_ram(&u16::to_le_bytes(packed))
        .expect("write packed bitfield");

    let mut symbol_handle = SymbolBus::new(&table, &mut cursor);
    let mut cursor = symbol_handle.value_cursor(handle).expect("cursor");
    let mut seen = Vec::new();
    while let Some(step) = cursor.try_next().expect("cursor step") {
        seen.push((step.entry.path.to_string(arena.as_ref()), step.value));
    }

    let expected = vec![
        ("a".to_string(), SymbolValue::Unsigned(1)),
        ("b".to_string(), SymbolValue::Unsigned(2)),
        ("c".to_string(), SymbolValue::Unsigned(3)),
        ("d".to_string(), SymbolValue::Unsigned(4)),
        ("e".to_string(), SymbolValue::Unsigned(5)),
    ];
    assert_eq!(
        seen, expected,
        "cursor should decode each 3-bit field independently"
    );
}

#[test]
fn bitfield_sign_extension_honors_pad() {
    let mut arena = TypeArena::new();
    let storage = ScalarStorage::for_bytes(1);
    let bitfield_id = {
        let spec = BitFieldSpec::builder(storage.bit_size())
            .range(4, 4)
            .pad(PadSpec::new(PadKind::Sign, 4))
            .signed(true)
            .finish();
        arena.push_record(TypeRecord::BitField(spec))
    };
    let agg_id = {
        let mut builder = TypeBuilder::new(&mut arena);
        builder
            .aggregate(AggregateKind::Struct)
            .layout(1, 0)
            .member("field", bitfield_id, 0)
            .finish()
    };
    let arena = Arc::new(arena);
    let mut table = SymbolTable::new(Arc::clone(&arena));
    let handle = table
        .builder()
        .label("PADDED")
        .type_id(agg_id)
        .runtime_addr(0x70)
        .size(1)
        .finish();

    let mut cursor = make_cursor(0x80);
    cursor
        .goto(0x70)
        .expect("absolute jump to bitfield start")
        .write_ram(&[0xE0])
        .expect("write bitfield data");

    let mut symbol_handle = SymbolBus::new(&table, &mut cursor);
    let mut cursor = symbol_handle.value_cursor(handle).expect("cursor");
    let value = cursor.try_next().expect("bitfield entry").expect("value");
    assert_eq!(value.entry.path.to_string(arena.as_ref()), "field");
    assert_eq!(
        value.value,
        SymbolValue::Signed(-2),
        "sign pad should extend high bit"
    );
}

#[test]
fn union_members_overlay_same_bytes() {
    let mut arena = TypeArena::new();
    let union_id = {
        let mut builder = TypeBuilder::new(&mut arena);
        let as_u32 = builder.scalar(
            Some("as_u32"),
            4,
            ScalarEncoding::Unsigned,
            DisplayFormat::Hex,
        );
        let as_f32 = builder.scalar(
            Some("as_f32"),
            4,
            ScalarEncoding::Floating,
            DisplayFormat::Default,
        );
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

    let mut cursor = make_cursor(0x80);
    let overlay = f32::to_le_bytes(1.0);
    cursor
        .goto(0x50)
        .expect("absolute jump to union start")
        .write_ram(&overlay)
        .expect("write overlay data");

    let mut symbol_handle = SymbolBus::new(&table, &mut cursor);
    let mut cursor = symbol_handle.value_cursor(handle).expect("cursor");

    let first = cursor.try_next().expect("as_u32 step").expect("value");
    assert_eq!(first.entry.path.to_string(arena.as_ref()), "payload.as_u32");
    assert_eq!(
        first.value,
        SymbolValue::Unsigned(0x3F80_0000),
        "raw bytes should decode as u32"
    );

    let second = cursor.try_next().expect("as_f32 step").expect("value");
    assert_eq!(
        second.entry.path.to_string(arena.as_ref()),
        "payload.as_f32"
    );
    assert_eq!(
        second.value,
        SymbolValue::Float(1.0),
        "same bytes should reinterpret as float"
    );
    assert_eq!(
        first.address, second.address,
        "union members must reference the same address"
    );

    assert!(
        cursor.try_next().unwrap().is_none(),
        "union member list should be exhausted"
    );
}
