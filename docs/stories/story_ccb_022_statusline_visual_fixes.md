# CCB Story 022: Statusline — Visual Bug Fixes & Feature Re-wire

**Status:** SHIPPED — PR #15 merged
**Priority:** P1 — code is written, just needs finishing
**PR:** https://github.com/thrtysxty/claude-code-barber/pull/8
**Sprint:** CCB-4 (can run parallel with graph work)
**Feature flag:** `status`
**Depends on:** None

## Narrative
**As a** CCB user,
**I want** `ccb status` to render a correct, readable statusline in my terminal,
**So that** I can see token usage, cost, model, git state, and session info at a glance without visual artifacts.

## Context

YASR (Yet Another Statusline in Rust) has been fully ported into CCB as `src/features/status/` — 6,511 lines across 9 files:

```
border.rs      327 lines — border rendering with gradient + connectors
ccb_bridge.rs  473 lines — CCB data → statusline data bridge
demo.rs        325 lines — demo/preview mode
gradient.rs    689 lines — multi-stop color interpolation engine
mod.rs          23 lines — module root
mon.rs         818 lines — monitoring / live refresh
renderer.rs  1,580 lines — main statusline renderer
session.rs   1,643 lines — session JSON parsing + token accounting
themes.rs      633 lines — theme definitions + model color mapping
```

The code was **removed from the build** during PR #7 (gateway discovery) because:
1. Rustfmt couldn't resolve the module (files were untracked, module was declared)
2. The `status` feature flag and its deps were stripped from Cargo.toml
3. Visual bugs remained: non-rendering Unicode glyphs and column alignment issues

The original Python reference lives at `~/Projects/yasr` and can be deleted once this ships.

**Reference:** `STATUSLINE_PLAN.md` documents the full parity spec from the Python original.

## Acceptance Criteria

### Visual Bug Fixes
- [ ] **AC1:** Identify all Unicode glyphs used in renderer.rs/border.rs that fail to render in common terminals
- [ ] **AC2:** Replace non-rendering glyphs with safe alternatives (test against: Terminal.app, iTerm2, VS Code integrated terminal)
- [ ] **AC3:** Fix column alignment: cell widths must account for multi-byte Unicode characters and ANSI escape sequence lengths
- [ ] **AC4:** Sparkline rendering: verify two-row half-block sparklines display correctly at various terminal widths (80, 120, 200 cols)
- [ ] **AC5:** Gradient borders: verify left/right border gradient renders without visual seams at row boundaries
- [ ] **AC6:** Pill overlays: verify pill text (model name, persona) doesn't overflow or clip

### Feature Re-wire
- [x] **AC7:** Add `status` feature flag to Cargo.toml with deps: `notify`, `shellexpand`, `libc` (all optional) — PR #8
- [x] **AC8:** Add `pub mod status` to `lib.rs` and `main.rs` behind `#[cfg(feature = "status")]` — PR #8 (main.rs; no lib.rs needed)
- [ ] **AC9:** Add `Status` variant to `Command` enum in `cli.rs` with subcommands: `show`, `demo`, `mon`
- [x] **AC10:** Add `status_cmd()` to `main.rs` dispatching to `features::status` — PR #8
- [ ] **AC11:** Add `status` to the `full` feature set in Cargo.toml
- [ ] **AC12:** `ccb status show` renders the statusline once to stdout
- [ ] **AC13:** `ccb status demo` renders with mock data (no live session needed)
- [ ] **AC14:** `ccb status mon` enters live refresh mode (re-renders on interval)

### Commit & Cleanup
- [x] **AC15:** All files in `src/features/status/` committed and tracked — PR #8
- [x] **AC16:** `cargo build --features status` compiles without warnings — PR #8 (builds clean with `full,status`)
- [ ] **AC17:** `cargo clippy --features status` passes with `-D warnings`
- [ ] **AC18:** CI passes with `full` features (which now includes `status`)
- [x] **AC19:** Remove `// status feature (WIP)` comment from main.rs line 273-274 — PR #8 (replaced with actual code)

## Terminal Test Matrix

| Terminal | Min Version | Test |
|----------|-------------|------|
| Terminal.app | macOS 15 | `ccb status demo` renders without artifacts |
| iTerm2 | 3.5+ | Same |
| VS Code integrated | 1.90+ | Same |

### Token Layout Restructuring (added during PR #8)
- [x] **AC20:** Restructure token display from timeframe-grouped (session row / daily row) to direction-grouped (inputs row / outputs row)
- [x] **AC21:** Session tokens on the left, daily tokens on the right within each row
- [x] **AC22:** Cost columns swapped: daily cost/d on inputs row, session cost on outputs row
- [x] **AC23:** Bottom sparkline no longer gated on daily data — always visible on outputs row

## Notes

- The Python original at `~/Projects/yasr` should be deleted AFTER this story ships — not before
- `STATUSLINE_PLAN.md` in repo root has the full parity spec — use it as the visual reference
- The `ccb_bridge.rs` file reads from `~/.cache/ccb/route_usage.jsonl` and env vars set by hooks — these paths are already established by the router
- Don't over-engineer glyph fallback — pick one safe set that works everywhere, not a runtime detection system
