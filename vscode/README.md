# Nanemu ISA VS Code Extension

This TypeScript project hosts the VS Code side of the Nanemu ISA language server. It launches the
Rust backend from this repository and wires diagnostics into the editor for `.isa`, `.isaext`,
`.coredef`, and `.sysdef` files.

## Quick Start

1. Install dependencies:

   ```bash
   cd vscode
   npm install
   ```

2. Start the extension in VS Code:
   - Open the repository in VS Code.
   - Run `npm run watch` from the `vscode` folder in a terminal.
   - Press `F5` (or run the *Launch Extension* debug configuration) to open an Extension
     Development Host. Opening an ISA file will automatically start the Rust language server via
     `cargo run --features language-server --bin isa_language_server`.

## Configuration

The extension contributes two settings under `nanemu.isaLanguageServer`:

- `serverCommand`: Optional path to a prebuilt `isa_language_server` binary. Leave empty to run via
  `cargo`.
- `serverArgs`: Additional arguments supplied to the command (ignored when the command is blank).

These settings are useful when you want to point VS Code at a release build distributed with the
extension or produced by CI.

## Packaging

Use the root-level helper script to ship a self-contained VS Code package:

```bash
./scripts/package_language_server.sh
```

It performs the following steps:

1. Builds the Rust `isa_language_server` binary with `cargo build --release --features language-server`.
2. Copies the resulting executable into `vscode/server/` so the extension can launch it directly.
3. Runs `npm install`, `npm run compile`, and `npx vsce package` to generate a `.vsix`.
4. Installs the packaged extension into your local VS Code (if the `code` CLI is available).

When a bundled binary exists in `vscode/server/`, the extension prefers it over spawning `cargo`. If
the folder is empty, it falls back to the `cargo run` workflow described in Quick Start.
