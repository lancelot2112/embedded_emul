# Parser Architecture

The parser is structured as a set of focused modules that cooperate through the `Parser` type defined in `document.rs`.

- `document.rs`
  - Owns the `Parser` struct, cursor helpers (`peek`, `consume`, `expect`, etc.), and `parse_document` loop that walks the token stream.
  - Exposes the ergonomic `parse_str` helper the loader uses when it only needs a one shot parse.
- `directives.rs`
  - Dispatches to directive-specific modules via extension `impl`s on `Parser`.
  - Guarantees that each directive consumes everything up to the next `:`; leftover tokens are collected and surfaced as parser errors so individual handlers stay simple.
  - Hosts tests for the shared directive infrastructure plus lightweight cases for directives implemented inline (e.g., `:param`).
- `space.rs`
  - Contains the full `:space` parser, including attribute validation, numeric literal decoding, and space tag registration.
  - Includes its own tests that exercise success/error cases so regressions stay localized.
- `space_context.rs`
  - Handles `:<space_tag>` contexts for non-logic spaces, parsing register forms, attribute lists, and nested `subfields={}` blocks.
  - Reuses the shared `Parser` helpers plus numeric literal routines to keep directive-specific logic focused on structure rather than token management.
- `parameters.rs`
  - Shared routines for decoding directive payloads into `ParameterDecl` values.
  - Leans on helper methods from `Parser` for token management and `parse_numeric_literal` when numbers are encountered.
- `literals.rs`
  - Centralizes numeric literal parsing so that directives and parameters stay focused on higher level concerns.
  - Includes lightweight unit tests that pin down overflow and sign handling rules.

`mod.rs` remains intentionally small: it wires the modules together, re-exports the public API, and exposes the shared lexer token types to the submodules. Adding a new directive normally involves editing only `directives.rs` (for the syntax) and, if necessary, extending `parameters.rs` or `literals.rs` for reusable helpers.
