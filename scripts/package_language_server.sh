#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")"/.. && pwd)"
CARGO_TARGET="isa_language_server"
FEATURE_FLAG="language-server"
PLATFORM_EXT=""

if [[ "${OS:-}" == "Windows_NT" || "${MSYSTEM:-}" =~ MINGW|MSYS ]]; then
  PLATFORM_EXT=".exe"
fi

BIN_PATH="$ROOT_DIR/target/release/${CARGO_TARGET}${PLATFORM_EXT}"
SERVER_DIR="$ROOT_DIR/vscode/server"
SERVER_DEST="$SERVER_DIR/${CARGO_TARGET}${PLATFORM_EXT}"
VSIX_DIR="$ROOT_DIR/vscode"

printf '==> Building Rust language server (cargo build --release --features %s --bin %s)\n' "$FEATURE_FLAG" "$CARGO_TARGET"
cargo build --release --features "$FEATURE_FLAG" --bin "$CARGO_TARGET"

mkdir -p "$SERVER_DIR"
cp "$BIN_PATH" "$SERVER_DEST"
chmod +x "$SERVER_DEST"
printf 'Copied server binary to %s\n' "$SERVER_DEST"

pushd "$ROOT_DIR/vscode" >/dev/null
printf '==> Installing npm dependencies\n'
npm install
printf '==> Compiling TypeScript sources\n'
npm run compile
printf '==> Packaging VS Code extension via vsce\n'
npx vsce package
VSIX_FILE=$(ls -t *.vsix | head -n 1)
printf 'Produced VSIX: %s/%s\n' "$VSIX_DIR" "$VSIX_FILE"

if command -v code >/dev/null 2>&1; then
  printf '==> Installing VSIX into VS Code\n'
  code --install-extension "$VSIX_FILE" --force
else
  printf 'VS Code command-line tool not found; skipping installation.\n'
fi
popd >/dev/null

printf '\nAll done. Latest package: %s/%s\n' "$VSIX_DIR" "$VSIX_FILE"
