# NAnEmu 

*A declarative, data-driven framework for defining ISAs, building cores, modeling embedded SoCs, and executing firmware for testing, fuzzing, and verification.*

`NAnEmu (Not Another Emulator)` is a language + tooling ecosystem for describing instruction sets (ISAs), architecture extensions, core configurations, and full SoC/system definitions in a concise, declarative format—then using these definitions to automatically build decoders, lifters, emulators, and verification environments.

The long-term goal is to provide a clean alternative to QEMU/Unicorn for embedded architectures where correctness, modularity, and testability matter more than raw performance.
This includes:

* **TriCore 1.6.2**
* **PowerPC e200v9 + VLE**
* **Multi-core automotive MCUs (e.g., MPC5777C)**
* **System devices (eTPU, EBI, MMU, IRQ fabric)**

…and a test harness for:

* Unit-testing isolated functions from real firmware
* Simulating scheduling, preemption, context switches, and interrupt-driven handshakes
* Fuzzing firmware paths by forcing preemption at arbitrary instruction boundaries

---

## ✨ Key Features (Current & Planned)

### ✔ Declarative ISA definitions

Define an ISA in `.isa` files:

* Register files
* Instruction formats and encodings
* Syntax and operands
* Instruction semantics in a tiny functional DSL

Extensions (e.g., **VLE**, **FPU**, **TriCore DSP**) can be layered as `.isaext` files.

### ✔ Declarative core & system definitions

Use `.coredef` to define:

* Endianness
* Alignment rules
* VLE mode
* MMU/paging model
* Exceptions/interrupt vector layout
* Pipeline detail (optional)

Use `.sysdef` to define:

* SoC topology (cores, shared memory, on-chip peripherals)
* Memory/bus layout
* MMIO device models
* IRQ routing
* Timers, DMA channels, etc.

### ✔ Auto-generated decoders

From ISA definitions, the toolchain generates:

* Efficient decode trees
* Instruction metadata
* A micro-IR form of each instruction’s semantics

### ✔ Execution engine (in progress)

Interprets/lifts ISA micro-IR into:

* A **portable interpreter** (zero dependencies)
* A later **JIT/emitter** for speed (Cranelift or custom)

Provides a **Unicorn-like API**:

```rust
emu.mem_map(addr, size, perms);
emu.mem_write(addr, data);
emu.reg_write(Reg::R3, 42);
emu.start(start_pc, until_pc, max_insns);
```

### ✔ ELF loading + function-level testing

Load full firmware images, jump to specific functions, and unit-test:

* argument passing
* stack usage
* register effects
* memory side-effects

### ✔ Preemption & interrupt fuzzing (planned)

Automatically validates concurrency-sensitive firmware:

* Interrupt at *any* instruction boundary
* Simulate multicore preemption
* Validate ISR/handshake correctness
* Produce minimal failing traces when a violation occurs

This is especially useful for automotive / safety-critical embedded systems.

---

## 🧠 Why this exists

Existing emulators like **QEMU** and **Unicorn** are powerful but:

* Difficult to extend
* Hard to target partial, proprietary, or variant ISAs
* Not designed for **unit testing firmware**
* Not built for **system-level preemption fuzzing**
* Carry a huge amount of historical complexity

This project takes the opposite approach:

1. **Data-driven instead of code-driven**
   → All decoders, cores, and systems come from declarative specs.

2. **Modular instead of monolithic**
   → You choose only the pieces your SoC requires.

3. **Verification-first instead of performance-first**
   → Every instruction boundary is testable and hookable.

4. **IR-driven instead of ad-hoc semantics**
   → ISA semantics compile to a uniform micro-IR
   → Enables clean interpreters, JITs, symbolic analysis, and testing.

---

## 📁 Project Layout

```
nanemu/
├── test/
│   ├── tricore/         # TriCore 1.6.2 definitions (WIP)
│   ├── ppc_vle/         # PowerPC e200v9 + VLE definitions (WIP)
│   └── mpc5xxx/         # Example system with multiple cores + devices
├── defs/
│   ├── core/            # .isa/.isaext/.coredef for various archs
│   └── system/          # .sysdef files for various SoC's
├── docs/
├── src/
│   ├── isa/             # Parser + schema for .isa / .isaext
│   ├── core/            # Core builder from .coredef
│   ├── system/          # SoC builder from .sysdef
│   ├── decode/          # Decoder generation
│   ├── ir/              # Micro-IR (lower-level instruction representation)
│   ├── exec/            # Interpreter engine
│   ├── loader/          # Executable(.elf,.exe)/Binary(.hex,.srec)/Overlay(.cdfx)/Config(.a2l) loader
│   └── api/             # Unicorn-like API surface
└── README.md
```

---

## 🧪 Example: Testing a Function

```rust
let mut emu = Emulator::from_sysdef("mpc5777c.sysdef");

// Load firmware ELF
emu.load_elf("firmware.elf");

// Set arguments
emu.reg_write(Reg::R3, 10);
emu.reg_write(Reg::R4, 20);

// Run until return
emu.start(foo_entry_pc, Some(foo_return_pc), 1_000);

// Assert correct result
assert_eq!(emu.reg_read(Reg::R3), 30);
```

---

## 🚦 Roadmap

### Phase 1 — Core infrastructure (WIP)

* [ ] Complete `.isa` → micro-IR compiler
* [ ] VLE decode tree
* [ ] TriCore 1.6.2 decode tree
* [ ] Minimal interpreter
* [ ] ELF loader
* [ ] Memory system + bus

### Phase 2 — System-level features

* [ ] Multi-core scheduler
* [ ] Interrupt controller + exceptions
* [ ] Device modeling (eTPU, EBI, timers, etc)

### Phase 3 — Verification tools

* [ ] Preemption/interrupt fuzzing engine
* [ ] State snapshots + deterministic replay
* [ ] Trace compression for minimal failing examples

### Phase 4 — Optimization

* [ ] Micro-IR → JIT backend
* [ ] Basic-block caching
* [ ] Portability layers for embedded simulators

---

## 🤝 Contributing

Contributions are welcome!

* New ISAs
* Extensions (e.g., VLE, DSP, FPU)
* Device models
* System definitions
* Improvements to the IR or interpreter
* Fuzzing tools & test harnesses

Open an issue or PR if you’d like to collaborate.
