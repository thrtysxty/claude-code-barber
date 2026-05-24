#!/usr/bin/env bash
# train-lora.sh — Train a LoRA adapter from a CCB expert dataset
#
# Usage:
#   train-lora.sh <training.jsonl> <output-dir> [--model <model>]
#
# Platforms:
#   - macOS (Apple Silicon) → mlx-lm
#   - Linux / 9020 GPU     → unsloth
#
# GPU requirements:
#   - Full fine-tune:  9020 (24 GB VRAM)
#   - LoRA fine-tune:  8 GB VRAM minimum
#
# Output:
#   <output-dir>/adapter.safetensors

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

usage() {
    cat <<EOF
Usage: train-lora.sh <training.jsonl> <output-dir> [--model <model>]

Train a LoRA adapter from a CCB expert instruction-tuning dataset.

Arguments:
  <training.jsonl>   Path to training data in Alpaca/ShareGPT JSONL format
  <output-dir>       Directory to write adapter.safetensors

Options:
  --model <name>     Base model name (default: qwopus3.5-9b-v3)

Environment:
  CCB_LORA_MODEL     Override default model
  MLX_LORA_DIR       mlx-lm output directory (macOS)
  UNSLOTH_LORA_DIR   unsloth output directory (Linux/9020)
EOF
    exit 1
}

if [[ $# -lt 2 ]]; then
    usage
fi

TRAINING_FILE="$1"
OUTPUT_DIR="$2"
shift 2

MODEL="${CCB_LORA_MODEL:-qwopus3.5-9b-v3}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --model)
            MODEL="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            usage
            ;;
    esac
done

if [[ ! -f "$TRAINING_FILE" ]]; then
    echo "Error: training file not found: $TRAINING_FILE"
    exit 1
fi

mkdir -p "$OUTPUT_DIR"

detect_platform() {
    case "$(uname -s)" in
        Darwin)
            echo "macos"
            ;;
        Linux)
            if command -v nvidia-smi &>/dev/null; then
                echo "linux-gpu"
            else
                echo "linux-cpu"
            fi
            ;;
        *)
            echo "unsupported"
            ;;
    esac
}

PLATFORM="$(detect_platform)"

echo "=== CCB LoRA Training Pipeline ==="
echo "Platform : $PLATFORM"
echo "Model    : $MODEL"
echo "Data     : $TRAINING_FILE"
echo "Output   : $OUTPUT_DIR/adapter.safetensors"
echo

case "$PLATFORM" in
    macos)
        echo "Detected macOS — using mlx-lm"
        if ! command -v mlx_lm &>/dev/null; then
            echo "Error: mlx-lm not installed"
            echo "  Install: pip install mlx-lm"
            echo "  Or:      brew install mlx-lm"
            exit 1
        fi
        MLX_DIR="${MLX_LORA_DIR:-$OUTPUT_DIR}"
        echo "Running: mlx_lm.lora --model $MODEL --train-data $TRAINING_FILE --output-dir $MLX_DIR"
        mlx_lm.lora \
            --model "$MODEL" \
            --train-data "$TRAINING_FILE" \
            --output-dir "$MLX_DIR" \
            --batch-size 1 \
            --iterations 500 \
            --steps-per-save 100 \
            --learning-rate 1e-4 \
            --lora_rank 16 \
            --lora_alpha 16 \
            --lora_dropout 0.1
        # mlx-lm uses adapter.safetensors in output dir
        if [[ -f "$MLX_DIR/adapter.safetensors" ]]; then
            cp "$MLX_DIR/adapter.safetensors" "$OUTPUT_DIR/adapter.safetensors"
            echo "Adapter written to $OUTPUT_DIR/adapter.safetensors"
        fi
        ;;
    linux-gpu)
        echo "Detected Linux with GPU — using unsloth"
        if ! command -v unsloth &>/dev/null; then
            echo "Error: unsloth not installed"
            echo "  Install: pip install unsloth"
            exit 1
        fi
        UNSLOTH_DIR="${UNSLOTH_LORA_DIR:-$OUTPUT_DIR}"
        echo "Running: unsloth train-lora --model $MODEL --data $TRAINING_FILE --output $UNSLOTH_DIR"
        unsloth train-lora \
            --model "$MODEL" \
            --data "$TRAINING_FILE" \
            --output "$UNSLOTH_DIR" \
            --lora_rank 16 \
            --lora_alpha 16 \
            --batch_size 4 \
            --learning_rate 1e-4 \
            --epochs 3 \
            --warmup_steps 10
        if [[ -f "$UNSLOTH_DIR/adapter.safetensors" ]]; then
            cp "$UNSLOTH_DIR/adapter.safetensors" "$OUTPUT_DIR/adapter.safetensors"
            echo "Adapter written to $OUTPUT_DIR/adapter.safetensors"
        fi
        ;;
    linux-cpu)
        echo "Error: CPU-only Linux not supported for LoRA training"
        echo "  LoRA training requires a GPU (9020 recommended, 8 GB minimum)"
        exit 1
        ;;
    *)
        echo "Error: unsupported platform: $(uname -s)"
        exit 1
        ;;
esac

echo
echo "=== Training complete ==="
echo "Adapter: $OUTPUT_DIR/adapter.safetensors"
echo
echo "To load with qwopus, set CCB_LORA_ADAPTER=$OUTPUT_DIR/adapter.safetensors"
echo "and restart llama-server with --lora-adapter <path>"