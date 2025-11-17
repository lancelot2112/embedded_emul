# System Builder Architecture

## 1. Purpose

The **System Builder** is responsible for assembling a complete emulated system from declarative specifications:

* ISA specifications (`.isa`, `.isaext`)
* Core definitions (`.coredef`)
* System definitions (`.sysdef`)

It constructs all cores, memory regions, buses, devices, timing relationships, and scheduler configuration into a coherent, runnable unit. The builder produces a `System` object consumed by the **Scheduler** and **Execution Engine**.

The System Builder is effectively the "linker" and "wiring harness" of the emulator.

---

## 2. Inputs

### 2.1 ISA Specifications

Contain:

* Instruction patterns and decode tables
* Operand schemas
* Semantic templates
* Timing classes

These are needed to construct core decoders and micro-IR generators.

### 2.2 Core Definitions (`.coredef`)

Define:

* ISA variant
* Endianness
* Register file shape
* Clock rate / divider
* Pipeline and timing model
* Timing class mappings

Each `.coredef` describes a reusable *core type*.

### 2.3 System Definitions (`.sysdef`)

Define:

* Cores instantiated from core types
* Memory map and memory regions
* Buses and bus arbitration settings
* Device instantiation and wiring
* Clock relationships between components
* Scheduler strategy and same-time policy

A `.sysdef` produces a **concrete SoC configuration**.

---

## 3. Outputs

The System Builder produces a tree of interconnected runtime objects:

* `System` — container for all instantiated components.
* `Core` instances — carry architectural and microarchitectural state.
* `Bus` instances — arbitrate memory requests.
* `MemoryRegion` objects — model SRAM, flash, MMIO regions.
* `Device` instances — timers, interrupt controllers, DMA engines, etc.
* `SchedulerConfig` — strategy, ordering policy, seeds.

The builder also produces:

* `DecodeTables` per core type
* Timing lookup tables derived from core/system configs
* Clock resolution plan (which components tick at which cycles)

---

## 4. Build Pipeline

The System Builder runs through well-defined phases:

### Phase 1 — Parse & Load Specifications

1. Load `.isa` and `.isaext` files.
2. Load `.coredef` files.
3. Load a `.sysdef` file.
4. Validate that required ISA and core types are available.

### Phase 2 — Construct Decode Structures

For each core type:

* Compile pattern tables into `DecodeTable` sets.
* Precompile semantic templates into micro-IR templates.
* Bind timing classes.

Output: a `CoreTypeDescriptor` containing:

* Decoder tables
* Semantic template map
* Timing class mapping
* Register file description

### Phase 3 — Instantiate Cores

For each core instance declared in `.sysdef`:

* Clone/instantiate from the `CoreTypeDescriptor`.
* Apply instance-specific parameters (e.g. initial PC, memory map visibility).
* Set up architectural registers.
* Initialize pipeline/micro-op buffers.
* Assign clock divider relative to base system clock.

### Phase 4 — Instantiate Memory Regions

For each memory region:

* Allocate backing memory buffer or MMIO handler.
* Attach region to appropriate bus.
* Record region timing: `read_latency`, `write_latency`, access width.
* Build memory map index for fast lookup.

### Phase 5 — Instantiate Buses

For each bus:

* Create bus component with:

  * Arbitration policy
  * Port count
  * Clock config
* Attach cores and memory regions to bus routing tables.

### Phase 6 — Instantiate Devices

For each device in `.sysdef`:

* Create device instance with given parameters.
* Assign to the proper bus/interrupt lines.
* Initialize device clock domains.

### Phase 7 — Assemble Interrupt Topology

* Build interrupt controller from system definition.
* Link cores to interrupt lines.
* Map device IRQ outputs to controller inputs.

### Phase 8 — Construct Scheduler Configuration

* Choose scheduler strategy (cycle-box or discrete-event).
* Configure same-time policy.
* Configure random seeds for deterministic replay.
* Build priority lists if needed.

### Phase 9 — Final System Assembly

Create a `System` object containing:

* All components (`Core`, `Bus`, `Device`)
* Global memory map
* Clock/timing resolver
* Interrupt topology
* Scheduler configuration

Return `System` to runtime.

---

## 5. Validation & Consistency Checks

The System Builder performs extensive validation:

* Check overlapping memory regions.
* Validate that addresses fall into exactly one region.
* Ensure each core has exactly one memory/bus connectivity path.
* Ensure all timing classes referenced in `.coredef` exist in the ISA.
* Verify device IRQs are wired.
* Validate clock divisor consistency.
* Ensure decode tables cover all specified instruction patterns.

If any checks fail, the builder emits precise error diagnostics.

---

## 6. Runtime Data Structures

The System Builder constructs or configures these runtime structures:

### 6.1 System

```text
struct System {
    cores: Vec<Core>,
    buses: Vec<Bus>,
    devices: Vec<Box<dyn Device>>,
    memory_map: MemoryMap,
    scheduler: SchedulerConfig,
    now: u64,
}
```

### 6.2 Core Instance

Includes:

* Architectural registers
* Decoder reference (tables + templates)
* Pipeline or micro-op queue
* Timing class lookup table
* Clock divisor

### 6.3 MemoryMap

Accelerates `addr -> region` lookup:

* Range table
* Optional 2-level page map
* Region descriptors (latencies, bus, permissions)

### 6.4 Bus Instance

* Arbitration policy
* Request/input queues
* Response queues
* Clocking

### 6.5 Device Instances

Unified interface via trait object.

---

## 7. System Builder API Sketch

### Build From Spec

```rust
let system = SystemBuilder::new()
    .load_isa("ppc_vle.isa")
    .load_coredef("e200v9.coredef")
    .load_sysdef("dualcore.sysdef")
    .build()?;
```

### Build From In-Memory Structures

```rust
let sys = SystemBuilder::from_specs(isa, coredefs, sysdef).build()?;
```

### Result

You receive a fully-wired `System`:

```rust
env.run(system)?;      // scheduler + execution engine
```

---

## 8. Extensibility

The System Builder is designed to be easy to extend:

* Adding new device types requires defining a Device implementor and updating the builder registry.
* Adding new buses or interconnects simply adds new components.
* Supporting new ISA extensions requires extending `.isaext` and decode tables.
* Additional timing models (e.g. caches, TLBs) integrate via device or bus components.

---

## 9. Summary

The System Builder:

* Reads and validates declarative system specifications.
* Constructs cores, memories, buses, and devices.
* Connects all components according to timing and wiring rules.
* Produces a fully-realized `System` object ready for scheduling and execution.

It is the "assembly line" that produces the emulated SoC from your declarative specs.
