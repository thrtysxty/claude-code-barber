# CCB-REL-002 — Create CHANGELOG.md with current feature inventory

## Status: READY

## Working directory
`/Users/dadmin/Projects/claude-code-barber`

## Summary
Create CHANGELOG.md documenting everything in v0.1.0. Follow Keep a Changelog format. Covers all feature modules and documents feature flag usage.

## Acceptance Criteria

### AC1: CHANGELOG.md at repo root
- [ ] Keep a Changelog format with `[0.1.0]` section and date
- [ ] Comparison links at bottom

### AC2: v0.1.0 feature inventory
- [ ] Document all features: trim, fade, graph, expert, classify, route, context, cut, lineup, buzz, gain, install
- [ ] One-line description per feature

### AC3: Feature flag documentation
- [ ] Note default features (trim, fade, route) vs opt-in (graph, expert, classify)
- [ ] Document `--features full` and `cargo install ccb`

### AC4: Format
- [ ] ISO 8601 dates, consistent heading levels
