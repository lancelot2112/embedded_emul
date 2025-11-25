# System Bus Architecture (Rust)

## 1. Objective

This module reimplements the .NET system bus (`integrations/dotnet/sysbus`) in idiomatic Rust so the core emulator can own the memory map without crossing the FFI boundary. The Rust bus must:

- Provide deterministic address-to-device resolution for any 64-bit physical address space.
- Offer typed read/write helpers that mirror `BasicDataBus` and its extension surface (streams, strings, LEB128, crypto helpers, etc.).
- Support register-symbol aware helpers (register bus, symbol bus) used by tooling.
- Remain embeddable inside SoC models (`src/soc/system`).

Non-goals: replicating UI-specific helpers or writing device implementations beyond a `MemoryDevice` reference unit.

---

## 2. Design Principles

1. **Zero-copy, borrow-safe access:** `Device` trait methods take immutable/mutable borrows that propagate ownership of the underlying memory slice. We avoid `unsafe` unless profiling proves we need it.
2. **Predictable resolution cost:** Device lookup is a two-level hash (similar to `BasicHashedDeviceBus`) providing O(1) average lookup while keeping memory usage bounded.
3. **Composable handles:** Addressing state is kept inside lightweight handles (`AddressHandle`, `DataHandle`, `RegisterHandle`, `SymbolHandle`) so multiple units (cores, DMA engines, debugger) can traverse concurrently.
4. **Separation of mapping vs. access:** `DeviceBus` is the authority over ranges, redirects, and registration. A handle never mutates the mapping—only the bus does—making threading and borrow checking tractable.
5. **Pluggable extensions:** Streaming/encoding helpers live in `ext::*` modules so consumers can opt-in without pulling extra dependencies into the hot path.

---

## 3. Core Concepts

### 3.1 Device trait (`device.rs`)

```rust
pub trait Device: Send + Sync {
    fn name(&self) -> &str;
    fn span(&self) -> Range<u64>; // size implied by end - start

    /// Optional transaction hooks so devices can make multi-call
    /// read/modify/write sequences atomic inside their own state.
    fn start_transact(&self) -> DeviceResult<()> { Ok(()) }
    fn end_transact(&self) -> DeviceResult<()> { Ok(()) }

    /// Read a contiguous slice of bytes from device-native storage.
    fn read(&self, offset: u64, buf: &mut [u8]) -> DeviceResult<()>;

    /// Write a contiguous slice of bytes into device-native storage.
    fn write(&self, offset: u64, data: &[u8]) -> DeviceResult<()>;
}
```

- Backed by device-specific state (SRAM, flash, peripherals, bridges).
- Devices never see bit offsets or bus/emulator endianness—they just move bytes.
- A `BasicMemory` reference implementation mirrors the C# `BasicMemory` for tests.

### 3.2 Ranges & overlays (`range.rs`)

- `BusRange` owns `(bus_start, bus_end, device_id, device_offset, priority)`.
- Redirects are represented as synthetic ranges pointing into another device’s address window.
- Ranges never overlap at a given priority. Higher priority entries allow overlays (e.g., debug windows) without rewriting the base mapping.

### 3.3 Device bus (`bus.rs`)

Responsibilities:

1. Register/unregister devices (`register(device, base_addr)`). Reject zero-sized devices and overlapping ranges.
2. Manage redirect rules (`redirect(src_range, dst_range)`, `remove_redirect`). Redirects must fall fully within the target range.
3. Resolve addresses (`resolve(addr) -> Option<ResolvedRange>`). Implementation mirrors the hashed lookup from `BasicHashedDeviceBus`:
   - Level1 hash = `addr >> addr_bits`; Level2 bucket is a sorted `Vec<BusRange>`.
   - Insertions maintain ordering to keep lookups fast.
4. Emit diagnostics (owner, overlap, redirect conflicts) via `tracing`.

`ResolvedRange` stores the device Arc, start/end, and precomputed `device_offset` for the exact address so handles can increment cheaply.

---

## 4. Access Handles

### 4.1 AddressHandle (`address.rs`)

- Equivalent to `BasicBusAccess`.
- Holds an `Arc<DeviceBus>` plus cached `ResolvedRange` and offsets.
- API: `jump(addr)`, `jump_relative(delta)`, `advance(bytes)`, `bytes_remaining()`.
- Provides `bus_address()` and `device_offset()` getters for instrumentation.

### 4.2 DataHandle (`data.rs`)

- Builds on `AddressHandle` and exposes byte-level getters/setters mirroring the `Device` trait (`read`, `write`).
- Adds `available(len)` pre-check plus scalar helpers (`read_u{8,16,32,64}`, etc.) that interpret bytes in emulator-native endianness.
- Provides `read_bits`/`write_bits` helpers that:
    - Compute the minimal covering byte window for the requested bit slice.
    - Open a device transaction, perform a byte read–modify–write, and close the transaction.
    - Handle all masking, shifting, and endianness conversions in a local buffer.
