# Parser Architecture

The parser is structured as a set of focused modules that cooperate through the `Parser` type defined in `document.rs`.

- `document.rs`
  - Owns the `Parser` struct, cursor helpers (`peek`, `consume`, `expect`, etc.), and `parse_document` loop that walks the token stream.
  - Exposes the ergonomic `parse_str` helper the loader uses when it only needs a one shot parse.
- `directives.rs`
  - Adds directive specific behavior via extension `impl`s on `Parser`.
  - Currently supports both the `:fileset` and `:param` directives and remains the place to bolt on more handlers.
  - Hosts directive tests so examples stay next to the code they describe.
- `parameters.rs`
  - Shared routines for decoding directive payloads into `ParameterDecl` values.
  - Leans on helper methods from `Parser` for token management and `parse_numeric_literal` when numbers are encountered.
- `literals.rs`
  - Centralizes numeric literal parsing so that directives and parameters stay focused on higher level concerns.
  - Includes lightweight unit tests that pin down overflow and sign handling rules.

`mod.rs` remains intentionally small: it wires the modules together, re-exports the public API, and exposes the shared lexer token types to the submodules. Adding a new directive normally involves editing only `directives.rs` (for the syntax) and, if necessary, extending `parameters.rs` or `literals.rs` for reusable helpers.
