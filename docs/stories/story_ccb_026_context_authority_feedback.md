# CCB Story 026: Context Authority — Weight Feedback, Gap Detection & Auto-Generation

**Status:** READY
**Priority:** P1 — makes the system self-tuning
**Sprint:** CCB-6 (Context Authority)
**Feature flag:** `context`
**Depends on:** CCB-025 (hook interception), CCB-019 (pattern mining), CCB-021 (LoCoMo)

## Narrative
**As a** Claude Code user,
**I want** context node weights to update automatically from session data, gaps in my workflow to be detected, and missing skills/experts to be suggested,
**So that** the system gets smarter over time — cheaper context, fewer blind spots, provably validated by LoCoMo.

## Context

CCB-024 creates the unified knowledge index with initial weights. CCB-025 wires it into hooks for per-turn retrieval. This story closes the loop: session traces feed back into weights, LoCoMo validates the changes, and gap detection identifies knowledge that's missing.

### The Weight Feedback Loop

```
Session N:
  1. Context authority injects nodes A, B, C (by weight)
  2. Tool calls succeed/fail → PostToolUse traces logged
  3. Trace records which nodes were injected AND what happened

Between sessions:
  4. ccb context tune — reads traces, updates weights
     - Node A was injected 50 times, correlated with success → weight ↑
     - Node D was never injected, never needed → weight stays low
     - Node E was never injected, but failures occurred where it would've been relevant → weight ↑ (gap signal)
  5. ccb gain --locomo — validates: did weight changes preserve knowledge?
     - Retention held → weights are good, system got cheaper
     - Retention dropped → which weights drifted? Roll back, re-test
```

### Gap Detection

Low weight means two different things:

```
Low weight + low importance = correctly tuned
  "worktree management" — weight 0.05 — rarely needed, correctly demoted

Low weight + high importance = blind spot
  "security audit" — weight 0.00 — ZERO usage across 50 sessions
  Expert sentinel exists, has domains loaded, but was NEVER activated
  That's not a tuning success. That's a gap.
```

Gap detection distinguishes these by cross-referencing:
- Node exists (expert/skill was built) → it was intended to be used
- Node weight is near zero across N sessions → it's never being used
- The code graph shows relevant symbols exist (auth code, SQL queries) → the domain is active in the codebase

If all three conditions are true, that's a gap — not irrelevance.

### Auto-Generation

When a gap is detected, CCB can help fill it:

```
Gap: "no security review in 30 sessions"
  → Suggest: ccb expert activate sentinel
  → Suggest: generate skill "security-review-checklist.md"
  → Suggest: wire PreToolUse hook for Edit calls touching auth/*
  → Track: new nodes enter the weight system, LoCoMo validates
```

CCB doesn't auto-wire without approval — it presents the suggestion. The user decides.

## Architecture

### Weight Update Formula

```
new_weight = α × current_weight + (1 - α) × session_signal

where:
  α = 0.7 (decay factor — recent sessions matter more)
  session_signal = f(injection_count, success_rate, relevance_hits)

  injection_count: how many times was this node injected?
  success_rate: when injected, did the subsequent tool call succeed?
  relevance_hits: did the user/model reference this node's content?
```

Weights are clamped to [0.01, 1.0] — nothing drops to absolute zero (recovery must be possible).

### Gap Detection Algorithm

```
fn detect_gaps(min_sessions: usize) -> Vec<GapReport>:
  1. Find all nodes where weight < 0.1 AND age > min_sessions sessions
  2. For each candidate:
     a. Is it an expert/skill that was deliberately built? (source_ref exists)
     b. Does the code graph contain symbols in the node's domain?
        (e.g. sentinel covers domain:security → are there auth symbols indexed?)
     c. Were there failures in recent sessions where this node's domain was relevant?
  3. If (a AND b) OR c → flag as gap
  4. Generate suggestion based on node kind:
     - expert → "activate this persona"
     - skill → "wire this skill into PreToolUse"
     - doc_section → "this rule is being ignored — promote weight"
     - domain → "no expert covers this active domain — build one"
```

### LoCoMo Validation Gate

