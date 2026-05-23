# CCB Story 002: Expert Graph SQLite Schema + Core Module

**Status:** READY
**Priority:** P0 — Layer 3 foundation
**Sprint:** CCB-1 (Expert/Persona Graph)

## Narrative
**As a** CCB developer,
**I want** `src/features/expert.rs` with the knowledge graph schema and core functions,
**So that** personas and domain knowledge can be stored, retrieved, and served to hooks.

## Context

The expert/persona knowledge graph stores:
- **Personas** (`sentinel`, `coder`, `architect`, etc.) as graph nodes
- **Domains** (OWASP categories, MITRE ATT&CK nodes, implementation patterns) as graph nodes
- **Edges** connecting personas to their relevant domains
- **Patterns** attached to domains — concrete mitigations, examples, CWE references

This is NOT a LoRA, NOT prompt injection, NOT RAG text. The knowledge IS the graph. Access is SQLite traversal; output is structured JSON consumed by hooks.

DB lives at `~/.cache/ccb/expert.db` (separate from `graph.db`).

## Schema

```sql
CREATE TABLE IF NOT EXISTS personas (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS domains (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    category    TEXT NOT NULL   -- "security", "implementation", "architecture", etc.
);

CREATE TABLE IF NOT EXISTS persona_domains (
    persona_id  INTEGER NOT NULL REFERENCES personas(id) ON DELETE CASCADE,
    domain_id   INTEGER NOT NULL REFERENCES domains(id)  ON DELETE CASCADE,
    weight      REAL NOT NULL DEFAULT 1.0,
    PRIMARY KEY (persona_id, domain_id)
);

CREATE TABLE IF NOT EXISTS patterns (
    id          INTEGER PRIMARY KEY,
    domain_id   INTEGER NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    pattern_id  TEXT NOT NULL,   -- e.g. "CWE-22", "OWASP-A01"
    name        TEXT NOT NULL,
    mitigations TEXT NOT NULL    -- JSON array of strings
);

CREATE TABLE IF NOT EXISTS active_persona (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    persona_id  INTEGER REFERENCES personas(id)
);

CREATE INDEX IF NOT EXISTS idx_persona_domains ON persona_domains(persona_id);
CREATE INDEX IF NOT EXISTS idx_patterns_domain  ON patterns(domain_id);
```

## Functions to Implement

```rust
pub fn build(name: &str, dataset_path: &std::path::Path) -> anyhow::Result<()>
// Parse dataset JSON, upsert persona + its domains + patterns into SQLite.
// Dataset format: see AC 5.

pub fn activate(name: &str) -> anyhow::Result<()>
// Set active_persona to the named persona. Error if persona not found.

pub fn deactivate() -> anyhow::Result<()>
// Clear active_persona (set to NULL).

pub fn list() -> anyhow::Result<()>
// Print all personas, their domain count, and whether currently active.

pub fn query_active(format: OutputFormat) -> anyhow::Result<()>
// Get active persona + all connected domains + patterns.
// Return structured JSON (Json) or human table (Human).
```

## Acceptance Criteria

### STEP ZERO
1. **Read `src/main.rs` and `src/cli.rs`** — verify the feature-gate pattern and module wiring used by existing features. Follow exactly.

### Module
2. **`src/features/expert.rs` created** gated with `#[cfg(feature = "expert")]` in `src/main.rs` pub mod block.

### Schema
3. **DB at `~/.cache/ccb/expert.db`** — created on first run of any `ccb expert` command.
4. **All five tables + two indexes** created with `CREATE TABLE IF NOT EXISTS`.

### Dataset Format
5. **`build()` parses this JSON shape:**
```json
{
  "persona": "sentinel",
  "description": "Security domain expert",
  "domains": [
    {
      "name": "path_traversal",
      "category": "security",
      "patterns": [
        {"id": "CWE-22", "name": "Path Traversal", "mitigations": ["validate input", "resolve then check root"]}
      ]
    }
  ]
}
```

### Functions
6. **`activate("sentinel")` sets active_persona** — subsequent `query_active()` returns sentinel's domains.
7. **`activate("nonexistent")` returns an error** — clear message, no panic.
8. **`deactivate()` clears active_persona** — `query_active()` returns empty/null after.
9. **`list()` prints all personas** — marks active one clearly.

### JSON Output
10. **`query_active(Json)` returns valid JSON:**
```json
{
  "persona": "sentinel",
  "active_domains": ["path_traversal", "sql_injection"],
  "patterns": [
    {"id": "CWE-22", "name": "Path Traversal", "mitigations": ["validate input", "resolve then check root"]}
  ]
}
```
Pipe through `python3 -m json.tool` to verify.

### Gate
11. **`cargo check --features expert`** — zero errors.

## Files in Scope
- `src/features/expert.rs` (new)
- `src/main.rs` — add `#[cfg(feature = "expert")] pub mod expert;` to features block only

## Frozen Surfaces
- All existing feature modules — do not modify
- `src/cli.rs` — CLI wiring is Story CCB-003

## Blocked By
- Story CCB-001

## Blocks
- Story CCB-003, CCB-004

## Definition of Done
- [ ] `src/features/expert.rs` created
- [ ] Schema creates correctly
- [ ] `build()`, `activate()`, `deactivate()`, `list()`, `query_active()` implemented
- [ ] JSON output valid (`python3 -m json.tool`)
- [ ] `cargo check --features expert` clean
