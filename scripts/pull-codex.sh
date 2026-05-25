#!/usr/bin/env bash
# Pull CodeX-2M-Thinking dataset to external drive and run CCB extraction.
# Run when external drive is mounted: bash scripts/pull-codex.sh [/Volumes/DriveName]

set -euo pipefail

DRIVE="${1:-}"

if [[ -z "$DRIVE" ]]; then
    DRIVE=$(ls /Volumes/ 2>/dev/null | grep -v "Macintosh HD\|Recovery\|VM" | head -1)
    [[ -n "$DRIVE" ]] || { echo "ERROR: No external drive found. Pass path as argument."; exit 1; }
    DRIVE="/Volumes/$DRIVE"
fi

DEST="$DRIVE/datasets/codex-2m-thinking"
echo "Drive: $DRIVE ($(df -h "$DRIVE" | awk 'NR==2{print $4}') free)"
echo "Destination: $DEST"

read -rp "Proceed with download (~24.4GB)? [y/N] " confirm
[[ "${confirm,,}" == "y" ]] || { echo "Aborted."; exit 0; }

mkdir -p "$DEST"
huggingface-cli download Modotte/CodeX-2M-Thinking \
    --repo-type dataset \
    --local-dir "$DEST" \
    --local-dir-use-symlinks False

echo "Download complete. Running CCB extraction..."
python3 "$(dirname "$0")/extract-codex-to-ccb.py" "$DEST" "$DRIVE/datasets/ccb-experts"

echo ""
echo "Ingest with:"
echo "  ccb expert ingest --dataset $DRIVE/datasets/ccb-experts/coder.yaml"
echo "  ccb expert ingest --dataset $DRIVE/datasets/ccb-experts/architect.yaml"
echo "  ccb expert ingest --dataset $DRIVE/datasets/ccb-experts/debugger.yaml"
