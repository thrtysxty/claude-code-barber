# CCB Story 006: Rust Integration Tests — graph

**Status:** READY
**Priority:** P1 — test coverage
**Sprint:** CCB-2 (Rust Test Suite)

## Narrative
**As a** CCB developer,
**I want** `tests/graph_integration.rs`,
**So that** the code symbol graph has regression coverage before ship.

## Context

Tests run against the compiled binary (`cargo test --features graph`). Each test gets a temp directory with a small fixture file to index. JSON output is validated with `serde_json::from_str`.

## Acceptance Criteria

### STEP ZERO
1. **Read `src/features/graph.rs`** — understand the index/search/show/stats function signatures and output formats.

### Fixture Setup
2. **Helper `tmp_repo()`** — creates a temp dir containing:
   - `main.rs` with one `fn main() {}` and one `struct Foo {}`
   - `lib.py` with one `def bar():` and one `class Baz:`

### Test Cases
3. **`test_graph_index_creates_db`** — run `ccb graph index <tmp_dir>`, assert exit 0, assert `~/.cache/ccb/graph.db` exists.
4. **`test_graph_search_finds_symbol`** — after indexing, run `ccb graph search main`, assert stdout contains "main".
5. **`test_graph_search_json_valid`** — `ccb graph search foo --format json`, pipe stdout through `serde_json::from_str::<serde_json::Value>()`, assert no error, assert `results` key exists.
6. **`test_graph_show_file`** — `ccb graph show <tmp_dir>/main.rs`, assert stdout contains "fn" or "struct".
7. **`test_graph_show_json_valid`** — `ccb graph show <tmp_dir>/main.rs --format json`, assert valid JSON, assert `symbols` key exists.
8. **`test_graph_stats`** — `ccb graph stats`, assert exit 0, stdout non-empty.
9. **`test_graph_stats_json_valid`** — `ccb graph stats --format json`, assert valid JSON, assert `total_files` key exists.
10. **`test_graph_show_unindexed_file`** — `ccb graph show /tmp/nonexistent.rs`, assert exit 0 (graceful), stdout contains "not indexed" or similar message (not a panic).

### Gate
11. **`cargo test --features graph`** — 0 FAILED.

## Files in Scope
- `tests/graph_integration.rs` (new)

## Frozen Surfaces
- `src/features/graph.rs` — do not modify implementation to fit tests

## Blocked By
- Story CCB-000 (graph feature must be implemented)

## Blocks
- None

## Definition of Done
- [ ] `tests/graph_integration.rs` — all 8 tests pass
- [ ] `cargo test --features graph` clean
