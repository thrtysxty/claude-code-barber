# CCB Story 016: Code Graph — Traversal & Impact Analysis

**Status:** READY
**Priority:** P1 — user-facing graph queries
**Sprint:** CCB-4 (Graph)
**Feature flag:** `graph`
**Depends on:** CCB-015 (edges table + extraction)

## Narrative
**As a** developer using CCB's code graph,
**I want** to query callers, callees, dependency chains, and impact radius for any symbol,
**So that** I can understand execution flow and assess change risk before editing code.

## Context

With edges from CCB-015, the graph supports traversal. This story adds the CLI subcommands and query logic to make the graph useful: "who calls this?", "what does this call?", "trace the path from A to B", "what's unreachable?".

CodeGraphContext supports all of these. CCB should match the query surface while keeping output token-efficient (this is still a context management tool).

## Architecture

```
ccb graph callers <symbol>     → reverse edge traversal (who calls this?)
ccb graph callees <symbol>     → forward edge traversal (what does this call?)
ccb graph chain <from> <to>    → BFS shortest path through call edges
ccb graph impact <symbol>      → transitive reverse traversal (everything that depends on this)
ccb graph dead                 → symbols with zero inbound edges (potential dead code)
ccb graph complexity <file>    → cyclomatic complexity per function (count branch nodes in AST)
```

All commands support `--format human|json` and `--depth N` (default 3 for transitive queries).

## Acceptance Criteria

- [ ] **AC1:** `ccb graph callers <name>` — list all symbols with a `calls` edge targeting `<name>`, with file:line
- [ ] **AC2:** `ccb graph callees <name>` — list all `calls` edges sourced from `<name>`, with target file:line
- [ ] **AC3:** `ccb graph chain <from> <to>` — BFS through `calls` edges, print shortest call path. Print "no path found" if unreachable
- [ ] **AC4:** `ccb graph impact <name> [--depth N]` — transitive reverse traversal through all edge kinds, default depth 3
- [ ] **AC5:** `ccb graph dead [--kind fn|class|all]` — symbols with zero inbound edges of any kind. Default `--kind fn`
- [ ] **AC6:** `ccb graph complexity <file>` — per-function cyclomatic complexity (count `if`, `match`, `for`, `while`, `&&`, `||` nodes inside each function body)
- [ ] **AC7:** All new subcommands support `--format human|json`
- [ ] **AC8:** JSON output uses consistent schema: `{ "query": ..., "results": [...], "depth": N }`
- [ ] **AC9:** Transitive queries respect `--depth` to prevent unbounded traversal
- [ ] **AC10:** CLI definitions added to `cli.rs` `GraphCmd` enum
- [ ] **AC11:** Unit tests for BFS chain finding (path exists, no path, cycle handling)
- [ ] **AC12:** Integration test: index fixture repo, verify callers/callees/chain/dead output

## Notes

- Cyclomatic complexity uses tree-sitter node counting, not edge analysis — it's a per-function metric
- Dead code detection only finds *structurally* unreachable code — it can't detect runtime-only calls (e.g. via function pointers, reflection)
- Chain query should cap at depth 20 to prevent runaway on large graphs
