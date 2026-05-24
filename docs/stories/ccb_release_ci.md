# CCB-REL-003 — GitHub Actions CI and release workflows

## Status: READY

## Working directory
`/Users/dadmin/Projects/claude-code-barber`

## Summary
Two workflows: CI on push (test, clippy, fmt for default and full features) and release on tag push (crates.io publish + cross-platform binary builds). Existing ci.yml should be extended or replaced.

## Acceptance Criteria

### AC1: CI workflow
- [ ] Triggers on push to main and PRs
- [ ] Matrix: default features AND `--features full`
- [ ] cargo test, cargo clippy -D warnings, cargo fmt --check
- [ ] Cargo registry and target caching

### AC2: Release — crates.io publish
- [ ] Triggers on `v*` tag push
- [ ] Dry-run before actual publish
- [ ] Uses `CARGO_REGISTRY_TOKEN` secret

### AC3: Release — binary builds
- [ ] Matrix: x86_64-apple-darwin, aarch64-apple-darwin, x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu
- [ ] Built with `--features full --release`
- [ ] Archived as `ccb-{target}.tar.gz`

### AC4: GitHub Release
- [ ] Creates release from tag, attaches binaries
- [ ] Marked as latest
