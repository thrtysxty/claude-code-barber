# CCB Story 014: YASR Statusline — CCB-owned renderer

**Status:** COMPLETE
**Priority:** P1 — core deliverable
**Sprint:** CCB-3 (Statusline)

## Narrative
**As a** CCB developer,
**I want** Claude Code's statusline to render from CCB's own data (token counts, rate limits, cost, git state, factory story state),
**So that** CCB owns the statusline display end-to-end, using yasr's renderer as an embedded library.

## Context

YASR ("Yet Another Statusline in Rust") was built specifically to be Claude Code's statusline renderer. It was previously a standalone binary with its own session file parsing. The goal of this story is to embed yasr as a library in CCB, so `ccb status` outputs the rendered statusline using CCB's own data sources.

**Data ownership:**
- Token counts → `~/.cache/ccb/route_usage.jsonl` (written by ccb-route per proxied request)
- Rate limits → `~/.cache/ccb/route_limits.json` (written by ccb-route per proxied request)
- Thinking multiplier → `CCB_THINKING` env var (from session hook)
- Context tokens → `CCB_CTX_TOKENS` / `CCB_CTX_MAX` env vars (from session hook)
- Model ID → `CCB_MODEL` env var
- Git state → `CCB_GIT_BRANCH` / `CCB_GIT_DIRTY` / `CCB_GIT_HASH` env vars (from session hook)
- Factory story → `factory.db` (via `factory::list_stories()`)

**Cost:** Anthropic published rates at actual model tier — regardless of which backend (minimax, ollama, direct) handled the request. So minimax calls count as if sent to Anthropic directly.

## Architecture

```
ccb status
  └─ StatusInput::load()
       ├─ ~/.cache/ccb/route_usage.jsonl  → token counts
       ├─ ~/.cache/ccb/route_limits.json → rate limits
       ├─ CCB_THINKING / CCB_CTX_TOKENS / CCB_MODEL env vars
       ├─ CCB_GIT_BRANCH / CCB_GIT_DIRTY / CCB_GIT_HASH env vars
       └─ factory.db → in-progress story
  └─ build_session_info(&StatusInput) → yasr::SessionInfo
  └─ yasr::render(&session_info, &theme, 120, "wide") → ANSI stdout
```

## yasr Library Integration

1. **yasr as embedded library** — split into `src/lib.rs` (library only) + `src/main_binary.rs` (standalone binary). `[[bin]]` uses `required-features = ["bin"]` so it doesn't compile when yasr is used as a CCB dep.
2. **CCB Cargo.toml** — `yasr = { path = "../yasr", optional = true }` behind `status` feature.
3. **No clap conflict** — yasr's clap only compiles when `bin` feature is active.

## ccbroute Token + Rate Limit Capture

For every proxied non-streaming request, ccb-route writes:
- **Token usage** → `~/.cache/ccb/route_usage.jsonl`:
  ```json
  {"t":"2026-05-26T10:00:00Z","mdl":"sonnet","in":1200,"out":340,"be":"ollama"}
  ```
- **Rate limits** → `~/.cache/ccb/route_limits.json` async after response:
  ```json
  {"five_hour":{"utilization":2.0,"resets_at":"..."},"seven_day":{"utilization":35.0}}
  ```

Backend-specific rate limit endpoints:
- **Anthropic** (direct): `GET https://api.anthropic.com/api/oauth/usage` — Bearer token from Keychain
- **Local Ollama**: `GET http://localhost:11434/api/usage` → `{remaining, limit, resets_at}`
- **Ollama cloud**: `GET https://ollama.com/api/usage` → same schema
- **Minimax**: `GET https://www.minimax.io/v1/token_plan/remains` → `{remaining, limit}`

## Anthropic Rate Table

```rust
// Haiku:  $0.80/$4 i/o,  no extended thinking tier
// Sonnnet: $3/$15 i/o,   thinking 3.5x
// Opus:   $15/$75 i/o,  thinking 3.5x
```

Cost = `(input_tokens / 1M) * rate.input_per_million + (output_tokens / 1M) * rate.output_per_million * thinking_multiplier`

## Acceptance Criteria

### STEP ZERO
1. **Read `src/features/status.rs`** — understand `StatusInput`, `load()`, `build_session_info()`
2. **Read `src/bin/ccb-route.rs`** — understand `write_usage_line()` and async rate limit fetch
3. **Read `yasr/src/lib.rs`** — understand exported `render()`, `SessionInfo`, `Theme`
4. **Read `settings.json`** — find current `statusLine.command` setting

### yasr Library Wiring
5. `cargo build --features "full"` — zero errors (yasr compiles as library, bin doesn't activate)
6. `ccb status` — prints yasr-rendered statusline to stdout (wide layout)
7. Session ID, branch, dirty indicator, commit hash, time, token count, rate limit bars, cost all visible

### Token + Rate Limit Capture
8. `ccb-route` (running) processes a request → `~/.cache/ccb/route_usage.jsonl` gets one new line
9. `ccb-route` (running) processes a request → `~/.cache/ccb/route_limits.json` updated
10. `ccb status` reads and displays these values correctly

### Cost Display
11. Token cost shown = Anthropic rate for actual model tier (not backend's pricing)
12. With `CCB_THINKING=true`, output tokens × thinking multiplier applied

### Factory Story State
13. With a story in `coding` state and factory feature active, story indicator shows in statusline

### Claude Code Statusline Integration
14. **`settings.json` `statusLine.command`** updated to `~/.local/bin/ccb status`
15. New Claude Code session → statusline shows CCB's yasr output

## Files Modified

| File | Change |
|------|--------|
| `yasr/src/lib.rs` | NEW — library-only export (render, SessionInfo, Theme) |
| `yasr/src/main_binary.rs` | NEW — standalone binary (activated by `bin` feature) |
| `yasr/Cargo.toml` | Added `[lib]`, `bin` feature, `[[bin]]` with `required-features` |
| `ccb/Cargo.toml` | Added `yasr` path dep behind `status` feature |
| `src/features/rates/mod.rs` | NEW — `src/features/rates/` (was analytics, renamed to avoid conflict) |
| `src/features/rates/model_rates.rs` | NEW — Anthropic rate table + cost computation |
| `src/features/status.rs` | NEW — `StatusInput`, `load()`, `build_session_info()`, `resolve_theme()` |
| `src/bin/ccb-route.rs` | Added `write_usage_line()`, async rate limit fetch per backend |
| `src/cli.rs` | Added `Status` command variant |
| `src/main.rs` | Added `status_cmd()` handler |
| `~/.claude/settings.json` | `statusLine.command` → `~/.local/bin/ccb status` |

## Frozen Surfaces
- `yasr::render()` signature — do not change
- `StatusInput::load()` env var names — must match session hook injection
- `route_usage.jsonl` JSON schema — `{t, mdl, in, out, be}` fields

## Blocked By
- None (this story enables factory story state in statusline, but factory exists independently)

## Blocks
- Story CCB-015 (statusline theming / layout options)

## Definition of Done
- [x] `cargo build --features "full"` clean
- [x] `ccb status` renders yasr statusline to stdout
- [x] `~/.cache/ccb/route_usage.jsonl` schema valid (t/mdl/in/out/be)
- [x] `~/.cache/ccb/route_limits.json` schema valid (five_hour/seven_day/utilization)
- [x] Cost shown at Anthropic rates for model tier
- [x] `settings.json` `statusLine.command` points at `ccb status`
- [x] Integration tests written and passing for status command
- [x] Rate model compute_cost bug fixed (thinking multiplier only applied when thinking=true)