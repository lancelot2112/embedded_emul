# `soc::progs::types` Architecture

## Intent

Port the expressive .NET type system under `integrations/dotnet/programs/types` into idiomatic, high-performance Rust. The module must faithfully represent the kinds of types that show up in loader/debugger metadata (DWARF, STABS, CodeView, etc.), but with a design that:

- avoids per-value heap churn (arena + interning friendly)
- isolates all tree-walking logic in one fast iterator API
- keeps layout/dynamic-evaluation code zero-copy when possible
- plays well with the rest of the SoC pipeline (scheduler, loader, exec)

## High-level goals

1. **Expressive coverage** – scalars, enums, bitfields, fixed-point values, arrays (static + dynamic), pointers/references, structs/classes/unions, subroutines, variant records, and runtime-defined aggregates.
2. **Deterministic speed** – predictable traversal, cache-friendly storage, zero-cost abstractions over tight loops.
3. **Tree-walking encapsulation** – consumers manipulate `TypeId` + `MemberPath` primitives, never digging through nested `Vec`s manually.
4. **Memory efficiency** – compact handles, shared name storage, lazy computation of expensive derived info.
5. **Pluggable ingestion** – clear boundary between debug-info backends and the canonical type arena.

## Module layout (proposed)

```
src/soc/progs/types/
├── mod.rs              # re-exports + module wiring
├── arena.rs            # TypeArena, TypeId, interning helpers
├── record.rs           # TypeRecord enum + data structs
├── scalar.rs           # numeric/string/fixed/enumeration logic
├── aggregate.rs        # structs/classes/unions + members
├── sequence.rs         # arrays, slices, dynamic containers
├── pointer.rs          # pointer/ref/function-pointer types
├── callable.rs         # subroutine signatures, pc ranges
├── dynamic.rs          # runtime-shaped aggregates + expr VM
├── expr.rs             # stack-based evaluator (DWARF-style ops)
├── walker.rs           # TypeWalker/MemberCursor APIs
├── builder.rs          # Debug info ingestion + dedup
└── fmt.rs              # formatting/display helpers
```

Splitting responsibilities this way keeps hot paths (`walker`, `arena`) lean while allowing specialized modules (e.g., `dynamic`) to evolve independently.

## Core data model

### Type identity & storage

```rust
#[repr(transparent)]
pub struct TypeId(NonZeroU32);

pub struct TypeArena {
    records: Vec<TypeRecord>,
    member_spans: Vec<MemberRecord>,
    strings: StringInterner,
}
```

- `TypeRecord` is an enum capturing each shape (`Scalar`, `Enum`, `BitField`, `Fixed`, `Array`, `Pointer`, `Aggregate`, `Callable`, `Dynamic`, `Opaque`).
- Members and array element metadata live in dense side tables (`member_spans`) to keep the enum small and cache-friendly.
- String data (names, labels, path segments) stored via a tiny string interner (`lasso`, `ahash`) to avoid cloning across members.
- `TypeId` stays compact (fits in `u32`, `NonZero` enables `Option<TypeId>` niche).

### Scalar & enumeration types

`scalar.rs` models what `GenBaseValue`, `GenFixedValue`, and `GenEnumeration` covered:

- `ScalarEncoding` enum mirrors unsigned/signed/float/string/fixed.
- `DisplayFormat` replicates decimal/hex/dot notation; conversions implemented via zero-allocation helpers using stack buffers.
- `FixedScalar` stores scale/offset as `f64` plus an inferred formatting precision.
- `EnumType` keeps a `BTreeMap<i64, EnumLabel>` + reverse hash map for label lookup; `smallvec` used when <5 entries to avoid extra allocations.

### Aggregates, classes, and unions

`aggregate.rs` unifies `GenStructure` + `GenClass` into a single `AggregateType` struct:

```rust
pub struct AggregateType {
    pub kind: AggregateKind, // Struct | Class | Union | Variant
    pub members: MemberSpan, // slice inside arena.member_spans
    pub static_members: SmallVec<[StaticMember; 2]>,
    pub byte_size: LayoutSize,
    pub has_dynamic: bool,
}
```

- `MemberRecord` contains `name_id`, `offset_bits`, `bit_size`, `TypeId`.
- `Class` kind optionally keeps `StaticMemberHandle` entries pointing into global variable tables.
- For unions, `offset_bits` stays zero while `byte_size` tracks max of variants.

### Arrays, slices, and dynamic containers

`sequence.rs` handles `GenArray` + DWARF flex arrays:

- `SequenceType { element: TypeId, count: SequenceCount }` where `SequenceCount` is `Static(NonZeroU32)` or `Dynamic(ExprId)`.
- Byte size computed lazily; when provided by DWARF `DW_AT_byte_size`, we store it and skip multiplication.
- For runtime-sized arrays referencing sibling members (DWARF `DW_AT_string_length`, etc.), we track `CountSource::Member(MemberPathId)`.

### Pointers, references, callable types