```
fn validate_weight_change(changes: Vec<WeightChange>) -> ValidationResult:
  1. Snapshot current weights
  2. Apply proposed changes
  3. Run ccb gain --locomo
  4. Compare retention score vs. baseline
  5. If retention_delta > -2% → ACCEPT changes
  6. If retention_delta <= -2% → REJECT, roll back to snapshot
  7. Log result for future tuning
```

The -2% threshold is configurable. Conservative start — tighten as confidence builds.

## Acceptance Criteria

### Weight Feedback
- [ ] **AC1:** `ccb context tune` reads trace_events from traces.db, computes per-node session signals
- [ ] **AC2:** Weight update uses exponential moving average (α=0.7 default, configurable)
- [ ] **AC3:** Weights clamped to [0.01, 1.0] — no node drops to zero
- [ ] **AC4:** `ccb context tune --dry-run` shows proposed weight changes without applying them
- [ ] **AC5:** `ccb context tune` logs all weight changes to `~/.cache/ccb/weight_history.jsonl`
- [ ] **AC6:** Weight history includes: node_id, old_weight, new_weight, session_count, timestamp

### LoCoMo Validation
- [ ] **AC7:** `ccb context tune --validate` runs LoCoMo after applying weight changes
- [ ] **AC8:** If retention drops > threshold (default 2%), changes are rolled back automatically
- [ ] **AC9:** Validation result logged: accepted/rejected, retention_before, retention_after, delta
- [ ] **AC10:** `ccb context tune --validate --threshold <pct>` overrides the acceptance threshold

### Gap Detection
- [ ] **AC11:** `ccb context gaps` scans for nodes with weight < 0.1 that meet gap criteria
- [ ] **AC12:** Gap criteria: node was deliberately built (source_ref exists) AND domain has active code symbols
- [ ] **AC13:** `ccb context gaps` also detects domain gaps: code domains with zero expert/skill coverage
- [ ] **AC14:** Output includes: gap type, node info, evidence (why it's a gap, not just low-relevance), suggestion

### Suggestions
- [ ] **AC15:** Each gap includes a concrete suggestion: activate expert, generate skill, promote weight, or build new expert
- [ ] **AC16:** `ccb context gaps --apply <gap-id>` executes a suggestion (with user confirmation prompt)
- [ ] **AC17:** For skill generation: creates a stub skill .md in `~/.claude/skills/auto/` with the gap context
- [ ] **AC18:** For expert activation: runs `ccb expert activate <name>`

### Reporting
- [ ] **AC19:** `ccb context report` shows: weight distribution, top gainers/losers since last tune, active gaps, LoCoMo trend
- [ ] **AC20:** Report supports `--format human|json`
- [ ] **AC21:** Weight history visualizable: `ccb context report --weights <node-name>` shows weight over time

### Tests
- [ ] **AC22:** Unit tests: weight update math (EMA, clamping, decay)
- [ ] **AC23:** Unit tests: gap detection with mock traces (deliberate-but-unused vs. genuinely-irrelevant)
- [ ] **AC24:** Unit tests: LoCoMo validation gate (accept path, reject+rollback path)
- [ ] **AC25:** Integration test: tune cycle — inject → trace → tune → verify weights changed correctly

## Notes

- The EMA decay factor (α=0.7) means ~50% of the weight comes from the last 2 sessions. This is deliberately aggressive — the system should adapt fast. If it's too volatile, increase α.
- Gap detection requires CCB-015 (code graph edges) to determine whether a domain is "active in the codebase." Without edges, it can only check whether symbols exist, not whether they're called.
- Auto-generated skills go in `~/.claude/skills/auto/` to distinguish them from human-authored skills. The context authority indexes both.
- Weight history is append-only JSONL. It's the audit trail for the entire tuning system.
- The LoCoMo validation gate is optional (`--validate` flag). For quick iteration, skip it. For production weight updates, always validate.
- Consider: periodic scheduled tune runs? A cron job that runs `ccb context tune --validate` weekly could keep weights fresh without manual intervention.
- The gap detection for "domain with zero coverage" is the most valuable signal — it identifies areas of the codebase where no expert or skill exists at all. This is where CCB transitions from context manager to development partner.
