# CCB Story 009: Expert Dataset Ingestion

**Status:** READY
**Priority:** P0 — knowledge pipeline
**Sprint:** CCB-2 (Expert Knowledge)

## Narrative
**As a** CCB user who has activated an expert persona,
**I want** to load domain knowledge from a YAML file into the expert graph,
**So that** the hook query returns real patterns and mitigations instead of an empty graph.

## Context

The expert graph schema (CCB-002) defines the tables. This story populates them. The ingest command reads a structured YAML dataset, upserts domains and patterns into SQLite, and links them to the named persona. Running ingest twice is safe — all operations are upsert.

## Dataset YAML Schema

```yaml
persona: sentinel
domains:
  - name: path_traversal
    display: "Path Traversal"
    patterns:
      - id: "CWE-22"
        name: "Path Traversal"
        mitigations:
          - "Resolve path then verify it starts with expected root"
          - "Reject paths containing '..' before resolution"
          - "Use allowlist of permitted directories"
  - name: sql_injection
    display: "SQL Injection"
    patterns:
      - id: "CWE-89"
        name: "SQL Injection"
        mitigations:
          - "Parameterized queries only — never string concatenation"
          - "Allowlist input validation before query construction"
          - "Least-privilege DB user — no DROP/ALTER in app role"
```

## Acceptance Criteria

### STEP ZERO
1. Read `src/features/expert.rs` (CCB-002) — verify `personas`, `domains`, `patterns` table schema and `persona_domains` join table.

### New CLI Variant
2. `ExpertCmd::Ingest` added to `src/cli.rs`:
```rust
/// Load domain knowledge from a YAML dataset into the expert graph
Ingest {
    persona: String,
    dataset: std::path::PathBuf,
},
```
3. Dispatch in `expert_cmd` → `features::expert::ingest(&persona, &dataset)`.

### Ingest Behavior
4. Reads and validates YAML against the schema above — bail with clear error if malformed.
5. Upserts persona row (creates if not exists).
6. Upserts each domain — `INSERT OR REPLACE INTO domains`.
7. Upserts each pattern with mitigations stored as JSON array in a `mitigations TEXT` column.
8. Links persona → domain in `persona_domains` (upsert, no duplicates).
9. Prints summary on completion:
```
Ingested sentinel: 2 domains, 6 patterns
```
10. Running ingest twice produces the same result — idempotent.

### Dependencies
11. Add `serde_yaml` to `Cargo.toml` under `expert` feature deps:
```toml
serde_yaml = { version = "0.9", optional = true }
```
Update `expert` feature to include `dep:serde_yaml`.

### Smoke Tests
12. `ccb expert ingest sentinel ~/.claude/experts/sentinel.yaml` — exits 0, prints summary.
13. Running it again — same output, no duplicates in DB.
14. `ccb expert query --format json` after ingest — returns populated JSON with patterns.

## Files in Scope
- `src/cli.rs` — add `Ingest` to `ExpertCmd`
- `src/main.rs` — add ingest dispatch
- `src/features/expert.rs` — implement `ingest()`, add `mitigations` column if missing
- `Cargo.toml` — add `serde_yaml`

## Blocked By
- CCB-002, CCB-003

## Blocks
- CCB-010 (sentinel dataset must exist to smoke test this)
- CCB-007 (integration tests need ingest to populate test data)

## Definition of Done
- [ ] `ccb expert ingest` command works end-to-end
- [ ] Idempotent — safe to run twice
- [ ] `cargo build --features expert` clean
- [ ] Smoke test 14 passes (query returns populated data after ingest)
