# Claude Code Barber (CCB) — Comprehensive Review

**Date:** 2026-06-11  
**Reviewer:** MiMo Code Agent  
**Scope:** Full codebase review, security analysis, improvement recommendations

---

## Project Overview

| Attribute | Detail |
|-----------|--------|
| **Stack** | Rust 2021, clap CLI, rusqlite, tree-sitter, axum/tokio (router), reqwest |
| **Purpose** | Token management layer for Claude Code — compress output, lazy-load skills, monitor context, route models, safety classification |
| **Maturity** | 0.1.0 — first release shipped, feature-rich, 14 feature flags |
| **Binaries** | `ccb` (CLI) + `ccb-route` (model router) |
| **Source LOC** | ~26,600 lines Rust (including tests) |
| **Tests** | ~291 (default), ~560 (full features) |

---

## Code Quality Findings

### 1. `lib.rs` / `main.rs` Module Duplication (HIGH)
Different module trees — `main.rs` missing `loop_cmd`, different `plugins` gating. Library and binary have drifted.

### 2. Excessive `#[allow(dead_code)]` (MEDIUM)
17 instances across 8 files. `config.rs` — all 4 items dead. `analytics.rs` — 6 instances.

### 3. `FeatureConfig` Mismatch with CLI (MEDIUM)
`config.rs` defines `trim, fade, sandbox, terse, graph` but actual features include `route, expert, classify, factory, hooks, context, status, plugins, memory`.

### 4. `Box::leak` Memory Leak (LOW)
`hooks.rs:247,252,263` leaks `String`s into `&'static str` per session-start call.

### 5. Hardcoded Binary Paths (MEDIUM)
`route.rs:140-165` hardcodes `~/Projects/claude-code-barber/target/{release|debug}/ccb-route`. Breaks for `cargo install`.

### 6. Excessive `.unwrap()` (MEDIUM)
212 instances. Dangerous ones in `ccb-route.rs:1039,1050,1057` — `panic!()` in async handlers crashes the router.

### 7. `process::exit` Bypasses Drop (LOW)
4 instances — bypasses destructors, risks corrupt SQLite state.

### 8. Operator Precedence Bug in Classifier (HIGH)
`classify.rs:164-167` — `&&` binds tighter than `||`, causing `echo '| sh'` to be denied even without curl.

---

## Security Concerns

| Issue | Severity | File |
|-------|----------|------|
| `.secrets` read without permission checks | MEDIUM | `route.rs:267-281` |
| `ANTHROPIC_API_KEY_REAL` visible via `/proc` | MEDIUM | `route.rs:173` |
| `kill -9` without PID validation | LOW | `route.rs:210` |

---

## Incomplete Features / Stubs

| Feature | Status |
|---------|--------|
| `sandbox` feature flag | Declared but empty — no code |
| `terse` feature flag | Declared but empty — no code |
| `loop_cmd.rs` (707 lines) | Not wired to CLI — dead code |
| `config.rs` FeatureConfig | Unused at runtime |
| Status feature | Partially complete (13 ACs remaining) |
| Atlas Context Engine | Not started |

---

## Recommended Improvements

### P0 — Bugs / Correctness
1. Fix `classify.rs:164` operator precedence — add parentheses
2. Fix `ccb-route.rs` panic in error paths — replace `panic!()` with error responses
3. Fix `route.rs:140` hardcoded binary path — use `which` or `current_exe()`

### P1 — Code Quality
4. Deduplicate `lib.rs` / `main.rs` module declarations
5. Fix or remove `FeatureConfig`
6. Remove or implement `sandbox` and `terse` features
7. Wire `loop_cmd.rs` to CLI or delete it

### P2 — Robustness
8. Replace `process::exit` with `anyhow::bail!`
9. Replace `Box::leak` with owned `String`s
10. Add PID validation in `route.rs:stop()`
11. Audit `.unwrap()` in `ccb-route.rs` async handlers

### P3 — Polish
12. Add router proxy unit tests
13. Add file permission checks for `~/.secrets`
14. Document `hooks.rs` module

---

## Suggested Stories

| ID | Title | Priority |
|----|-------|----------|
| CCB-030 | Deduplicate lib.rs/main.rs module declarations | P1 |
| CCB-031 | Fix classify operator precedence bug | P0 |
| CCB-032 | Router error resilience — replace panic with error responses | P0 |
| CCB-033 | Dynamic binary resolution | P0 |
| CCB-034 | Router unit tests | P2 |
| CCB-035 | Clean up dead features (sandbox, terse, loop) | P2 |
| CCB-036 | Config struct alignment | P2 |
| CCB-037 | Secrets file permission audit | P3 |
| CCB-038 | PID validation on router stop | P2 |
| CCB-039 | Status feature completion (13 ACs) | P2 |