- `pointer.rs` supports data pointers, reference types, `restrict/volatile` qualifiers, and segmented addresses.
- `callable.rs` models `GenSubroutine`: return list, parameter list, local variable table handles, PC range, calling convention.
- Function pointers reuse the same `CallableId` inside a pointer record to avoid string concatenation each time.

### Dynamic & expression evaluation

`dynamic.rs` + `expr.rs` translate `GenDynamic` + `GenExpression`:

```rust
pub struct ExprProgram {
    ops: SmallVec<[OpCode; 8]>,
}

pub enum OpCode {
    PushConst(u64),
    ReadMember(MemberPathId),
    ReadVar(VariableId),
    SizeOf(TypeId),
    CountOf(TypeId),
    Add,
    Sub,
    Mul,
    Div,
    Neg,
    Deref,
}
```

- Programs execute against a lightweight `EvalContext` that provides typed memory + variable lookup, eliminating heap allocations during traversal.
- `DynamicAggregate` owns a list of `DynamicField` definitions `(label, TypeId, SizeExpr, CountExpr, VarSource)`.
- When requested, `DynamicAggregate::materialize(ctx)` builds a transient `AggregateInstance` that can be cached behind `TypeId` if the layout is stable for given inputs.

## Tree-walking & member access

All nested traversal is centralized inside `walker.rs`:

- `TypeWalker<'a>` exposes `enter(TypeId) -> WalkerFrame` and yields members breadth-first or depth-first without allocating.
- `MemberCursor` keeps a stack of `{ type_id, member_idx, offset_bits }` stored inside a `SmallVec<[Frame; 4]>`; the stack doubles when deeper trees occur but stays on the stack for the common case.
- `MemberPathId` is a canonicalized handle for frequent lookups (e.g., `frame->regs[3]->status`). Paths are interned so repeated lookups reuse the same resolved offsets.
- Path resolution accepts bracket/dot syntax similar to `GenType.ResolvePath` but returns a `ResolvedPath { type_id, offset, bit_range }` struct ready for memory reads.
- Tree walking obeys visitor hooks: `Visitor::enter_aggregate`, `Visitor::visit_scalar`, etc., enabling future transformations (pretty-printing, validation) without duplicating traversal logic.

## Performance considerations

- **Arena packing** – store numeric metadata (`byte_size`, `bit_size`) as `u32`/`u16` when possible; promote to `u64` only when DWARF mandates large values.
- **Cache locality** – `TypeRecord` kept <32 bytes (mostly discriminant + indices). Large arrays of members live in dedicated contiguous vectors for prefetch-friendly scans.
- **Zero-copy formatting** – use `itoa`/`ryu` to format ints/floats when converting scalars to strings; avoid `String` allocations by writing into caller-provided buffers.
- **Fast maps** – prefer `ahash::AHashMap` or `rustc_hash::FxHashMap` for label/member lookup, with `SmallVec` fallback for <=4 entries to cut heap allocations.
- **Shared metadata** – `CompactString`/`SmartString` for names; `Arc<str>` only when data must outlive the arena.
- **Optional `no_std`** – keep dependencies to `alloc` so the type system can run inside bare-metal targets if required.

## Debug-info ingestion path

`builder.rs` acts as the single entry point for DWARF/STABS/other producers:

1. Parse backend-specific DIEs/records into a `RawTypeDesc` (no allocations beyond strings).
2. Feed the descriptor into `TypeBuilder`, which consults the arena to deduplicate equivalent shapes (structural hash of discriminant + metadata).
3. Emit stable `TypeId` handles to the rest of the SoC loader/executor.
4. Attach provenance spans (e.g., DWARF offsets) for debugging.

Backends implement `DebugTypeProvider`:

```rust
pub trait DebugTypeProvider {
    fn resolve_type(&mut self, handle: DebugHandle, builder: &mut TypeBuilder) -> TypeId;
}
```

This separation lets us swap DWARF/STABS readers without touching the hot path.

## Simplifications vs. the .NET code

- Collapse `GenStructure` and `GenClass` into `AggregateType` with `AggregateKind` enum.
- Represent `GenMember` as POD structs stored in contiguous slices, avoiding per-member objects.
- Replace reflection-heavy dynamic expressions with a compact bytecode evaluated over stack-allocated state.
- Use `TypeWalker` and `MemberCursor` everywhere instead of duplicating traversal logic across components.
- Remove mutable global state (`GenType._ID_`) by having the arena assign ids during interning.

## Future extensions

- Union variants + tagged enums: extend `AggregateKind::Variant` to carry discriminant info and dynamic evaluators for discriminant expressions.
- Bitfield composition reuse: `BitConstruct` can become a `BitLayout` helper shared by instruction decoders and struct bitfields.
- Persisted caches: optional on-disk cache of `(structural hash -> TypeId)` to skip rebuilding types between runs.
- Parallel builders: the arena can accept batches of `RawTypeDesc` once `TypeBuilder` is `Send` + uses sharded maps.

---

This plan keeps the richness of the .NET implementation while leaning into Rust strengths: tight data layouts, explicit lifetimes, and composable iterators. The resulting module should provide fast, safe building blocks for any loader or debugger component that needs to reason about program types.
