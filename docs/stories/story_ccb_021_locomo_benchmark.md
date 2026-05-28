# CCB Story 021: LoCoMo Benchmark — Compression Quality Measurement

**Status:** READY
**Priority:** P1 — validates all other CCB features
**Sprint:** CCB-5 (Memory)
**Feature flag:** `bench` (or default, no gate)
**Depends on:** None (can run independently, but most valuable after CCB-018/019/020)

## Narrative
**As a** CCB developer,
**I want** to measure whether CCB's compression preserves the information that matters for long conversations,
**So that** I can prove compression quality, not just compression quantity.

## Context

The LoCoMo paper (arxiv.org/abs/2402.17753, "Evaluating Very Long-Term Conversational Memory of LLM Agents") defines a benchmark for testing memory retention across 300-turn, 35-session, ~9000-token conversations. It evaluates three task types:

1. **Question Answering** — can the agent recall facts from earlier in the conversation?
2. **Event Summarization** — can the agent summarize temporal event sequences?
3. **Multimodal Dialogue** — can the agent maintain coherence across modalities?

CCB's `gain` command currently measures tokens saved (quantity). LoCoMo measures information retained (quality). Together they prove: "we saved X tokens AND preserved Y% of retrievable information."

The LoCoMo dataset is CC-BY-4.0 licensed. We use a subset for automated benchmarking.

## Architecture

```
ccb gain --locomo [--dataset path] [--compression-level trim|cut|buzz]

Pipeline:
  1. Load LoCoMo conversation dataset (JSON)
  2. For each conversation:
     a. Run through CCB compression (trim/cut/buzz)
     b. Present QA tasks against compressed output
     c. Score: exact match + fuzzy match on expected answers
  3. Report:
     - Tokens saved (quantity)
     - QA accuracy (quality)
     - Per-session retention curve
     - Compression level vs. accuracy tradeoff

Output:
  ╭─────────────────────────────────────────────────────────╮
  │              CCB — LoCoMo Quality Benchmark             │
  ├──────────────┬──────────┬──────────┬───────────────────┤
  │ compression  │ tokens↓  │ saved %  │ QA accuracy       │
  ├──────────────┼──────────┼──────────┼───────────────────┤
  │ none         │   9,000  │    0%    │  baseline         │
  │ trim         │   6,200  │   31%    │  94.2%            │
  │ cut          │   4,100  │   54%    │  87.1%            │
  │ buzz         │   2,800  │   69%    │  72.3%            │
  ╰──────────────┴──────────┴──────────┴───────────────────╯
```

## Acceptance Criteria

- [ ] **AC1:** LoCoMo dataset adapter: parse the published JSON format into CCB-internal conversation structs
- [ ] **AC2:** Conversation struct: `Vec<Session>` where each Session has `Vec<Turn>` with role/content/timestamp
- [ ] **AC3:** Compression pipeline: apply specified CCB feature (trim, cut, or buzz) to each conversation's content
- [ ] **AC4:** QA evaluation: for each conversation, extract QA pairs from the dataset's annotations
- [ ] **AC5:** Scoring: exact match (case-insensitive) + fuzzy match (Levenshtein distance ≤ 2) against expected answers
- [ ] **AC6:** `ccb gain --locomo` runs the full pipeline and prints the comparison table
- [ ] **AC7:** `--dataset path` allows custom dataset path (default: bundled subset)
- [ ] **AC8:** `--compression-level` accepts `trim`, `cut`, `buzz`, or `all` (default: `all` — runs all three)
- [ ] **AC9:** `--format human|json` output modes
- [ ] **AC10:** Bundled dataset: include a 10-conversation subset (≤500KB) in `testdata/locomo/` for CI
- [ ] **AC11:** Per-session retention curve: report accuracy at session boundaries (session 5, 10, 15, 20, 25, 30, 35)
- [ ] **AC12:** Baseline comparison: always run uncompressed as baseline alongside compressed
- [ ] **AC13:** Report includes total runtime and tokens processed
- [ ] **AC14:** Unit tests: scoring logic (exact match, fuzzy match, edge cases)
- [ ] **AC15:** CI integration: `ccb gain --locomo --dataset testdata/locomo/ --format json` runs in benchmark CI job

## Notes

- The full LoCoMo dataset has 300-turn conversations — the bundled subset should be representative but small enough for CI (<30 seconds)
- QA accuracy is self-contained scoring (string comparison) — no LLM judge needed for Phase 1
- Phase 2 could add LLM-as-judge for summarization quality (requires a model endpoint)
- This benchmark should eventually gate PRs: if compression quality drops below threshold, block merge
- The retention curve is the most valuable output — it shows WHERE in the conversation CCB loses information
