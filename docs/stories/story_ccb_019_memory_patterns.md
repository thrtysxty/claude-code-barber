# CCB Story 019: Memory — Pattern Mining & Skill Generation

**Status:** READY
**Priority:** P1 — core memory value
**Sprint:** CCB-5 (Memory)
**Feature flag:** `memory`
**Depends on:** CCB-018 (traces)

## Narrative
**As a** CCB user with accumulated session traces,
**I want** CCB to automatically detect repeated patterns and codify them as reusable skills,
**So that** future sessions benefit from past experience without manual skill authoring.

## Context

Hivemind's key insight: a background worker mines session traces for repeated patterns and converts them into SKILL.md files that propagate to all agents. CCB already has `style index-build` which scans `~/.claude/skills/` and generates INDEX.md. This story connects the two: traces → pattern detection → skill file generation → index rebuild.

Pattern detection is frequency-based, not LLM-based (Phase 1). If the same tool+file combination appears across N sessions, or the same error→fix sequence repeats, it's a pattern worth codifying.

## Architecture

```
ccb memory mine [--min-frequency N] [--dry-run]
  └─ Query traces.db for repeated patterns:
       ├─ Tool+file combinations across sessions (e.g. "always runs cargo test after editing .rs")
       ├─ Error→recovery sequences (e.g. "clippy lint X → applied fix Y")
       ├─ File co-edit clusters (e.g. "when A.rs changes, B.rs always changes too")
       └─ Persona→domain associations (e.g. "sentinel persona always used for security reviews")
  └─ For each pattern above threshold:
       ├─ Generate ~/.claude/skills/auto/<pattern_name>.md
       └─ Log pattern to patterns table in traces.db
  └─ Run style index-build to regenerate INDEX.md

Pattern storage (in traces.db):

  mined_patterns (
      id            INTEGER PRIMARY KEY,
      pattern_type  TEXT NOT NULL,       -- 'tool_sequence' | 'file_cluster' | 'error_fix' | 'persona_domain'
      description   TEXT NOT NULL,
      frequency     INTEGER NOT NULL,
      first_seen    TEXT NOT NULL,
      last_seen     TEXT NOT NULL,
      skill_path    TEXT,                -- path to generated skill file, if any
      suppressed    INTEGER DEFAULT 0    -- user can suppress false positives
  )
```

## Acceptance Criteria

- [ ] **AC1:** `mined_patterns` table added to traces.db schema
- [ ] **AC2:** `ccb memory mine` scans trace_events for tool+file frequency patterns (same tool on same file across ≥3 sessions)
- [ ] **AC3:** `ccb memory mine` detects file co-edit clusters (files edited in the same session ≥3 times)
- [ ] **AC4:** `ccb memory mine` detects error→fix sequences (error event followed by successful edit within same session, repeated ≥2 times)
- [ ] **AC5:** `--min-frequency N` overrides the default threshold (default: 3)
- [ ] **AC6:** `--dry-run` prints detected patterns without generating skill files
- [ ] **AC7:** Generated skill files written to `~/.claude/skills/auto/` with descriptive filenames
- [ ] **AC8:** Skill file format matches existing CCB skill conventions (markdown with front matter)
- [ ] **AC9:** After generation, automatically runs `style index-build` to update INDEX.md
- [ ] **AC10:** `ccb memory patterns [--type T]` lists all mined patterns with frequency and status
- [ ] **AC11:** `ccb memory suppress <pattern_id>` marks a pattern as suppressed (won't regenerate)
- [ ] **AC12:** Suppressed patterns are excluded from future `mine` runs
- [ ] **AC13:** Idempotent: re-running `mine` updates frequencies but doesn't duplicate skill files
- [ ] **AC14:** Unit tests: pattern detection logic with mock trace data
- [ ] **AC15:** Integration test: insert trace events across 3+ sessions, run mine, verify skill file generated

## Notes

- Phase 1 is pure frequency analysis — no LLM involved
- Phase 2 (future story) could use a local model to generate richer skill descriptions
- `auto/` subdirectory keeps generated skills separate from hand-authored ones
- Pattern mining should complete in <5 seconds for 1000 sessions — use SQL aggregation, not in-memory
