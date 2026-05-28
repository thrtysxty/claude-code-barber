# CCB Story 017: Code Graph — File Watcher & Incremental Re-index

**Status:** READY
**Priority:** P2 — quality of life
**Sprint:** CCB-4 (Graph)
**Feature flag:** `graph`
**Depends on:** CCB-015 (edges), CCB-016 (traversal)

## Narrative
**As a** developer using CCB's code graph,
**I want** the graph to stay fresh as I edit code without re-running `ccb graph index`,
**So that** queries always reflect the current state of my codebase.

## Context

Currently `ccb graph index` does a full directory walk every time. For a large codebase this is slow and stale between runs. CodeGraphContext solves this with file watching (inotify/kqueue) and incremental updates.

CCB can use the `notify` crate for cross-platform file watching. On change, only the modified file is re-parsed and its symbols/edges updated. The `files.indexed` timestamp already tracks staleness.

## Architecture

```
ccb graph watch [path]
  └─ Start notify::Watcher on path (default: .)
  └─ On file change:
       ├─ detect_lang() — skip if unsupported
       ├─ Delete old symbols + edges for that file_id
       ├─ Re-extract symbols + edges
       ├─ Re-run resolve_edges() for new edges
       └─ Log: "re-indexed src/foo.rs (12 symbols, 8 edges)"
  └─ Ctrl-C to stop
```

## Acceptance Criteria

- [ ] **AC1:** `ccb graph watch [path]` starts a file watcher on the given directory (default `.`)
- [ ] **AC2:** File create/modify events trigger re-index of that single file
- [ ] **AC3:** File delete events remove the file's symbols and edges from the database
- [ ] **AC4:** File rename events treated as delete + create
- [ ] **AC5:** Watcher respects the same `SKIP_DIRS` list as `ccb graph index`
- [ ] **AC6:** Debounce: batch changes within 500ms to avoid thrashing on save-all
- [ ] **AC7:** Edge re-resolution runs only for edges involving the changed file, not the entire DB
- [ ] **AC8:** Logs each re-index event to stderr with file path, symbol count, edge count
- [ ] **AC9:** Graceful shutdown on SIGINT/SIGTERM
- [ ] **AC10:** `notify` added as optional dependency gated behind `graph` feature
- [ ] **AC11:** Integration test: start watcher, create a file, verify symbols appear in DB within 2 seconds

## Notes

- `notify` crate supports both inotify (Linux) and kqueue (macOS) — no platform-specific code needed
- Consider `notify-debouncer-mini` for built-in debouncing
- Watch mode is foreground/blocking — suitable for `tmux` pane or background `&`
