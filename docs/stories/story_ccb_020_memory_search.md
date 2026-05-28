# CCB Story 020: Memory — Hybrid Search & Context Injection

**Status:** READY
**Priority:** P1 — makes memory useful at inference time
**Sprint:** CCB-5 (Memory)
**Feature flag:** `memory`
**Depends on:** CCB-018 (traces), CCB-019 (patterns)

## Narrative
**As a** CCB user starting a new session,
**I want** to search past sessions for relevant context and have it injected into my current session,
**So that** I don't re-solve problems that past sessions already handled.

## Context

Hivemind uses hybrid search (semantic embeddings + BM25 lexical fallback). CCB can implement this local-first: BM25 via SQLite FTS5 (zero dependencies), with optional embedding support via the expert system's existing infrastructure.

The key integration point is hook-based: a `SessionStart` hook queries memory for relevant context based on the current project/persona/task, and injects it as a compact context block.

## Architecture

```
Search pipeline:

  ccb memory search <query> [--limit N] [--project P]
    ├─ FTS5 full-text search on trace_events.name + metadata
    ├─ BM25 ranking (built into FTS5)
    ├─ Optional: embedding similarity on session summaries (if embeddings exist)
    └─ Output: ranked list of relevant sessions/events with context snippets

Context injection:

  ccb memory recall [--project P] [--persona P] [--task T]
    └─ Combines:
         ├─ Relevant mined patterns for this project
         ├─ Recent session summaries for this project
         ├─ Error→fix patterns seen in this codebase
         └─ Active skills from auto/ directory
    └─ Output: compact markdown block (≤500 tokens) for hook injection

Hook integration:

  SessionStart hook → ccb memory recall --project $(git root) --persona $CCB_PERSONA
    └─ Output appended to session context
```

## Acceptance Criteria

- [ ] **AC1:** FTS5 virtual table created on `trace_events(name, metadata)` for full-text search
- [ ] **AC2:** `ccb memory search <query>` returns ranked results with session ID, timestamp, event kind, and name
- [ ] **AC3:** Search supports `--project P` filter (match on sessions.project)
- [ ] **AC4:** Search supports `--limit N` (default 10)
- [ ] **AC5:** Results include BM25 relevance score
- [ ] **AC6:** `--format human|json` output modes
- [ ] **AC7:** `ccb memory recall` generates a compact context block from relevant patterns + recent sessions
- [ ] **AC8:** Recall output is capped at 500 tokens (estimated via `log::estimate_tokens`)
- [ ] **AC9:** Recall prioritizes: (1) error→fix patterns for current project, (2) mined skills, (3) recent session summaries
- [ ] **AC10:** Recall output is valid markdown suitable for hook injection
- [ ] **AC11:** Hook example provided: SessionStart hook that calls `ccb memory recall` and injects output
- [ ] **AC12:** `ccb memory search` with no results prints "No matching sessions found" (not an error)
- [ ] **AC13:** Unit tests for FTS5 search ranking
- [ ] **AC14:** Integration test: insert sessions with known content, verify search returns correct results ranked by relevance

## Notes

- FTS5 is built into rusqlite's `bundled` feature — no additional dependencies
- Embedding support is Phase 2 — BM25/FTS5 is sufficient for text-based recall
- The 500-token cap on recall output is critical — this is a context *reduction* tool
- Recall should degrade gracefully: if no traces exist, output nothing (don't inject stale/empty blocks)
