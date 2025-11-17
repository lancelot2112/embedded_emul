# Execution Engine Architecture

## 1. Purpose

The execution engine is responsible for simulating the dynamic behavior of a system built from:

* Cores (CPU instances implementing a given ISA/coredef)
* Memory and buses
* Devices (timers, eTPU, peripherals, etc.)

Given decoded instructions and micro-IR from the decoder, the engine:

* Executes micro-IR for each core in a time-aware manner.
* Applies core-specific timing rules and system-level memory/bus timing.
* Manages scheduling and interaction between multiple components.
* Supports deterministic and fuzzed execution orders to expose race conditions.

It is the layer where **time, concurrency, and side effects** actually happen.

The engine should be:

* **Deterministic** given the same seeds and configuration.
* **Configurable** for different cores and systems without changing ISA specs.
* **Instrumentation-friendly** (hooks for tracing, coverage, and fuzzing).

---

## 2. Core Concepts

### 2.1 Components

Everything that evolves over time is modeled as a **Component**:

* CPU cores
* Buses
* Memories (if they have internal timing/queues)
* Devices (timers, eTPUs, DMA engines, etc.)

Each component has:

* A notion of when it wants to be scheduled next (`next_tick`).
* A method to advance its internal state (`tick`).
* A clock relationship to the base system clock (divider or multiplier).

Conceptual trait:

```text
trait Component {
    fn id(&self) -> ComponentId;
    fn next_tick(&self) -> u64;                    // in base cycles
    fn tick(&mut self, now: u64, sys: &mut System) -> u64;
    // returns updated next_tick
}
```

### 2.2 Global Time Base

The engine tracks a **global time** in base cycles:

* `now: u64` — number of base clock ticks since reset.

Cores and devices may run at divided clocks:

* Core0: `clock = base * 1`
* Core1: `clock = base / 2`
* Bus: `clock = base * 1`

Clock relationships from `.coredef`/`.sysdef` translate into how frequently components tick.

---

## 3. Scheduling Models

The engine supports two closely-related scheduling strategies, both built on the same Component abstraction.

### 3.1 Discrete-Event Scheduler

In the discrete-event model, we manage a global min-heap of component wake-up times:

* Priority queue keyed by `(next_tick, ComponentId)`.
* Main loop:

```text
while now < limit {
    let (t, comp_id) = queue.pop_min();
    now = t;
    let comp = &mut components[comp_id];
    let new_next = comp.tick(now, &mut system_state);
    queue.push((new_next, comp_id));
}
```

Characteristics:

* Efficient when many components are idle most of the time.
* Natural fit for devices that only wake on events (e.g. timers, DMA).
* Good for modeling precise times between events (e.g. next timer expiry).

### 3.2 Synchronous "Cycle Box" Scheduler

For reasoning about pipeline-level and bus-level interactions, we sometimes want a more synchronous model:

* Iterate over **each base cycle**:

```text
for cycle in 0..max_cycles {
    now = cycle;

    // 1) determine which components tick this cycle
    let mut active = components
        .iter_mut()
        .filter(|c| (cycle % c.clock_divider()) == 0)
        .collect::<Vec<_>>();

    // 2) resolve ordering for this cycle (deterministic or randomized)
    schedule_policy.order(&mut active, cycle);

    // 3) tick each active component once
    for comp in active {
        comp.tick(now, &mut system_state);
    }

    // 4) advance bus and memory timing by one cycle
    system_state.bus.advance_one_cycle(now);
}
```

Characteristics:

* Simple mental model: "everyone with a clock edge gets to act once per cycle".
* Straightforward place to model race conditions and bus contention.
* Can be layered on top of the discrete-event substrate by treating each `tick` as at most one-cycle work.

### 3.3 Same-Time Arbitration and Fuzzing

When multiple components are active at the same time (same cycle), we must define an ordering:

