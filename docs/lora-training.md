# LoRA Training Pipeline

This document describes the end-to-end pipeline for training a LoRA adapter from the CCB expert knowledge graph and loading it with qwopus on aibox.

## Overview

Phase 1 (CCB-002–004) surfaces expert knowledge via PreToolUse hooks — structured JSON injected into context. This works but has token cost and latency.

Phase 2 adapts the base model directly: the expert graph is converted to instruction-tuning pairs, a LoRA adapter is trained on a GPU (9020 recommended), and the adapter is loaded alongside qwopus3.5-9b-v3 at inference time.

**Result:** domain expertise is baked into weights. The hook still runs for dynamic state (active persona, session context), but static pattern knowledge costs zero tokens.

## Architecture

```
expert graph (SQLite)
    ↓ ccb expert export --format alpaca
training pairs (JSONL)
    ↓ scripts/train-lora.sh (mlx-lm or unsloth on 9020)
LoRA adapter (.safetensors)
    ↓ llama-server --lora-adapter <path>
qwopus + adapter (aibox:8080)
```

## Prerequisites

| Component | Requirement |
|-----------|-------------|
| GPU VRAM | 24 GB for full fine-tune; 8 GB for LoRA |
| Platform | macOS (Apple Silicon + mlx-lm) or Linux (CUDA + unsloth) |
| Tools | `mlx-lm` (macOS) or `unsloth` (Linux) |
| Model | qwopus3.5-9b-v3 on aibox |
| CCB | Built with `--features full` or `--features expert` |

## Step 1 — Export Training Data

From an active expert persona, export instruction-tuning pairs:

```bash
ccb expert export sentinel \
  --format alpaca \
  --output /tmp/sentinel-train.jsonl
```

**Alpaca format example:**
```json
{"instruction": "What mitigations apply to CWE-22 Path Traversal?", "input": "", "output": "1. Resolve path then verify it starts with expected root\n2. Reject paths containing '..' before resolution\n3. Use allowlist of permitted directories"}
```

**ShareGPT format:**
```bash
ccb expert export sentinel \
  --format sharegpt \
  --output /tmp/sentinel-train.jsonl
```

The export produces one training pair per pattern, plus cross-domain synthesis pairs for domains with multiple patterns.

**Output:** `Exported N training pairs to /tmp/sentinel-train.jsonl`

## Step 2 — Train the Adapter

Run the training script:

```bash
scripts/train-lora.sh /tmp/sentinel-train.jsonl \
  /tmp/lora-output \
  --model qwopus3.5-9b-v3
```

The script auto-detects your platform:
- **macOS** → uses `mlx_lm.lora` (Apple Silicon, no GPU needed)
- **Linux + GPU** → uses `unsloth train-lora`
- **Linux CPU** → errors (LoRA requires GPU)

GPU requirements:
- Full fine-tune: 9020 or equivalent (24 GB VRAM)
- LoRA fine-tune: 8 GB VRAM minimum

**Output:** `adapter.safetensors` in the specified output directory.

## Step 3 — Load the Adapter

### With llama-server (aibox)

Pass the adapter at server startup via `--lora-adapter`:

```bash
llama-server \
  --model /models/qwopus3.5-9b-v3 \
  --lora-adapter /path/to/adapter.safetensors \
  --port 8080
```

### With CCB route

Set the `CCB_LORA_ADAPTER` environment variable:

```bash
export CCB_LORA_ADAPTER=/path/to/adapter.safetensors
ccb route start
```

When `CCB_LORA_ADAPTER` is set, `ccb route` startup shows:

```
adapter: /path/to/adapter.safetensors
```

The CCB route passes the adapter path to aibox via `X-LoRA-Adapter` header (future: llama-server will support per-request adapter selection via header).

## Step 4 — Verify

Query the active persona to confirm the adapter is active:

```bash
ccb expert query --format json
```

Expected output (excerpt):
```json
{
  "persona": "sentinel",
  "active_domains": ["path_traversal", "sql_injection"],
  "patterns": [...]
}
```

For hook-based verification, activate the persona first:

```bash
ccb expert activate sentinel
ccb expert query --format json
```

Then trigger a tool call that matches a pattern — the hook should inject the persona context, but the model already has domain knowledge baked in.

## GPU Memory Guide

| Task | GPU | VRAM |
|------|-----|------|
| Full fine-tune | 9020 | 24 GB |
| LoRA fine-tune | T4 / RTX 3060 | 8 GB |
| QLoRA (4-bit) | RTX 3060 | 6 GB |
| mlx-lm (Apple Silicon) | M1 Max / M2 Ultra | unified |

## Files

| File | Purpose |
|------|---------|
| `src/cli.rs` | `Export` command definition |
| `src/main.rs` | Export dispatch |
| `src/features/expert.rs` | `export()` implementation |
| `scripts/train-lora.sh` | Platform-aware training script |
| `docs/lora-training.md` | This document |

## Troubleshooting

**"persona not found"** — Run `ccb expert activate <name>` before exporting.

**"no patterns to export"** — The persona has no patterns. Ingest a dataset first:
```bash
ccb expert ingest --dataset /path/to/sentinel.yaml
```

**macOS: mlx-lm not found** — Install: `pip install mlx-lm`

**Linux: unsloth not found** — Install: `pip install unsloth`

**llama-server: --lora-adapter not recognized** — Update llama-server to a version that supports LoRA adapters.