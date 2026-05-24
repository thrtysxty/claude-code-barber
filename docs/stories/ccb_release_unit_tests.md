# CCB-REL-004 — Unit tests for core feature modules

## Status: READY

## Working directory
`/Users/dadmin/Projects/claude-code-barber`

## Summary
CCB has integration tests but lacks unit tests inside feature modules. Each core module needs at least one test to catch regressions. Priority: classify (security-critical), expert (knowledge retrieval), trim (compression correctness).

## Acceptance Criteria

### AC1: Classify tests (`src/features/classify.rs`)
- [ ] tier1_bash safe prefixes → ALLOW
- [ ] Fast-deny patterns (rm -rf, curl|bash) → DENY
- [ ] Memory write detection → ALLOW
- [ ] Edge cases: empty input, long commands
- [ ] `cargo test --features classify` passes

### AC2: Expert tests (`src/features/expert.rs`)
- [ ] query_active_json with in-memory SQLite
- [ ] Non-existent expert returns None
- [ ] `cargo test --features expert` passes

### AC3: Trim tests (`src/features/trim.rs`)
- [ ] Compression reduces input size
- [ ] Critical content (code blocks, errors) preserved
- [ ] `cargo test` passes (trim is default)

### AC4: Coverage across remaining modules
- [ ] At least one test per module: fade, context, cut, lineup, buzz, gain, install
- [ ] `#[cfg(test)] mod tests` blocks inside each module
- [ ] `cargo test --features full` passes — all tests included