* **Deterministic** (e.g. fixed ComponentId order) for reproducibility.
* **Randomized** (with seed) for fuzzing race conditions.
* **Custom policies** (e.g. priority-based, round-robin).

Configuration example:

```text
same_time_policy = "randomized(seed=0x1234)";
```

The policy governs:

* Order of core `tick` calls per cycle.
* Order in which the bus arbitrates between outstanding memory requests.

This provides a controllable source of non-determinism for uncovering concurrency bugs while still allowing replay with a known seed.

---

## 4. Core Execution Model

### 4.1 Per-Core State

Each core maintains:

* Architectural state:

  * General-purpose registers, special registers, flags.
  * PC (program counter).
* Microarchitectural state (optional/extended):

  * Pipeline queues
  * Reservation stations, etc. (if modeled)
* Timing:

  * Local cycle counter (core cycles)
  * Clock divider relative to base.

### 4.2 Execution Granularity

The core can execute at one of several granularities:

1. **Instruction-level**

   * Each `tick` executes a whole `DecodedInstr` (all micro-ops at once).
   * Simple and fast, but coarse for timing.

2. **Micro-op level**

   * Each `tick` executes a bounded number of micro-ops.
   * Allows finer-grained placement of memory operations within an instruction.

3. **Cycle-level pipeline** (optional advanced mode)

   * Each `tick` advances pipeline stages by one cycle.
   * Highest fidelity, more complexity.

The architecture assumes at least micro-op-level granularity to model memory timing reasonably:

* Memory IR ops explicitly issue and wait on bus transactions.
* Non-memory IR ops charge cycles via timing classes.

### 4.3 Core Tick Pseudocode

Conceptual per-core `tick` at micro-op granularity:

```text
fn tick(&mut self, now: u64, sys: &mut System) -> u64 {
    let cycles_budget = self.cycles_per_tick();
    let mut used = 0;

    while used < cycles_budget {
        // Fetch/decode if needed
        if self.ir_queue.is_empty() {
            let decoded = self.decode_next(sys.mem)?;
            self.ir_queue.extend(decoded.ir);
            self.current_timing_class = decoded.timing_class;
            self.current_pc = decoded.pc;
            self.current_size = decoded.size;
        }

        // Execute one micro-op
        let op = self.ir_queue.pop_front().unwrap();
        let op_cost = self.exec_micro_op(op, sys)?;

        used += op_cost;

        // If instruction complete, advance PC and reset state
        if self.ir_queue.is_empty() {
            self.pc = self.pc.wrapping_add(self.current_size as u64);
            self.current_timing_class = None;
        }

        // Optional: yield to scheduler / allow preemption injection
        if sys.should_yield(self.id()) {
            break;
        }
    }

    // Compute next_tick based on clock divisor
    now + self.clock_divider
}
```

`exec_micro_op` applies:

* Core timing rules from `.coredef` (mapping timing_class → base latency).
* Memory/bus interactions for load/store ops.

---

## 5. Memory and Bus Timing

### 5.1 Memory Requests and Responses

Memory operations in micro-IR are staged:

* `IssueMemLoad { addr_reg, size, tag }`
* `IssueMemStore { addr_reg, src_reg, size }`
* `WaitMem { tag }`

The core issues a request via the bus and later receives a response.

### 5.2 Bus Component

The **Bus** is a Component responsible for:

* Accepting memory requests from cores/devices.
* Arbitrating which requests start in a given cycle.
* Converting requests into responses with appropriate latency.

Conceptual data structures:

```text
struct MemRequest {
    core_id: ComponentId;
    addr: u64;
    size: u8;
    kind: Load/Store;
    issued_at: u64;
}

struct MemResponse {
    core_id: ComponentId;
    data: u64;        // for loads
    completes_at: u64;
    tag: Tag;
}
```

Bus logic per cycle (simplified):

1. Collect new requests from components.
2. For each memory region / port:

   * Select zero or one request to serve based on arbitration policy.
   * Compute `latency = region.read_latency` or `write_latency`.
   * Schedule a `MemResponse` at `now + latency`.
