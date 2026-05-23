# CCB Story 007: Rust Integration Tests — expert graph

**Status:** READY
**Priority:** P1 — test coverage
**Sprint:** CCB-2 (Rust Test Suite)

## Narrative
**As a** CCB developer,
**I want** `tests/expert_integration.rs`,
**So that** the Layer 3 expert/persona graph has regression coverage.

## Context

Tests invoke the real binary. A minimal fixture dataset JSON is written to a temp file for `ccb expert build` tests. JSON output is parsed with `serde_json`.

## Acceptance Criteria

### STEP ZERO
1. **Read `src/features/expert.rs`** — verify `build`, `activate`, `deactivate`, `list`, `query_active` signatures and the dataset JSON format.

### Fixture
2. **`fixture_dataset()` helper** — writes this JSON to a temp file:
```json
{
  "persona": "test_sentinel",
  "description": "Test security persona",
  "domains": [
    {
      "name": "test_injection",
      "category": "security",
      "patterns": [
        {"id": "TEST-01", "name": "Test Pattern", "mitigations": ["validate", "escape"]}
      ]
    }
  ]
}
```

### Test Cases
3. **`test_expert_build`** — `ccb expert build test_sentinel --dataset <fixture>`, assert exit 0.
4. **`test_expert_list`** — after build, `ccb expert list`, assert exit 0, stdout contains "test_sentinel".
5. **`test_expert_activate`** — `ccb expert activate test_sentinel`, assert exit 0.
6. **`test_expert_activate_unknown`** — `ccb expert activate nonexistent_persona`, assert exit non-zero, stderr contains error message.
7. **`test_expert_query_empty`** — with no active persona, `ccb expert query`, assert exit 0, stdout is `{}`.
8. **`test_expert_query_active_json`** — after activating test_sentinel, `ccb expert query --format json`, assert valid JSON, assert `persona` == "test_sentinel", assert `patterns` array non-empty.
9. **`test_expert_deactivate`** — after activating, `ccb expert deactivate`, then `ccb expert query`, assert stdout is `{}`.
10. **`test_expert_query_always_valid_json`** — run query in all states (no persona, active persona, after deactivate), assert JSON valid every time.

### Gate
11. **`cargo test --features expert`** — 0 FAILED.

## Files in Scope
- `tests/expert_integration.rs` (new)

## Frozen Surfaces
- `src/features/expert.rs` — do not modify implementation to fit tests

## Blocked By
- Story CCB-004

## Blocks
- None

## Definition of Done
- [ ] `tests/expert_integration.rs` — all 8 tests pass
- [ ] `cargo test --features expert` clean
- [ ] `cargo test --features full` clean
