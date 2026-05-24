# CCB Story 013: Telemetry + A/B Test Infrastructure

**Status:** READY
**Priority:** P1 — measurement before optimization
**Sprint:** CCB-2 (Measurement)

## Narrative
**As a** CCB developer,
**I want** to measure actual token impact of CCB processing, bypass mode, and expert injection side by side,
**So that** we have real data on what trim/fade/knowledge graphs actually save — not just estimates.

## Context

`CompressionEvent` currently logs `tokens_in` and `tokens_out` as byte-length estimates (`len / 4`).
There is no way to compare a CCB session against a raw (bypass) session, and no tracking of whether
the expert persona was active. This story adds:

1. **`mode` field** — distinguishes CCB-processed sessions from bypass sessions
2. **`persona` + `domains_hit` fields** — tracks expert graph injection per event
3. **`CCB_BYPASS=1` env var** — skips trim/fade processing but still logs with `mode: bypass`
4. **`ccb gain --ab`** — side-by-side comparison table of CCB vs bypass events
5. **`ccb gain --expert`** — filter to events where expert was injected; show delta vs non-expert

Phase 2 (depends on CCB-008 route proxy): capture actual `usage.prompt_tokens` from
llama-server response body and store as `server_tokens_in` / `server_tokens_out`.

## Acceptance Criteria

### STEP ZERO
1. **Read `src/log.rs`** — verify `CompressionEvent` struct fields and `record()` method.
2. **Read `src/analytics.rs`** — verify `gain()` reads `CompressionEvent` from JSONL.
3. **Read `src/features/trim.rs`** — find where `CompressionEvent::record()` is called to understand injection point.

### Log Schema Extension
4. **`CompressionEvent` gains three new optional fields** (all `#[serde(skip_serializing_if = "Option::is_none")]`):
   ```rust
   pub mode: Option<String>,        // "ccb" | "bypass" — None → "ccb" (backward compat)
   pub persona: Option<String>,     // active expert persona name, if any
   pub domains_hit: Option<Vec<String>>,  // expert domains queried
   ```
5. **Existing log entries deserialize without error** — `Option` fields default to `None` on missing keys.
6. **`gain()` backward-compatible** — old log entries (no `mode` field) count as mode "ccb".

### Bypass Mode
7. **`CCB_BYPASS=1` env var** skips all trim/fade processing — content passes through unchanged.
8. **Bypass still logs a `CompressionEvent`** with `mode: "bypass"`, `tokens_in == tokens_out` (no reduction).
9. **`CCB_BYPASS=1 ccb trim <anything>`** exits 0, outputs content unchanged, event logged.

### Expert Injection Logging
10. **When `ccb expert` is active** (persona set in `active_persona` table) and trim/fade runs,
    the logged event includes `persona: Some("sentinel")` and `domains_hit: Some([...])`.
11. **When no persona is active**, `persona` and `domains_hit` are `None` — no change to existing behavior.
12. Implementation: trim/fade calls `features::expert::active_context() -> Option<(String, Vec<String>)>`
    to get `(persona_name, domains)`. This function opens `expert.db`, reads `active_persona` join.
    Returns `None` if `expert` feature not compiled in (via cfg).

### `ccb gain --ab` Report
13. **`ccb gain --ab`** flag added to `Gain` CLI command.
14. **Output table** shows CCB-mode events vs bypass-mode events side by side:
    ```
    ╭──────────────────────────────────────────────────────────╮
    │                 CCB — A/B Comparison                      │
    ├────────────┬──────────────┬──────────────┬───────────────┤
    │ mode       │ avg tokens↓  │ avg tokens↑  │ avg saved     │
    ├────────────┼──────────────┼──────────────┼───────────────┤
    │ ccb        │        8,420 │        2,103 │  6,317  75%   │
    │ bypass     │        8,390 │        8,390 │      0   0%   │
    ╰────────────┴──────────────┴──────────────┴───────────────╯
      ccb: 142 events   bypass: 18 events
    ```
15. If fewer than 2 bypass events exist, prints: `Not enough bypass sessions — run with CCB_BYPASS=1 to generate baseline.`

### `ccb gain --expert` Report
16. **`ccb gain --expert`** flag filters events where `persona` is Some vs None:
    ```
    ╭───────────────────────────────────────────────────────────╮
    │             CCB — Expert Injection Delta                   │
    ├──────────────────┬────────────┬────────────┬─────────────┤
    │ condition        │ avg tok↓   │ avg tok↑   │ avg saved   │
    ├──────────────────┼────────────┼────────────┼─────────────┤
    │ expert active    │      9,210 │      1,870 │  7,340  80% │
    │ no expert        │      7,940 │      2,240 │  5,700  72% │
    ╰──────────────────┴────────────┴────────────┴─────────────╯
      expert: 34 events   no expert: 108 events
    ```
17. Top 3 most-hit domains listed below the table.

### Gate
18. `cargo build --features expert` — zero errors.
19. `CCB_BYPASS=1 echo "test content" | target/debug/ccb trim` — exits 0, event logged with `mode: "bypass"`.
20. `target/debug/ccb gain --ab` — exits 0 (may show "not enough data" message).
21. `target/debug/ccb gain --expert` — exits 0.

## Files in Scope
- `src/log.rs` — extend `CompressionEvent`
- `src/analytics.rs` — add `--ab` and `--expert` modes to `gain()`
- `src/cli.rs` — add `--ab` and `--expert` flags to `GainArgs`
- `src/features/trim.rs` — read bypass env + inject expert context into log event
- `src/features/fade.rs` — same bypass + expert context
- `src/features/expert.rs` — add `active_context() -> Option<(String, Vec<String>)>` fn

## Phase 2 (out of scope here — belongs in CCB-008)
- Capture `usage.prompt_tokens` / `usage.completion_tokens` from llama-server HTTP response
- Store as `server_tokens_in: Option<usize>` and `server_tokens_out: Option<usize>` in log
- `gain --real` flag: use server-reported counts instead of estimates where available

## Frozen Surfaces
- `CompressionEvent::record()` call sites — do not change signature
- `gain()` output when called without flags — existing table unchanged

## Blocked By
- Story CCB-002 (expert.rs must exist for `active_context()`)

## Blocks
- Story CCB-008 (Phase 2 server-token capture extends this schema)

## Definition of Done
- [ ] `CompressionEvent` has `mode`, `persona`, `domains_hit` (all optional, backward-compat)
- [ ] `CCB_BYPASS=1` skips processing, logs with `mode: "bypass"`
- [ ] Expert active context logged when persona set
- [ ] `ccb gain --ab` outputs comparison table
- [ ] `ccb gain --expert` outputs expert delta table
- [ ] `cargo build --features expert` clean
- [ ] Bypass gate passes
