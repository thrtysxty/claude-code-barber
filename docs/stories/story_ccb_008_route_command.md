# CCB Story 008: `ccb route` — Model Router as Managed Service

**Status:** READY
**Priority:** P0 — agent dispatch infrastructure
**Sprint:** CCB-1

## Narrative
**As a** CCB user,
**I want** `ccb route start/stop/status/env` commands,
**So that** the model router is a first-class CCB feature rather than a loose Python script.

## Context

`~/Projects/scripts/model-router.py` is a working standalone proxy that routes Claude Code
API calls by model name:
- `haiku`  → aibox:8080        (qwopus, Anthropic-compat)
- `sonnet` → localhost:11434    (Ollama, OpenAI-compat + format conversion)
- `opus`   → api.anthropic.com  (real Anthropic)

This story moves that logic into CCB as a Rust feature so it's installed with the binary,
configurable via `~/.claude/ccb.toml`, and managed with standard start/stop/status commands.

The router runs as a background process (PID file at `~/.cache/ccb/router.pid`).
Listening port default: 9001.

## Acceptance Criteria

### STEP ZERO
1. **Read `src/main.rs` and `src/cli.rs`** — follow the existing feature-gate pattern.
2. **Read `~/Projects/scripts/model-router.py`** — understand the three-route logic and
   Anthropic↔OpenAI SSE conversion that must be preserved in Rust.

### Feature Flag
3. **`route` added to `[features]` in `Cargo.toml`** with deps:
   - `tokio = { version = "1", features = ["full"], optional = true }`
   - `axum = { version = "0.7", optional = true }`
   - `reqwest = { version = "0.12", features = ["stream", "json"], optional = true }`
4. **`full` feature updated** to include `route`.

### `src/features/route.rs`
5. **Three backend configs** read from env or `~/.claude/ccb.toml` (fallback to hardcoded defaults):
   - `AIBOX_URL` / `AIBOX_MODEL` → haiku route
   - `OLLAMA_URL` / `OLLAMA_MODEL` → sonnet route
   - Real Anthropic key from `~/.secrets` → opus/default route
6. **`start(port: u16)` function** — spawns axum server as background process, writes PID to
   `~/.cache/ccb/router.pid`, prints launch env:
   ```
   Router started on :9001
   Run Claude Code with:
     ANTHROPIC_BASE_URL=http://localhost:9001 ANTHROPIC_API_KEY=router claude
   ```
7. **`stop()` function** — reads PID file, kills process, removes PID file.
8. **`status()` function** — checks PID alive, prints route table and health.
9. **`env()` function** — prints the export block to paste into shell or `.env`:
   ```
   export ANTHROPIC_BASE_URL=http://localhost:9001
   export ANTHROPIC_API_KEY=router
   ```

### Routing Logic (port from Python)
10. **`/v1/messages` POST handler** — routes by model name prefix:
    - `haiku` → forward to aibox (Anthropic-compat, pass-through)
    - `sonnet` → forward to Ollama (OpenAI-compat, convert request + SSE response)
    - all others → forward to Anthropic with real API key
11. **SSE streaming** — all three routes stream responses back to the client.
12. **Anthropic→OpenAI conversion** for the Ollama route:
    - Request: flatten `system` + `messages` content blocks → OpenAI messages array
    - Response SSE: emit `message_start`, `content_block_start`, `ping`, then
      `content_block_delta` per OpenAI chunk, then `content_block_stop` + `message_delta`
      + `message_stop`
13. **`/health` GET** — returns JSON with route table.

### CLI
14. **`Command::Route(RouteArgs)` in `src/cli.rs`** with subcommands:
    ```
    Start { #[arg(default_value = "9001")] port: u16 }
    Stop
    Status
    Env
    ```
15. **`route_cmd` dispatch** in `src/main.rs` following `graph_cmd` pattern.

### Smoke Tests
16. **`ccb route start`** — router runs, `curl http://localhost:9001/health` returns JSON.
17. **`ccb route status`** — shows three routes with URLs.
18. **`ccb route env`** — prints the two export lines.
19. **`ccb route stop`** — process gone, PID file removed.

### Gate
20. **`cargo build --features route`** — zero errors.
21. All 4 smoke tests pass.

## Files in Scope
- `Cargo.toml` — add route feature + deps
- `src/features/route.rs` (new)
- `src/cli.rs` — add Route variants
- `src/main.rs` — add mod, match arm, dispatch

## Frozen Surfaces
- `~/Projects/scripts/model-router.py` — leave in place; this story adds the CCB equivalent

## Blocked By
- None (standalone feature, no dependency on graph or expert)

## Blocks
- CCB-009

## Definition of Done
- [ ] `ccb route start` boots the proxy
- [ ] All three routes functional (haiku/sonnet/opus)
- [ ] SSE streaming works end-to-end
- [ ] `ccb route stop` cleans up
- [ ] `cargo build --features route` clean
