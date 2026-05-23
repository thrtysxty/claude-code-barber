# CCB Story 011: LoRA Training Pipeline (Phase 2)

**Status:** READY
**Priority:** P2 — Phase 2, requires 9020 GPU
**Sprint:** CCB-3 (Model Adaptation)

## Narrative
**As a** CCB operator running qwopus on aibox,
**I want** a LoRA adapter trained from the expert graph's domain knowledge,
**So that** qwopus applies security and domain patterns natively — zero token cost, no hook overhead.

## Context

Phase 1 (CCB-002–004) surfaces expert knowledge via PreToolUse hooks — structured JSON injected into context. This works but has token cost and latency. Phase 2 adapts the base model directly: the expert graph is converted to instruction-tuning pairs, a LoRA adapter is trained on the 9020 GPU (aibox), and the adapter is loaded alongside qwopus3.5-9b-v3 at inference time.

Result: domain expertise is baked into weights. Hook still runs for dynamic state (active persona, session context), but static pattern knowledge costs zero tokens.

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

## Acceptance Criteria

### Export Command
1. `ExpertCmd::Export` added to `src/cli.rs`:
```rust
/// Export expert graph as instruction-tuning pairs for LoRA training
Export {
    persona: String,
    #[arg(long, default_value = "alpaca")]
    format: ExportFormat,   // alpaca | sharegpt
    #[arg(long)]
    output: std::path::PathBuf,
},
```
2. `ccb expert export sentinel --format alpaca --output /tmp/sentinel-train.jsonl` generates JSONL in Alpaca format:
```json
{"instruction": "What mitigations apply to CWE-22 Path Traversal?", "input": "", "output": "1. Resolve path then verify it starts with expected root\n2. Reject paths containing '..' before resolution\n3. Use allowlist of permitted directories"}
```
3. Generates at minimum one training pair per pattern, plus cross-domain synthesis pairs.
4. Output count printed: `Exported 42 training pairs to /tmp/sentinel-train.jsonl`

### Training Script
5. `scripts/train-lora.sh` created:
```bash
#!/usr/bin/env bash
# Train a LoRA adapter from a CCB expert dataset
# Usage: train-lora.sh <training.jsonl> <output-dir> [--model qwopus3.5-9b-v3]
# Requires: mlx-lm (macOS) or unsloth (Linux/9020)
```
6. Script detects platform (macOS → mlx-lm, Linux → unsloth) and runs appropriate trainer.
7. Produces adapter at `<output-dir>/adapter.safetensors`.
8. Documents minimum GPU requirements: 9020 (24GB VRAM) for full fine-tune, 8GB for LoRA.

### Adapter Loading
9. `ccb route` startup message shows active adapter if `CCB_LORA_ADAPTER` env var is set:
```
  adapter: /path/to/adapter.safetensors
```
10. `ccb-route` passes adapter path to aibox via `X-LoRA-Adapter` header (future: llama-server supports this via `--lora-adapter` at startup, not per-request).

### Documentation
11. `docs/lora-training.md` written — end-to-end walkthrough: export → train → load → verify.

## Files in Scope
- `src/cli.rs` — add `Export` to `ExpertCmd`
- `src/main.rs` — add export dispatch
- `src/features/expert.rs` — implement `export()`
- `scripts/train-lora.sh` (new)
- `docs/lora-training.md` (new)

## Blocked By
- CCB-009 (needs populated expert graph to export)
- CCB-010 (sentinel dataset as reference training corpus)

## Blocks
- None

## Definition of Done
- [ ] `ccb expert export sentinel --format alpaca --output /tmp/test.jsonl` runs and produces valid JSONL
- [ ] `scripts/train-lora.sh` exists and is executable
- [ ] `docs/lora-training.md` covers the full pipeline
- [ ] `cargo build --features expert` clean
