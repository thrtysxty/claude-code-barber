# CCB Story 004: Expert Hook Query + Claude Code Integration

**Status:** READY
**Priority:** P0 — Layer 3 hook contract
**Sprint:** CCB-1 (Expert/Persona Graph)

## Narrative
**As a** Claude Code user with CCB installed,
**I want** a `ccb expert query` command and hook configuration,
**So that** PreToolUse hooks can surface domain knowledge to the model as structured JSON — zero prompt injection, zero context waste.

## Context

The hook fires when Claude Code invokes a tool (Read, Bash, etc.). CCB queries the active persona's connected domains and patterns from SQLite and writes JSON to stdout. Claude Code injects this into the model's context as structured data, not as prose.

The knowledge is the graph. Access is traversal. The model gets typed data, not a system prompt dump.

## Acceptance Criteria

### STEP ZERO
1. **Read `src/features/expert.rs`** (from CCB-002) — verify `query_active()` signature and JSON output shape.

### New CLI Variant
2. **`ExpertCmd::Query` added** to `src/cli.rs`:
```rust
/// Query active persona for hook consumption (used by PreToolUse hook)
Query {
    #[arg(long)]
    tool: Option<String>,       // tool_name from hook env
    #[arg(long, value_enum, default_value = "json")]
    format: OutputFormatArg,
},
```
3. **`expert_cmd` dispatch** handles `ExpertCmd::Query` → `features::expert::query_active(fmt(&format))`.

### Query Behavior
4. **No active persona** → exits 0, prints `{}` (empty JSON object) — hook gets no-op, never errors.
5. **Active persona with domains** → prints JSON matching this shape:
```json
{
  "persona": "sentinel",
  "active_domains": ["path_traversal", "sql_injection"],
  "patterns": [
    {"id": "CWE-22", "name": "Path Traversal", "mitigations": ["validate input", "resolve then check root"]},
    {"id": "CWE-89", "name": "SQL Injection",  "mitigations": ["parameterized queries", "allowlist validation"]}
  ]
}
```
6. **`--tool Read` filters patterns** — returns only domains relevant to file reads (future: tool-aware filtering; for now, return all active domains regardless of `--tool`).
7. **JSON is always valid** — `python3 -m json.tool` passes on all output.

### Hook Configuration
8. **`hooks/expert_pretooluse.sh` created** — shell script that invokes `ccb expert query --format json` and exits with its exit code:
```bash
#!/usr/bin/env bash
exec ~/.local/bin/ccb expert query --tool "${TOOL_NAME:-}" --format json
```
9. **Documented Claude Code settings.json snippet** added as a comment in the script:
```json
{
  "hooks": {
    "PreToolUse": [{
      "hooks": [{ "type": "command", "command": "~/.local/bin/ccb expert query --format json" }]
    }]
  }
}
```

### Smoke Tests
10. **`ccb expert query`** with no active persona → prints `{}`, exit 0.
11. **`ccb expert query`** with active persona → valid JSON, exit 0.
12. **`ccb expert query --format json | python3 -m json.tool`** — passes.

## Files in Scope
- `src/cli.rs` — add `Query` to `ExpertCmd`
- `src/main.rs` — add query dispatch
- `src/features/expert.rs` — ensure `query_active()` handles no-persona case gracefully
- `hooks/expert_pretooluse.sh` (new)

## Frozen Surfaces
- `hooks/context_monitor.sh`, `hooks/skill_loader.sh` — do not modify

## Blocked By
- Story CCB-003

## Blocks
- Story CCB-007 (integration tests)

## Definition of Done
- [ ] `ExpertCmd::Query` in `src/cli.rs`
- [ ] `hooks/expert_pretooluse.sh` created and executable
- [ ] All 3 smoke tests pass
- [ ] JSON always valid
- [ ] `cargo build --features expert` clean
