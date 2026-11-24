#!/usr/bin/env bash
set -euo pipefail
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "1/2 Running Rust unit tests & /// doctests..."
cargo test

echo "2/2 Building EdgeBridge v1.0.0-rc.1 standalone binary..."
cargo build --release

OS_NAME="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH_NAME="$(uname -m)"
BIN="$SCRIPT_DIR/target/release/edgebridge"
REL="$SCRIPT_DIR/releases/edgebridge-v1.0.0-rc.1-${OS_NAME}-${ARCH_NAME}"
mkdir -p "$SCRIPT_DIR/releases"
cp "$BIN" "$REL"
chmod +x "$REL"
echo "BUILD COMPLETE: $BIN -> $REL"
