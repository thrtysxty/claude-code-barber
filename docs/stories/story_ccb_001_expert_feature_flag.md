# CCB Story 001: Expert Feature Flag + Deps

**Status:** READY
**Priority:** P0 — Layer 3 foundation
**Sprint:** CCB-1 (Expert/Persona Graph)

## Narrative
**As a** CCB developer,
**I want** an `expert` feature flag with appropriate dependencies wired in,
**So that** Layer 3 can be built without affecting the default trim/fade build.

## Context

CCB uses Cargo feature flags for optional capabilities. The `graph` flag pattern is the reference:
- Optional deps declared with `optional = true`
- Feature declared in `[features]` pulling in `dep:*` entries
- `full` feature aggregates all flags
- `cargo check --features expert` is the gate

Layer 3 needs SQLite (reuse `rusqlite` from graph, mark optional independently) and `serde_json` (already a dep — no change needed).

## Acceptance Criteria

### STEP ZERO
1. **Read `Cargo.toml`** — verify current `[features]` and `[dependencies]`. Document what's already present vs what needs adding.

### Feature Flag
2. **`expert = []` added** to `[features]` in `Cargo.toml` — initially empty (deps added in this story).
3. **`full` feature updated** — `"expert"` added to the `full` array.
4. **`rusqlite` marked shared** — if `graph` already pulls it in, `expert` should also list `dep:rusqlite` so it's available independently of the graph flag.

### Cargo Check Gate
5. **`cargo check --features expert` passes** — zero errors.
6. **`cargo check --features full` passes** — zero errors.
7. **`cargo check` (default, no flags) still passes** — no regressions.

## Files in Scope
- `Cargo.toml` only

## Frozen Surfaces
- All existing feature flags — do not remove or rename `trim`, `fade`, `sandbox`, `terse`, `graph`
- Existing dependency versions — do not bump

## Blocked By
- Story CCB-000 (graph feature) — `rusqlite` must be present before this story reuses it

## Blocks
- Story CCB-002

## Definition of Done
- [ ] `expert` in `[features]`
- [ ] `full` includes `expert`
- [ ] `cargo check`, `cargo check --features expert`, `cargo check --features full` — all clean
