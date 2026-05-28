#!/usr/bin/env bash
# ccb-setup — load all personas and install binary to ~/.local/bin
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CCB_DIR="$(dirname "$SCRIPT_DIR")"
PERSONAS_DIR="$CCB_DIR/personas"

echo "=== CCB Setup ==="

# Build with full features
echo "[1/4] Building ccb (full features)..."
cd "$CCB_DIR"
CC=/usr/bin/gcc cargo build --features "full" 2>&1 | tail -3

# Install binary
echo "[2/4] Installing binary to ~/.local/bin/ccb ..."
cp target/debug/ccb ~/.local/bin/ccb
chmod +x ~/.local/bin/ccb

# Load all personas
echo "[3/4] Loading personas..."
for f in "$PERSONAS_DIR"/*.json; do
  name=$(basename "$f" .json)
  echo "  - $name"
  ~/.local/bin/ccb expert build "$name" --dataset "$f" 2>&1
done

# Verify
echo "[4/4] Verifying..."
~/.local/bin/ccb expert list
echo ""
echo "=== Ready. Activate a persona: ccb expert activate <name> ==="