3. When responses mature (`completes_at == now`), deliver them back to cores/devices.

### 5.3 Memory Regions & System Timing

Memory timing is configured in `.sysdef`:

* `mem region` declarations with:

  * `base`, `size`
  * `read_latency`, `write_latency`
  * `kind` (sram, flash, mmio, etc.)
  * Associated bus.

The bus consults these properties to determine each request's latency.

---

## 6. Timing Integration (ISA/Core/System)

The execution engine is where timing information from three layers is combined:

1. **ISA (`.isa`)**

   * Each instruction has a **timing class**.

2. **Core (`.coredef`)**

   * Maps timing classes to baseline latencies and pipeline behavior.
   * Defines which micro-ops are associated with which timing classes.

3. **System (`.sysdef`)**

   * Specifies memory region and bus timings.
   * Determines final latency for memory-related classes.

At runtime, the engine:

* Uses the timing class of the current instruction to determine non-memory cycle costs.
* For memory ops, combines core behavior with region/bus latency to determine when `WaitMem` can complete.

---

## 7. Concurrency, Preemption, and Fuzzing

### 7.1 Preemption Injection

To validate cross-thread interactions and handshake robustness, the engine supports preemption at instruction or micro-op boundaries:

* Hooks before or after each instruction/micro-op.
* A **preemption controller** that decides when to:

  * Suspend a core.
  * Schedule another core.
  * Inject an interrupt into a core.

Possible strategies:

* Systematic: try preemption at every instruction boundary along a baseline trace.
* Randomized: preempt with some probability per boundary.

### 7.2 Interrupt Handling

Interrupts are modeled as events that:

* Are raised by devices or test harness.
* Are queued into a per-core or shared interrupt controller.
* Cause the core to:

  * Save context (PC, state) according to ISA.
  * Jump to the ISR vector.

The engine must support raising interrupts based on time, device state, or fuzzing strategies.

### 7.3 Determinism and Reproducibility

To support testing and fuzzing:

* All sources of randomness (scheduling, arbitration, preemption decisions) are driven by seeded PRNGs.
* With the same seed and configuration, runs are reproducible.
* Execution traces can be recorded (PC, cycles, memory ops) and replayed.

Optional trace tools:

* Delta reduction to find minimal failing traces.

---

## 8. Instrumentation and Hooks

The engine provides hooks for:

* **Instruction trace**: `(cycle, core_id, pc, instr_id)`.
* **Memory trace**: `(cycle, core_id, addr, size, kind, region)`.
* **Branch trace**: taken/not-taken, targets.
* **Coverage**: which basic blocks or instructions were executed.

Hooks can be installed at:

* Decoder level (when `DecodedInstr` is produced).
* Core execution level (before/after each instruction or micro-op).
* Bus/memory level (on each request/response).

Instrumentation should be cheap to disable in performance-critical runs, but detailed enough for debugging and verification when enabled.

---

## 9. Error Handling and Invariants

The execution engine should enforce key invariants:

* No out-of-bounds memory accesses (unless explicitly allowed for testing).
* No illegal instruction fetches (decode failures) without reporting.
* Consistency of timing (no negative or backwards time).

On violations, the engine can:

* Stop execution and report a detailed error.
* Record a trace segment for debugging.

---

## 10. Integration Points

The execution engine integrates with:

* **Decoder**: consumes `DecodedInstr` units.
* **System Builder**: constructed from `.coredef` and `.sysdef` declarations.
* **User APIs**: Unicorn-like interface for:

  * Mapping memory, loading firmware.
  * Reading/writing registers and memory.
  * Running until PC, cycle limit, or custom condition.
  * Installing hooks and preemption scenarios.

This document defines the responsibilities and behavior of the execution engine; other design docs (Decoder, IR, System/Timing) specify the inputs it consumes and the interfaces it exposes.
