# CCB Story 018: Memory — Session Trace Capture

**Status:** READY
**Priority:** P1 — foundational for memory features
**Sprint:** CCB-5 (Memory)
**Feature flag:** `memory`
**Depends on:** None

## Narrative
**As a** CCB user running Claude Code sessions,
**I want** CCB to capture structured traces of each session (tools used, files touched, patterns applied),
**So that** insights from one session are available to future sessions without manual documentation.

## Context

Hivemind (activeloopai/hivemind) demonstrates the pattern: capture every prompt, tool call, and response as structured traces, then mine them for reusable patterns. CCB currently logs only compression events (`ccb_log.jsonl`). This story extends tracing to capture session-level metadata.

The trace format must be lightweight — CCB is a context *reduction* tool, so traces that bloat context defeat the purpose. Store structured metadata, not raw transcripts.

## Architecture

```
~/.cache/ccb/traces.db (SQLite)

  sessions (
      id          TEXT PRIMARY KEY,     -- UUID or session hash
      started     TEXT NOT NULL,        -- ISO 8601
      ended       TEXT,
      model       TEXT,                 -- from CCB_MODEL env
      persona     TEXT,                 -- active expert persona
      project     TEXT,                 -- git repo root
      summary     TEXT                  -- auto-generated on session end
  )

  trace_events (
      id          INTEGER PRIMARY KEY,
      session_id  TEXT NOT NULL REFERENCES sessions(id),
      timestamp   TEXT NOT NULL,
      kind        TEXT NOT NULL,        -- 'tool_use' | 'file_edit' | 'file_read' | 'command' | 'error' | 'pattern'
      name        TEXT NOT NULL,        -- tool name, file path, command, etc.
      metadata    TEXT,                 -- JSON blob for kind-specific data
      tokens      INTEGER              -- estimated token cost of this event
  )

Capture points (hooks):
  - SessionStart hook → create session row
  - PreToolUse hook → log tool_use event
  - PostToolUse hook → update with result metadata
  - SessionEnd hook → close session, generate summary
```

## Acceptance Criteria

- [ ] **AC1:** `traces.db` schema created with `sessions` and `trace_events` tables
- [ ] **AC2:** `ccb memory init` creates the database if absent, prints path
- [ ] **AC3:** `ccb memory log <kind> <name> [--meta JSON]` appends a trace event to the current session
- [ ] **AC4:** `ccb memory session-start [--model M] [--persona P] [--project P]` creates a new session row, prints session ID
- [ ] **AC5:** `ccb memory session-end [--summary S]` closes the current session with timestamp and optional summary
- [ ] **AC6:** `ccb memory show [--session ID] [--last N]` displays trace events (default: last session, last 20 events)
- [ ] **AC7:** `ccb memory sessions [--limit N]` lists sessions with timestamps, model, persona, event count
- [ ] **AC8:** Hook integration: `SessionStart` hook calls `ccb memory session-start` automatically
- [ ] **AC9:** Hook integration: `PreToolUse` hook calls `ccb memory log tool_use <tool_name>` automatically
- [ ] **AC10:** Trace events store estimated token cost (from `log::estimate_tokens` or explicit)
- [ ] **AC11:** Session summary auto-generation: on `session-end`, if no summary provided, generate one from the trace event names (e.g. "Edited 3 files, ran 5 commands, used trim 2x")
- [ ] **AC12:** `memory` feature flag added to Cargo.toml, gated behind `dep:rusqlite`
- [ ] **AC13:** Unit tests for session lifecycle (create, log events, close, query)
- [ ] **AC14:** CLI definitions added to `cli.rs`

## Notes

- Traces are metadata, not transcripts — store "used Bash tool on `cargo test`", not the full command output
- `metadata` JSON blob is schema-free by design — different event kinds store different fields
- Session ID can come from `CLAUDE_SESSION_ID` env var if available, else UUID
- DB is separate from `graph.db` to keep concerns isolated — graph is code structure, traces are session history
