# Semantic Architecture

## Goal

Provide a reusable execution layer that turns the DSL embedded in `.isa` files into concrete mutations on a `CoreState`. The runtime must evaluate expressions, honor register/host/macro/instruction calls, and surface deterministic side effects so CPU models can emulate families of instructions (PowerPC `add*`, etc.) without bespoke code. This document stitches together the parser, runtime, machine metadata, and bitfield utilities so we know exactly which crates to wire up when finishing the interpreter.

## Example Walkthrough

The POWER PPC snippet below illustrates the full stack:

```
:space reg addr=32 word=64 type=register align=16 endian=big
:space insn addr=32 word=32 type=logic align=16 endian=big

:reg GPR[0..31] offset=0x0 size=64 reset=0 disp="r%d"
subfields={
    msb @(0..31)
    lsb @(32..63)
}

:reg SPR[0..1023] offset=0x1000 size=64
subfields={
    msb @(0..31)
    lsb @(32..63)
}

:reg XER redirect=SPR1
subfields={
    SO @(32)
    OV @(33)
    CA @(34)
}

:reg CR[0..7] offset=0x900 size=4
subfields={
    NEG @(0)
    POS @(1)
    ZERO @(2)
    SO @(3)
}

:insn X_Form subfields={
    OPCD @(0..5) op=func
    RT @(6..10) op=target|$reg::GPR
    RA @(11..15) op=source|$reg::GPR
    RB @(16..20) op=source|$reg::GPR
    XO @(21..30) op=func
    Rc @(31) op=func
} disp="#RT, #RA, #RB"

:macro upd_cr0(res) {
    $reg::CR0::NEG = #res < 0
    $reg::CR0::POS = #res > 0
    $reg::CR0::ZERO = #res == 0
    $reg::CR0::SO = $reg::XER::SO
}

:insn::X_Form add mask={OPCD=31, XO=266, Rc=0} descr="Add (X-Form)" op="+" semantics={
    a = $reg::GPR(#RA)
    b = $reg::GPR(#RB)
    (res,carry) = $host::add_with_carry(a,b,#SIZE_MODE)
    $reg::GPR(#RT) = res
    (res,carry)
}

:insn::X_Form add. mask={OPCD=31, XO=266, Rc=1} descr="Add and record (X-Form)" op="+" semantics={
    $insn::add(#RT,#RA,#RB)
    $macro::upd_cr0(res)
}
```

* `MachineDescription` ingests the register and instruction spaces, associating operands with bitfield specs and register bindings.
* `SemanticProgram` (from `semantics/program.rs`) parses the DSL for both `add` and `add.` into IR statements.
* The runtime resolves operand placeholders (`#RT`, `#D`, etc.) using `BitFieldSpec::read_signed` and calls back into `CoreState` to fetch/write registers.
* Host helpers run via `HostServices::add_with_carry`, and macros/instructions become callable sub-programs, enabling the `add.` instruction to share the `add` mutations and then extend behavior.

## Actionable Requirements

1. **Register access**: `$reg::<space>(index)[::subfield]` must map to `CoreState::read_register`/`write_register` calls using metadata from `CoreSpec` and `MachineDescription` (including redirects and subfields).
2. **Host helpers**: `$host::<fn>` invocations must route to the active `HostServices` implementation, passing masked values and width hints coming from the ISA parameters (e.g., `#SIZE_MODE`).
3. **Instruction/macro dispatch**: `$insn::foo(...)` and `$macro::bar(...)` evaluate nested `SemanticProgram`s with their own argument scopes while sharing the same `CoreState` and `HostServices` handles.
4. **Tuple semantics**: Allow `(res, carry)` style tuple assignment/returns. The runtime must track multi-value temporaries and enforce arity checks when binding tuple targets.
5. **Parameter binding**: `#RA`, `#imm`, etc., originate from decoder operands. The interpreter needs an input map keyed by operand name plus optional instruction parameters defined via `:param`.
6. **Expression evaluation**: Implement logical, bitwise, relational, arithmetic, and bit-slice operators exactly as encoded in `SemanticProgram::Expr`.
7. **State isolation**: Each execution uses a scratch environment (variables defined via `a = ...`) without leaking to future invocations, while still mutating the shared `CoreState`/`HostServices` as side effects.
8. **Error reporting**: Surface `IsaError::Machine` diagnostics that pinpoint illegal operations (unknown register, tuple arity mismatch, unsupported host call) to aid ISA authors.

