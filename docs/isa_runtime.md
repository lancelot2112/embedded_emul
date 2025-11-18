# ISA Runtime Pipeline (WIP)

This repository now contains an initial scaffolding for turning `.isa` / `.isaext` sources into
runtime data structures that can power disassembly and semantic IR generation.

## Module Overview

- `src/soc/isa/lexer.rs` – streaming tokenizer. Currently stubbed; porting the existing linter
  tokenizer is the next task.
- `src/soc/isa/parser.rs` – produces AST nodes defined in `ast.rs`.
- `src/soc/isa/validator.rs` – cross-file semantic checks and conversion into a
  `MachineDescription` (see `machine.rs`).
- `src/soc/isa/loader.rs` – orchestrates include resolution, parsing, validation.
- `src/soc/isa/handle.rs` – public API exporting `IsaHandle`, suitable for construction inside the
  system bus. Consumers can disassemble byte ranges and fetch semantic blocks by mnemonic.
- `src/soc/isa/semantics.rs` – placeholder IR that will eventually describe the semantic blocks that
  appear in `.isa` files.

## Near-Term TODOs

1. Port the tokenizer rules from `docs/spec/isa_language_specification.md` into `lexer.rs`.
2. Implement the directive grammar in `parser.rs` and populate real AST nodes.
3. Expand `validator.rs` to enforce the rules outlined in the spec (duplicate detection, range
   checking, mask validation, etc.).
4. Teach `MachineDescription::disassemble` to consult bitfield metadata when decoding opcodes using
   the existing `BitFieldSpec` readers.
5. Flesh out the semantic IR so downstream passes can emit a richer internal representation for
   execution or translation.
