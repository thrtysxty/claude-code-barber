# CCB Story 015: Code Graph — Edges & Relationships

**Status:** READY
**Priority:** P1 — foundational for graph features
**Sprint:** CCB-4 (Graph)
**Feature flag:** `graph`
**Depends on:** None

## Narrative
**As a** CCB user indexing a codebase,
**I want** the graph to track relationships between symbols (calls, imports, inheritance),
**So that** I can answer "what calls this?", "what does this depend on?", and "what breaks if I change this?"

## Context

CCB's `graph.rs` currently stores symbol *definitions* in a flat `symbols` table (name, kind, file, line) with no relationship tracking. This makes it a symbol index, not a graph.

CodeGraphContext (github.com/CodeGraphContext/CodeGraphContext) demonstrates the standard: caller→callee edges, import dependencies, inheritance/impl relationships, and transitive traversal across files. CCB needs parity.

The existing tree-sitter walk in `walk_tree()` already visits every node — extending it to capture *references* (function calls, use/import statements, type annotations) is a second pass over the same AST.

## Architecture

```
graph.db schema additions:

  edges (
      id          INTEGER PRIMARY KEY,
      source_id   INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
      target_name TEXT NOT NULL,        -- unresolved name (e.g. "foo", "Bar::new")
      target_id   INTEGER REFERENCES symbols(id) ON DELETE SET NULL,  -- resolved after indexing
      kind        TEXT NOT NULL,        -- 'calls' | 'imports' | 'inherits' | 'implements'
      line        INTEGER NOT NULL      -- call site line number
  )

  Resolution is two-phase:
  1. walk_tree() extracts edges with target_name (unresolved)
  2. resolve_edges() joins target_name → symbols.name to fill target_id
```

## Acceptance Criteria

- [ ] **AC1:** `edges` table created in graph.db schema migration (source_id, target_name, target_id nullable, kind, line)
- [ ] **AC2:** Rust — extract `call_expression` nodes → `calls` edges (function name + line)
- [ ] **AC3:** Rust — extract `use_declaration` nodes → `imports` edges
- [ ] **AC4:** Rust — extract `impl_item` with trait → `implements` edges
- [ ] **AC5:** Python — extract `call` nodes → `calls` edges
- [ ] **AC6:** Python — extract `import_statement` / `import_from_statement` → `imports` edges
- [ ] **AC7:** Python — extract `class_definition` with base classes → `inherits` edges
- [ ] **AC8:** TypeScript/JavaScript — extract `call_expression` → `calls` edges
- [ ] **AC9:** TypeScript/JavaScript — extract `import_statement` → `imports` edges
- [ ] **AC10:** TypeScript/JavaScript — extract `class_heritage` → `inherits`/`implements` edges
- [ ] **AC11:** Post-index resolution pass: match `target_name` to `symbols.name`, fill `target_id` where unambiguous (single match)
- [ ] **AC12:** `ccb graph index` reports edge count alongside symbol count
- [ ] **AC13:** Indexes on `edges(source_id)`, `edges(target_id)`, `edges(target_name)` for fast traversal
- [ ] **AC14:** Unit tests: edge extraction for each language (at least 2 per language × 4 languages = 8 tests)
- [ ] **AC15:** Integration test: index a multi-file fixture, verify cross-file edges resolve

## Notes

- `target_name` stays populated even if `target_id` is NULL — supports "unresolved reference" queries
- Method calls like `self.foo()` or `obj.bar()` store the method name; receiver type resolution is Phase 2
- Don't attempt full type inference — name-based resolution catches 80% of cases
- Keep `walk_tree` changes additive — existing symbol extraction must not regress
