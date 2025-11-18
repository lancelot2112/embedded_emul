# Shared BitField Specification

This document captures the proposed representation for bitfield extraction that is shared by both memory-backed structs (register fields, device data) and instruction decoding per the `.isa` specification.

## Goals

1. Support every syntax form described in `docs/spec/isa_language_specification.md` §5.1.5 (single bits, ranges, concatenation, literal padding, sign extension) using a common runtime structure.
2. Allow type arena records (`TypeRecord::BitField`) to store the full extraction recipe so that higher layers only need the arena reference.
3. Provide a single implementation for "extract bits from container bytes" used by:
   - `SymbolValueCursor` (reading register fields/bus data)
   - Future instruction walkers/decoders built on the same type arena
   - Validation/linting to ensure bit ranges are sane
4. Keep storage compact: capture just enough metadata to reconstruct values without re-parsing textual specs.

## Core Types

```
pub struct BitFieldSpec {
    pub container: TypeId,
    pub segments: SmallVec<[BitFieldSegment; N]>,
    pub total_width: u16,
    pub is_signed: bool,
}

pub enum BitFieldSegment {
    Range { msb: u16, lsb: u16 },
    Literal { value: u64, width: u8 },
    SignExtend { bit: u16 },
}
```

- `container` points to the parent scalar/aggregate type describing the backing bytes.
- `segments` are evaluated left-to-right, mirroring the `@(<spec1>|<spec2>|...)` ordering.
- `Range` slices bits out of the container using MSB-0 numbering (`msb` ≤ `lsb`).
- `Literal` injects constant bit sequences (`0b00` etc.). Width must fit into 64 bits.
- `SignExtend` indicates an entire result should be sign-extended using the specified bit from the result built so far (set `is_signed = true`). The parser emits this when encountering `?1`/`?0` clauses.

## Builder API

- Extend `TypeBuilder` with `fn bitfield(&mut self, container: TypeId, spec: BitFieldSpec) -> TypeId` so both register definitions and instruction forms can allocate bitfields with identical backing data.
- Provide helper constructors so the ISA parser can parse `@(...)` text into `BitFieldSegment`s without knowing about arena internals.

## Extraction Algorithm

1. Determine the byte span needed by scanning all `Range` segments and computing `(max(lsb) / 8) - (min(msb) / 8)` relative to the `container` base.
2. Read the minimal slice of bytes from the bus or instruction word.
3. For each `Segment`:
   - `Range`: convert MSB-0 indexing into little-endian bit positions, shift/mask, append to an accumulator (`u128` if necessary).
   - `Literal`: shift the accumulator left by `width` and OR the literal.
   - `SignExtend`: after all segments, sign-extend the accumulator so the requested bit becomes the sign bit.
4. Return `SymbolValue::Unsigned` or `SymbolValue::Signed` depending on `is_signed`.

This logic will live alongside `SymbolValueCursor::read_entry_value`, but extracted into a helper so future instruction decoders can share it.

## Validation

While building `BitFieldSpec`, enforce:
- All referenced bits fall within the container's declared size (from `type_size`).
- `Literal.width` > 0.
- Sign extension segments appear at most once.
- `total_width` ≤ 64 by default (future extension could allow arbitrary precision as needed).

Violations should bubble up as structured errors so `.isa` linting can report precise line numbers.

## Migration Plan

1. Wrap the existing `{offset,width}` bitfield usages into a single `Range` segment and set `total_width = width`.
2. Update `TypeRecord::BitField` to store `BitFieldSpec` instead of the minimal struct.
3. Teach `SymbolWalker` to emit `ValueKind::BitField` (or continue using `Unsigned` with an extra flag) referencing the new spec.
4. Update cursor/tests (e.g., `bitfield_members_read_individually`) to assert behavior still matches, then add new tests covering literals and sign extension once instruction parsing lands.
