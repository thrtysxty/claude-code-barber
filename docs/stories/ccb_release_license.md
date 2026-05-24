# CCB-REL-001 — Add MIT LICENSE and verify license metadata

## Status: READY

## Working directory
`/Users/dadmin/Projects/claude-code-barber`

## Summary
CCB declares `license = "MIT"` in Cargo.toml but no LICENSE file exists. crates.io will warn on publish without one. Add the file, verify metadata, add README badge.

## Acceptance Criteria

### AC1: MIT LICENSE file at repo root
- [ ] Create `LICENSE` with full MIT text
- [ ] Copyright holder: "thrtysxty contributors"
- [ ] Copyright year: 2025-2026
- [ ] Plain text, no `.md` extension

### AC2: Cargo.toml license field matches
- [ ] Verify `license = "MIT"` present in `[package]`

### AC3: README license badge
- [ ] Add MIT license badge (shields.io) linking to LICENSE file

### AC4: Verification
- [ ] `cargo package --list` includes `LICENSE`
- [ ] `cargo publish --dry-run` produces no license warnings
