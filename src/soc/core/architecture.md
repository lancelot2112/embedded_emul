# Core Runtime Architecture

## Intent

Describe how `CoreSpec` and `CoreState` evolve into a typed register file backed by the existing `soc::prog::types` system. The goal is to expose structured views of architectural registers (and their subfields) so the semantics runtime can read/write registers and individual bitfields without duplicating packing logic.

## Register Typing Strategy

1. **Structure-per-register definition**
   - Every `:reg` declaration in the ISA machine maps to a `StructureType` (see `soc::prog::types::structure`).
   - The structure name mirrors the register symbol (`GPR`, `SPR`, `CR`, etc.).
   - Members derive from the register's subfield declarations; each member carries a `BitFieldSpec` describing its bit slice within the register container.
2. **Register arrays**
   - Range declarations (`:reg GPR[0..31]`) produce an `ArrayType<Structure<GPR>>` describing the number of lanes and the stride.
   - Scalar registers (no range) still reuse the structure type but are wrapped in a single-element array for uniformity when populating the register file.
3. **Symbol handles for registers**
   - Each structure/array pair is also registered as a `Symbol` so downstream systems can navigate registers via the existing symbol table APIs.
   - Individual register slots (`GPR0`, `SPR1`, etc.) become symbol instances pointing at the shared structure type plus their array index. Redirects reuse the same symbol handle as their target, avoiding duplicate metadata.
   - Any optimizations in the symbol subsystem (caching, lazy decoding, debug metadata) now apply directly to register access.
3. **CoreSpec metadata**
   - `CoreSpec::from_machine` records both the physical layout (bit offsets/lengths) and the associated type handle for each register "slot".
   - Registers are keyed using the same `space::field` notation already emitted today, ensuring compatibility with `CoreState::read_register`/`write_register`.

## Data Flow

```
MachineDescription (register spaces) -> StructureType definitions
                                 \-> ArrayType metadata (counts, names)
                      CoreSpec::from_machine consumes both to emit:
                        * Register layout entries (bit offset/len)
                        * Type handles (structure + array) per logical register
CoreState instantiates backing memory sized to total bits
Typed Register Helpers look up the array + structure to interpret bits
```

## Runtime Usage

- **Reads**: `$reg::GPR(#RA)::lsb`
  1. Resolve `GPR` structure type and `ArrayType` metadata via `CoreSpec`.
  2. Evaluate `#RA` to pick the array element.
  3. Use the structure definition to compute the subfield's `BitFieldSpec` and mask/apply it over the `CoreState` bits.
- **Writes**: Reverse process—mask in the new value via the structure's bitfield definition and write the updated register back through `CoreState`.
- **Redirects**: Register definitions that redirect to another register share the same structure type; the redirector simply reuses the target array metadata and structure handle.

## Integration Points

- `soc::isa::machine::space`: during form/register ingestion, capture subfield metadata in a shape usable for structure generation.
- `soc::prog::types`: extend/instantiate `StructureType` and `ArrayType` entries for each register space.
- `soc::prog::symbols`: emit `SymbolHandle`s for each register type/instance so the runtime can traverse register files via the symbol navigator.
- `soc::core::specification`: store references to the generated types alongside existing layout info.
- `soc::core::state`: expose helpers that accept a type handle + index and return typed views (or at least typed masks) for the semantics runtime.
- `soc::isa::semantics::runtime`: register helpers query the typed metadata instead of reconstructing bit slices ad-hoc.

## Next Steps

1. Implement the structure/array emission pass when building the machine description.
2. Register the emitted structures/arrays as `Symbol`s and plumb the handles through `MachineDescription` → `CoreSpec`.
3. Extend `CoreSpec` to hold both type and symbol handles for each register slot.
4. Teach `CoreState` to surface typed access helpers (e.g., `get_struct(&RegisterHandle)` returning a proxy that knows the bitfields) backed by symbol lookups.
5. Update the semantics architecture doc to call out that register helpers depend on these typed definitions.