## Library Touchpoints

- `soc/isa/semantics/program.rs`: Produces the `SemanticProgram`, `SemanticStmt`, `Expr`, and assignment targets the runtime must interpret.
- `soc/isa/semantics/runtime.rs`: Home of the interpreter; depends on the pieces listed here.
- `soc/prog/types/bitfield.rs`: `BitFieldSpec::read_signed` and `read_bits` convert container words into properly extended operands, eliminating manual sign logic.
- `soc/core/specification.rs` and `soc/core/state.rs`: Provide `CoreSpec` (layout metadata) and `CoreState` (mutable register file backed by `DeviceBus` and `BasicMemory`). Use `CoreState::read_register`, `write_register`, and bit-slice helpers for subfield access.
- `soc/isa/machine/register.rs`, plus the symbol plumbing in `soc/prog/symbols::*` and `soc/system/bus/symbol::*`: `RegisterSchema::lookup` yields `RegisterMetadata` + per-element `SymbolHandle`s, and the symbol readers/writers can already map those handles onto the `DeviceBus`, so register helpers should reuse them instead of recomputing offsets.
- `soc/isa/machine/host.rs`: Defines `HostServices`, `HostArithResult`, `HostMulResult`, and the `SoftwareHost` fallback. Runtime should accept any `HostServices` impl so tests can inject deterministic behavior.
- `soc/isa/machine/mod.rs` and `soc/isa/machine/space.rs`: Carry operand ordering, register bindings, and form metadata needed to resolve operand names to `BitFieldSpec`s.
- `soc/isa/machine/macros.rs`: Macro bodies (`MacroInfo`) are exposed here; runtime must look up macro semantics via this registry.
- `soc/isa/semantics.rs`: `SemanticBlock::ensure_program` compiles raw source strings; execution should request the compiled program lazily to amortize parse costs.

When finishing `runtime.rs`, keep these dependencies in mind: parse once, evaluate many times; rely on `BitFieldSpec` for operand extraction; and treat `CoreState` as the single source of truth for all architectural state mutations.

## Build Plan

1. **Runtime Scaffold**
    - Implement `SemanticValue`, tuple handling, and an execution context inside `runtime.rs` (file-level doc comment summarizing scope).
    - Tests: verify scalar/tuple conversions, tuple arity checks, and environment scoping for locals vs. parameters.

2. **Operand & Parameter Binding**
    - Resolve decoder operands/parameters into a map using `BitFieldSpec::read_signed` and instruction metadata.
    - Tests: ensure `#RT`, immediates, and negative values decode correctly with sign extension.

3. **Register Access Helpers**
    - Add helpers translating `$reg::SPACE(name)[::subfield]` into `CoreState` reads/writes, honoring redirects and subfields.
    - Tests: mock `CoreState` to confirm full register and subfield access plus error reporting for unknown targets.

4. **Expression Evaluator**
    - Evaluate all `Expr` variants (numbers, parameters, calls, bit slices, binary ops).
    - Tests: cover logical/bitwise/arithmetic ops, nested expressions, and invalid operations raising `IsaError`.

5. **Statement Interpreter**
    - Support `Assign`, `Expr`, and `Return`, including tuple destructuring and propagation of return values.
    - Tests: mini-programs exercising variable assignment, tuple returns, and side-effect-only expressions (e.g., macros updating flags).

6. **Call Dispatch**
    - Implement `$host::`, `$macro::`, `$<logicspacetag>::`, and register calls with recursion/stack checks.
    - Tests: host helper invocation (`add_with_carry`), macro chaining (e.g., `add.` calling `add` then CR update), and instruction-to-instruction reuse.

7. **Integration Harness**
    - Load PPC `add` semantics, run against a `CoreState` with seeded registers, and assert register + CR outcomes for `add`, `add.`, `addo`, `addo.`.
    - Tests: scenario-level assertions ensuring runtime, host services, and register helpers cooperate end-to-end.