- Implements `std::io::Read`/`Write` for stream interoperability, replacing `BusByteStream`.

### 4.3 RegisterHandle (`register.rs`)

- Wraps `AddressHandle` plus a `RegisterTable` (shared with the rest of the emulator).
- `get_value("PCR")` resolves the symbol, jumps to its offset, masks the slice (clones `RegisterBus` behavior).
- Provides `peek_field`/`poke_field` utilities for bit slices.

### 4.4 SymbolHandle (`symbol.rs`)

- Bridges to `VariableTable` so debugger tooling can step through C structs like the .NET `SymbolBus`.
- Stores traversal stack to support nested structs/arrays.
- Offers `resolve_path("task.control_block.pid")`, `next_value()`, `deref()` semantics aligned with the existing generator types in `EmbedEmul.Variables`.

---

## 5. Extension Layers (`ext/`)

| Module | Purpose | Notes |
| --- | --- | --- |
| `ext/stream.rs` | `Read`/`Write` adapters, `iter_bytes(count)` | mirrors `DataBusStreamExtensions` |
| `ext/string.rs` | Null-terminated, length-prefixed string helpers | used by symbol tooling |
| `ext/leb128.rs` | Encodes/decodes LEB128 integers | parity with `DataBusLEB128Extensions` |
| `ext/crypto.rs` | Hash and MAC helpers for firmware loaders | parity with `DataBusCryptoExtensions` |
| `ext/float.rs` | IEEE754 conversions for unaligned loads | parity with `DataBusFloatExtensions` |
| `ext/signed.rs` | Sign/zero extension helpers | parity with `DataBusSignedExtensions` |
| `ext/string_repr.rs` | Hex/ASCII dump utilities for debugging | complements CLI tools |

Extensions depend only on the `DataHandle` trait so they can be tested independently.

---

## 6. Concurrency & Borrowing

- `DeviceBus` owns devices inside `Arc<dyn Device>` and protects its range tables with `RwLock`. Writes (registration, redirect) are rare; reads (resolve) are frequent.
- Each handle contains its own `Arc<DeviceBus>` and caches only immutable data, allowing clones to move across threads.
- Device implementations choose their own interior mutability strategy (`Mutex`, `Atomic*`, `Cell`). The bus never assumes exclusivity beyond what the device trait enforces.
- DMA or multi-core scenarios spin up independent handles; they remain consistent because redirects and registrations mutate via the `DeviceBus` lock.

---

## 7. Error Handling & Instrumentation

- `BusError` enum: `NotMapped(addr)`, `Overlap { addr, existing }`, `RedirectInvalid`, `DeviceFault { device, source }`, `OutOfRange { addr, span }`.
- `DeviceBus::resolve` never panics; callers receive `NotMapped` and can decide whether to fault the CPU or return zero.
- Use `tracing::instrument` on registration, redirect, and handle jumps for debugger hooks and log replay.
- Provide feature-guarded stats (cache hits/misses, bytes transferred) for performance runs.

---

## 8. Module Layout & File Plan

```
src/soc/system/bus/
├── architecture.md        # this document
├── mod.rs                 # re-exports & feature gates
├── range.rs               # BusRange, redirect modeling
├── bus.rs                 # DeviceBus implementation
├── address.rs             # AddressHandle
├── data.rs                # DataHandle + io::traits impl
├── register.rs            # RegisterHandle integration
├── symbol.rs              # SymbolHandle integration
├── error.rs               # BusError and Result alias
└── ext/
    ├── mod.rs
    ├── stream.rs
    ├── string.rs
    ├── leb128.rs
    ├── float.rs
    ├── crypto.rs
    ├── signed.rs
    └── string_repr.rs
```

Each submodule will ship unit tests focused on functional parity with the .NET version. Integration tests live under `test/soc/bus/` to exercise multi-device maps, redirects, and symbol walking end-to-end.

---

## 9. Implementation Phases

1. **Scaffold core types** (`error.rs`, `device.rs`, `range.rs`, `bus.rs`) with documentation tests covering overlap detection and redirects.
2. **Bring up Address/Data handles** and parity tests for typed loads/stores.
3. **Add Register/Symbol helpers** once the register & variable tables gain Rust bindings.
4. **Port extensions** in order of consumer need (streams first, crypto last).
5. **Hook into SoC** by replacing any ad-hoc memory maps with `DeviceBus` instances; update loader to register firmware memory devices.

This staged approach keeps the module shippable while still converging on full feature parity with the .NET reference implementation.
