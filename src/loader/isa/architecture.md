# Loader ISA Architecture

The loader/isa module is responsible for turning raw ISA source files into the strongly typed `IsaDocument` model used by the rest of the emulator. It is intentionally split into two primary layers:

1. **Lexing (`lexer.rs`)** – Performs the lightweight scan over the source text and produces normalized tokens. The lexer stays focused on character level concerns: handling comments, punctuation, quoted strings, and the numeric literal grammar. Because the parser relies on token kinds rather than raw text, all syntax edge cases (underscores in numbers, different radix prefixes, etc.) are hidden behind the lexer.
2. **Parsing (`parser/`)** – Consumes the token stream and builds `IsaItem` entries. The parser is now a directory so that each concern lives in a small, testable file:
   - `document.rs` owns the `Parser` struct, cursor management (`peek`, `consume`, etc.), and `parse_document` entry point.
   - `directives.rs` contains the `:directive` dispatch and directive specific logic (currently `:fileset` and `:param`).
   - `parameters.rs` converts directive payloads into `ParameterDecl` instances, including value decoding.
   - `literals.rs` houses helpers such as `parse_numeric_literal` that need to be shared across directives.

This split keeps `mod.rs` free of implementation so it can act as the public surface (`Parser`, `parse_str`) while wiring the internal modules together. Tests now live beside the code they exercise (e.g., directive tests in `directives.rs`), which should make it clear where to extend behavior as additional directives are implemented.
