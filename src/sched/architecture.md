# Scheduler Architecture

## 1. Purpose

The scheduler is the subsystem responsible for determining **when** each component in the system runs. It manages global simulated time, resolves ordering among simultaneously active components, and orchestrates concurrency, preemption, and fuzzing behaviors.

**The scheduler does *not*** execute instructions or micro-ops. That is the job of the **executor** inside each component.

Instead, the scheduler provides:

* A coherent notion of global simulated time.
* A policy for selecting which components should run at each time step.
* Deterministic and randomized ordering strategies.
* Hooks and control points for preemption and interrupt injection.
* Coordination between multiple cores, buses, memory systems, and devices.

It is the "traffic controller" of the emulator.

---

## 2. Core Responsibilities

The scheduler:

1. **Maintains global time** (`now: u64` in base cycles).
2. **Determines when each component becomes eligible to run**, based on:

   * Clock dividers in `.coredef` / `.sysdef`.
   * Device-specific wake-up times (e.g. timers).
   * Bus/memory event timings.
3. **Invokes Component::tick()** in an order defined by a configurable policy.
4. **Supports two modes** of scheduling:

   * Discrete-event scheduling (event-driven).
   * Cycle-box scheduling (cycle-accurate synchronous).
5. **Coordinates preemption, interrupts, and concurrency fuzzing**.
6. **Ensures reproducibility** when given explicit random seeds.

The scheduler operates over the set of **components**, each exposing:

```text
type ComponentId = u32;

trait Component {
    fn id(&self) -> ComponentId;
    fn next_tick(&self) -> u64;                    // next time (in base cycles) the component should run
    fn tick(&mut self, now: u64, sys: &mut System) -> u64; // returns new next_tick value
}
```

This abstraction allows cores, buses, timers, DMA engines, and other devices to participate uniformly.

---

## 3. Scheduling Models

The scheduler implements two complementary strategies that use the same data model:

### 3.1 Discrete-Event Scheduler (Event-Driven)

The discrete-event scheduler advances time to the next interesting event:

```text
let (wake_time, comp_id) = priority_queue.pop_min();
now = wake_time;
let next = components[comp_id].tick(now, &mut system);
priority_queue.push((next, comp_id));
```

**Characteristics:**

* Efficient when many components are idle.
* Ideal for timers, DMA engines, and low-frequency peripherals.
* Time jumps directly to the next scheduled event.
* Good for functional correctness simulations.
* Less precise for tightly-timed interactions unless augmented with cycle-granular behavior.

### 3.2 Cycle-Box Scheduler (Cycle-Accurate Synchronous)

A synchronous scheduler advances in fixed increments (base cycles):

```text
for cycle in 0..max_cycles {
    now = cycle;

    let mut active = components
        .iter_mut()
        .filter(|c| cycle % c.clock_divider() == 0)
        .collect::<Vec<_>>();

    same_time_policy.order(&mut active, cycle);

    for comp in active {
        comp.tick(now, &mut system);
    }

    system.bus.advance_one_cycle(now);
}
```

**Characteristics:**

* Exact time-step progression.
* Serves cores and bus interactions with highest fidelity.
* Straightforward place to introduce concurrency fuzzing.
* Simplifies reasoning about races and overwrites.

### 3.3 Selecting a Scheduler

The scheduling strategy is configured via a runtime or build-time setting:

```text
scheduler_strategy = "cycle_box";

# or
scheduler_strategy = "discrete_event";
```

The system builder is responsible for instantiating the proper scheduler.

---

## 4. Same-Time Ordering

When multiple components become active at the same simulated time (e.g. two cores at cycle 100), a **same-time policy** governs the order in which they are ticked.

### 4.1 Policy Options

**Deterministic:** fixed ordering by ComponentId or explicit priority.

```text
same_time_policy "deterministic";
```

**Randomized:** insertion of controlled nondeterminism for race fuzzing.

```text
same_time_policy "randomized(seed=0xCAFEBABE)";
```

**Priority-based:** components specify priority classes (e.g. bus before cores).

```text
same_time_policy "priority(bus > core0 > core1)";
```

### 4.2 Purpose

Same-time ordering affects:

* Instruction interleavings
* Bus arbitration
* Memory request ordering
* Device/interrupt timing

This gives the scheduler powerful control over concurrency behavior and reproducibility.

---

## 5. Clocks, Dividers, and Time Domains

All timing is expressed relative to the **base clock** defined in `.sysdef`.

### 5.1 Core Clocking

Cores may run at different speeds:

```text
core e200v9_0 { clock base * 1; }
core e200v9_1 { clock base / 2; }
```

A core is active in a cycle when:

```text
(now % core.clock_divider) == 0
```

### 5.2 Bus and Device Clocks

Likewise for buses and devices:

```text
bus ahb0 { clock base * 1; }
device timer0 { clock base / 4; }
```

This enables:

* Lower-frequency peripheral modeling
* Asynchronous device behavior
* Realistic SoC-level timing differences

---

## 6. Preemption & Interrupt Control

The scheduler collaborates with a **preemption controller** to reproduce or explore concurrency behaviors.

### 6.1 Instruction-/Micro-Op Boundary Preemption

The scheduler can:

* Interrupt a core at instruction boundaries
* Or at micro-op boundaries for tighter control

### 6.2 Strategies

**Systematic:** try preemption at every boundary (one-by-one across a trace).

**Randomized:** with probability `p` per boundary.

**Targeted:** preempt when PC equals a specific address.

### 6.3 Interrupt Injection

Interrupt events are queued and delivered by the scheduler as appropriate:

* Based on time
* Based on device signals
* As part of fuzzing mode

This allows testing of interrupt-driven protocols and handshake logic.

---

## 7. Determinism and Replay

The scheduler is a major source of nondeterminism — therefore:

* All nondeterministic decisions use PRNGs with user-supplied seeds.
* Given the same seeds and configuration, execution is **fully reproducible**.
* The scheduler can record traces of all decisions for debugging.

Example configuration:

```text
scheduler_strategy   "cycle_box";
same_time_policy     "randomized(seed=0x1234ABCD)";
preemption_seed      0xDEADBEEF;
arbitration_seed     0xCAFEBABE;
```

---

## 8. Integration Points

The scheduler integrates with:

### 8.1 Execution Engine

Scheduler calls into components via `tick(now, &mut System)` but does **not** interpret instructions.

### 8.2 System Builder

Takes `.coredef` and `.sysdef` clocking, component list, and timing info to construct the schedule map.

### 8.3 Debugger / Fuzzing Framework

Fuzzing tools plug into the scheduler:

* Reorder components
* Vary arbitration
* Inject preemptions
* Sweep seeds systematically

---

## 9. Summary

The scheduler is:

* **Global** (controls simulated time for the entire SoC).
* **Configurable** (two scheduling strategies, multiple ordering policies).
* **Deterministic when needed**, randomized when exploring concurrency.
* **Responsible only for *when* components run**, not *what* they do.

This separation keeps the emulator modular, testable, and extensible across different ISAs, cores, and SoC configurations.
