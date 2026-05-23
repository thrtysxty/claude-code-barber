# CCB Story 005: Rust Integration Tests — trim + fade

**Status:** READY
**Priority:** P1 — test coverage
**Sprint:** CCB-2 (Rust Test Suite)

## Narrative
**As a** CCB developer,
**I want** `tests/trim_integration.rs` and `tests/fade_integration.rs`,
**So that** the core token management features have regression coverage before ship.

## Context

CCB uses `assert_cmd` and `predicates` (already in `[dev-dependencies]`) for CLI integration tests. Tests invoke the compiled binary and assert on stdout/stderr/exit codes. No mocking — hit the real binary.

Reference: `assert_cmd` docs — `Command::cargo_bin("ccb")`.

## Acceptance Criteria

### STEP ZERO
1. **Read `src/features/trim.rs`** and `src/features/fade.rs` — understand what each command does and what output to assert against.
2. **Read `Cargo.toml`** — confirm `assert_cmd` and `predicates` are in `[dev-dependencies]`.

### `tests/trim_integration.rs`
3. **`test_trim_compresses_output`** — run `ccb trim echo "line1\nline2\nline3"`, assert exit 0, stdout non-empty.
4. **`test_trim_no_args_fails`** — run `ccb trim` with no subcommand args, assert exit non-zero.
5. **`test_trim_stderr_included`** — run a command that writes to stderr (e.g. `ccb trim cat /nonexistent`), assert exit 0 (trim runs the command and captures output regardless of the inner command's exit).
6. **`test_trim_logs_to_jsonl`** — after any trim run, assert `~/.claude/ccb_log.jsonl` exists and the last line is valid JSON containing `"feature":"trim"`.

### `tests/fade_integration.rs`
7. **`test_fade_runs`** — `ccb fade`, assert exit 0.
8. **`test_fade_with_resource`** — `ccb fade some_skill`, assert exit 0 (fade is a soft operation; it doesn't error on unknown resources).

### Gate
9. **`cargo test --features trim,fade`** — 0 FAILED.
10. **`cargo test --features full`** — 0 FAILED (no regressions).

## Files in Scope
- `tests/trim_integration.rs` (new)
- `tests/fade_integration.rs` (new)

## Frozen Surfaces
- `src/features/trim.rs`, `src/features/fade.rs` — do not modify implementation to fit tests

## Blocked By
- None (trim and fade are already implemented)

## Blocks
- None

## Definition of Done
- [ ] `tests/trim_integration.rs` — all 4 tests pass
- [ ] `tests/fade_integration.rs` — both tests pass
- [ ] `cargo test --features full` clean
