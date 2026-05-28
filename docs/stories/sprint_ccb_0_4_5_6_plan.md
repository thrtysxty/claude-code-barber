# CCB Sprint 0, 4, 5 & 6 Plan — Loop + Graph + Memory + Context Authority

**Created:** 2026-05-28
**Updated:** 2026-05-28
**Status:** PLANNED

## Overview

Four sprints, thirteen stories. Sprint 0 builds the tool that builds everything else. Sprints 4-6 use it.

```
Sprint 0: Build the loop (plan→build→verify pipeline)
Sprint 4: Build the data sources (code graph) — USING the loop
Sprint 5: Build the feedback mechanism (memory + benchmark) — USING the loop
Sprint 6: Build the roof (context authority) — USING the loop, feeds back into it
```

The architecture follows the Karpathy principle: index everything once, retrieve what's relevant per turn, stop paying tokens for bulk injection. The context authority is the roof — everything else is a data source that feeds it. The loop is the construction method — it structures how everything gets built.

---

## Sprint 0: Plan → Build → Verify Loop

Build the development pipeline before starting feature work. Every subsequent sprint uses this infrastructure.

| Story | Title | Priority | ACs | Est. Sessions | Depends On |
|-------|-------|----------|-----|---------------|------------|
| CCB-028 | Plan → Build → Verify Loop | P1 | 40 | 2-3 | None |

### The Pipeline

Informed by: BMAD-METHOD (persona-per-stage pipeline), Agent-Spec (traceability: tests→requirements→stories, typed contracts between stages), OpenSpec (artifact structure: proposal→spec→design→tasks), agentic-loop/RALPH (autonomous code→test→commit loop with failure persistence).

```
ccb plan <story.md>                    ccb build [--plan <plan.json>]
┌──────────────────────────┐           ┌──────────────────────────────────┐
│                          │           │                                  │
│  1. INTERVIEW            │           │  4. SENTINEL REVIEW              │
│     Read story, parse    │           │     Security pre-check on        │
│     ACs, ask clarifying  │           │     implementation approach      │
│     questions, identify  │           │     (expert:sentinel)            │
│     ambiguities          │           │                                  │
│     (BMAD: brainstorm)   │           │  5. IMPLEMENT                    │
│     (Agent-Spec: analysis)│          │     TDD: tests first, then code  │
│                          │           │     Per-phase, per-AC            │
│  2. SENTINEL PRE-CHECK   │           │     (BMAD: development)          │
│     Security review of   │           │                                  │
│     the approach before  │           │  6. GATE                         │
│     design begins        │           │     Repo-detected quality gates  │
│     (expert:sentinel)    │           │     Rust: fmt→clippy→test→build  │
│                          │           │     TS: tsc→lint→test→build      │
│  3. ARCHITECT             │           │     Swift: build→test            │
│     Design phases,       │           │     Python: pyright→pytest       │
│     structure the plan,  │           │     (RALPH: 5-stage pipeline)    │
│     assign ACs to phases │           │                                  │
│     (expert:architect)   │           │  7. VALIDATE                     │
│     (OpenSpec: design.md)│           │     AC traceability: each AC     │
│                          │           │     verified against code        │
│  Output: plan.json       │           │     (Agent-Spec: traceability)   │
│  (OpenSpec: tasks.md)    │           │                                  │
│                          │           │  8. COMMIT + NEXT PHASE          │
└──────────────────────────┘           │     On all gates pass: commit    │
                                       │     Move to next phase           │
ccb lesson <description>               │     After all phases: push + PR  │
┌──────────────────────────┐           │                                  │
│  Capture failure pattern │           │  FAILURE HANDLING                │
│  Store in lessons/       │           │  Save context to failures/       │
│  Auto-loaded on next run │           │  Retry with context (max 3)      │
│  → migrates to CCB-019   │           │  3 strikes: STOP, report         │
│    when memory ships     │           │  (RALPH: last_failure.txt)       │
└──────────────────────────┘           └──────────────────────────────────┘
```

### Expert Persona Integration

Each pipeline stage activates the relevant expert persona (when available):

| Stage | Expert | Role |
|-------|--------|------|
| Interview | — | Parse story, identify ambiguities |
| Sentinel pre-check | `sentinel` | Flag security concerns in the approach |
| Architect | `architect` | Design phases, identify blast radius |
| Sentinel review | `sentinel` | Review implementation for security |
| Implement | `coder` | TDD implementation guidance |
| Validate | — | AC traceability verification |

Experts are optional — the loop works without them but produces better results with them. When the context authority (024-025) ships, expert activation becomes automatic based on file/domain context.

**Sprint 0 deliverables:**
- `ccb plan` / `ccb build` / `ccb lesson` commands
- `ccb detect` — repo type detection (Rust, TS, Swift, Python)
- `ccb gates` — quality gate registry, `ccb gates --run` for standalone verification
- Failure persistence in `~/.cache/ccb/failures/`
- Lesson storage in `~/.cache/ccb/lessons/`
- STEP ZERO enforcement (git log check before implementation)

**Sprint 0 new feature flags:** `loop` (included in `full`)

**Sprint 0 success criteria:**
1. `ccb plan` on CCB-015 story produces a valid phased plan with correct Rust gates
2. `ccb gates --run` passes on current CCB main branch
3. `ccb build` executes a single-phase change, gates pass, commits to feature branch
4. After intentional gate failure: failure context saved, retry loads it, 3-strike stops the loop

**Sprint 0 validation:** Sprint 4 is the proof. If the loop works, CCB-015 gets built faster and cleaner than any prior CCB story. If it doesn't, fix the loop before continuing.

---

## Sprint 4: Full Code Graph

Transform CCB's flat symbol index into a real code graph with edges, traversal, and live updates.

**Built using:** `ccb plan` + `ccb build` from Sprint 0.

| Story | Title | Priority | ACs | Est. Sessions | Depends On |
|-------|-------|----------|-----|---------------|------------|
| CCB-015 | Graph Edges & Relationships | P1 | 15 | 2-3 | — |
| CCB-016 | Graph Traversal & Impact Analysis | P1 | 12 | 2 | CCB-015 |
| CCB-017 | File Watcher & Incremental Re-index | P2 | 11 | 1-2 | CCB-015 |

**Sprint 4 deliverables:**
- `edges` table with call/import/inherit extraction for Rust, Python, TS/JS
- `ccb graph callers|callees|chain|impact|dead|complexity` subcommands
- `ccb graph watch` for live re-indexing
- New Cargo dependency: `notify` (optional, gated behind `graph`)

**Sprint 4 new feature flags:** None (extends existing `graph`)

**Sprint 4 risk:** Edge resolution accuracy. Name-based matching will produce false positives for common names (`new`, `get`, `set`). Acceptable for Phase 1 — type-aware resolution is a future story.

---

## Sprint 5: Memory + Validation Harness

Add session memory (traces, pattern mining, search) and the LoCoMo quality benchmark. LoCoMo is built early — it's not just a benchmark, it's the validation gate for Sprint 6's weight tuning.

**Built using:** `ccb plan` + `ccb build` from Sprint 0.

| Story | Title | Priority | ACs | Est. Sessions | Depends On |
|-------|-------|----------|-----|---------------|------------|
| CCB-018 | Memory — Session Trace Capture | P1 | 14 | 2 | — |
| CCB-019 | Memory — Pattern Mining & Skill Gen | P1 | 15 | 2-3 | CCB-018 |
| CCB-020 | Memory — Hybrid Search & Injection | P1 | 14 | 2 | CCB-018, CCB-019 |
| CCB-021 | LoCoMo Benchmark | P1 | 15 | 2 | — |

**Sprint 5 deliverables:**
- `traces.db` with session + event tracking
- `ccb memory init|log|session-start|session-end|show|sessions|mine|patterns|suppress|search|recall` subcommands
- Auto-generated skills in `~/.claude/skills/auto/`
- FTS5-based hybrid search with BM25 ranking
- Hook-based context injection (SessionStart → recall)
- `ccb gain --locomo` quality benchmark with retention curves
- Bundled LoCoMo test subset in `testdata/locomo/`

**Sprint 5 new feature flags:** `memory` (deps: `rusqlite`)

**Sprint 5 risk:** Pattern mining sensitivity. Too aggressive = noisy skills. Too conservative = no value. The `--min-frequency` threshold and `suppress` command are the pressure valves.

**Sprint 5 critical note:** CCB-021 (LoCoMo) is prerequisite infrastructure for Sprint 6, not an optional benchmark. Weight tuning without LoCoMo is guesswork. With LoCoMo, weight tuning is validated science. Build it early in the sprint.

**Sprint 5 bootstrap:** When CCB-018 (traces) ships, `ccb build` loop traces become the first real data. When CCB-019 (patterns) ships, `ccb lesson` entries migrate into the pattern mining system. The loop that built the memory system becomes its first data source.

---

## Sprint 6: Context Authority

The roof. Wraps code graph, expert graph, memory, skills, tools, MCP servers, and CLAUDE.md into a single weighted knowledge base. Replaces bulk prompt injection with focused retrieval. Self-tunes via session data. Detects and fills knowledge gaps.

**Built using:** `ccb plan` + `ccb build` from Sprint 0, with memory traces feeding back.

| Story | Title | Priority | ACs | Est. Sessions | Depends On |
|-------|-------|----------|-----|---------------|------------|
| CCB-024 | Context Authority — Unified Knowledge Index | P1 | 29 | 3 | CCB-015, CCB-018 |
| CCB-025 | Context Authority — Hook Interception & Retrieval | P1 | 28 | 2-3 | CCB-024, CCB-020 |
| CCB-026 | Context Authority — Weight Feedback & Gap Detection | P1 | 25 | 2-3 | CCB-025, CCB-019, CCB-021 |
| CCB-027 | Context Authority — Dashboard UI | P2 | 30 | 2 | CCB-024, CCB-025, CCB-026 |

**Sprint 6 deliverables:**
- `context_nodes` + `context_edges` tables in graph.db — unified knowledge graph
- `ccb context sync|query|stats|set-weight|weights` subcommands
- Hook interception: SessionStart (focused injection), PreToolUse (tool-specific context), PostToolUse (trace logging)
- Two-tier injection: always-on core (~500 tokens) + weight-driven retrieval (~0-800 tokens)
- `ccb context tune` — automatic weight updates from session traces
- LoCoMo validation gate: weight changes that drop retention are auto-rolled back
- `ccb context gaps` — detects unused-but-important knowledge, suggests skills/experts to fill
- `ccb context report` — weight distribution, trends, gap summary
- CLAUDE.md decomposition: section-level retrieval instead of full-file injection
- TUI dashboard (`ccb context dash`) — terminal view of what's indexed, injected, missing, costing
- Web dashboard (`ccb context serve`) — local browser UI with knowledge graph, weight explorer, token treemap, gap report

**Sprint 6 new feature flags:** `context` (deps: builds on `graph` + `memory`), `dashboard` (opt-in, not in `full`)

**Sprint 6 risk:** CLAUDE.md decomposition quality. Code symbols have clean structure (tree-sitter AST). Documentation is prose with embedded rules, cross-references, and context-dependent applicability. Section-level chunking at `##` boundaries is the simplest approach — semantic chunking is future work if needed.

**Sprint 6 validation:** Every weight change runs through LoCoMo (CCB-021). Retention score is the gate. If retrieval loses knowledge, the data proves it and the change rolls back.

**Sprint 6 closes the loop:** The context authority makes `ccb build` smarter — it provides the right expert at each pipeline stage automatically, based on file context and domain detection. The loop that was built manually in Sprint 0 becomes context-aware in Sprint 6.

---

## Execution Order

```
Sprint 0 (Loop — the construction method):
  CCB-028 (plan→build→verify)
  ↓ used to build everything below

Sprint 4 (Graph — data source 1):
  CCB-015 (edges)  ──→  CCB-016 (traversal)  ──→  CCB-017 (watch)
                                                        ↑ can defer

Sprint 5 (Memory + Benchmark — data source 2 + validation):
  CCB-018 (traces)  ──→  CCB-019 (patterns)  ──→  CCB-020 (search)
  CCB-021 (LoCoMo)  ──→  (build early, prerequisite for Sprint 6)

Sprint 6 (Context Authority — the roof):
  CCB-024 (index)  ──→  CCB-025 (hooks)  ──→  CCB-026 (feedback)  ──→  CCB-027 (dashboard, P2)
       ↑                     ↑                      ↑
    needs 015,018         needs 024,020          needs 025,019,021
```

### Cross-Sprint Dependencies

```
028 (loop) ───────────────────────────────────→ ALL (used to build every story)
015 (edges) ──────────────────────────────────→ 024 (context index needs code data)
018 (traces) ─────────────────────────────────→ 024 (context index needs trace schema)
019 (patterns) ───────────────────────────────→ 026 (gap detection uses mined patterns)
020 (search) ─────────────────────────────────→ 025 (hook retrieval uses hybrid search)
021 (LoCoMo) ─────────────────────────────────→ 026 (validation gate for weight tuning)
```

## Cargo.toml Changes

```toml
[features]
loop    = []
memory  = ["dep:rusqlite"]
context = ["graph", "memory"]
full    = ["trim", "fade", "sandbox", "terse", "graph", "route",
           "expert", "classify", "factory", "loop", "memory", "context"]

[dependencies]
notify  = { version = "6", optional = true }  # graph watch
```

## Schema Summary

```
graph.db (existing, extended across sprints 4+6):
  files           — existing
  symbols         — existing
  edges           — NEW (CCB-015)
  personas        — existing
  domains         — existing
  persona_domains — existing
  patterns        — existing
  active_persona  — existing
  context_nodes   — NEW (CCB-024)
  context_edges   — NEW (CCB-024)

traces.db (new, Sprint 5):
  sessions        — NEW (CCB-018)
  trace_events    — NEW (CCB-018)
  mined_patterns  — NEW (CCB-019)
  trace_events_fts — NEW FTS5 virtual table (CCB-020)

Loop artifacts (Sprint 0):
  ~/.cache/ccb/failures/<repo>/<story>_<timestamp>.md
  ~/.cache/ccb/lessons/<repo>/<slug>.md
  .ccb/plans/<story-slug>.json

Weight history (append-only log):
  ~/.cache/ccb/weight_history.jsonl — NEW (CCB-026)
```

## Success Criteria

### Sprint 0
1. `ccb plan` on CCB-015 story produces a valid phased plan with correct Rust gates
2. `ccb gates --run` passes on current CCB main branch
3. `ccb build` executes a single-phase change, gates pass, commits to feature branch
4. After intentional gate failure: failure context saved, retry loads it, 3-strike stops the loop

### Sprint 4
5. `ccb graph callers main` on CCB's own codebase returns the correct call sites
6. `ccb graph dead` finds genuinely unreachable functions

### Sprint 5
7. `ccb memory mine` generates at least one skill from 5+ sessions of real use
8. `ccb memory recall` produces a ≤500-token context block relevant to the current project
9. `ccb gain --locomo` shows trim preserves ≥90% QA accuracy while saving ≥25% tokens

### Sprint 6
10. `ccb context inject --hook session-start` returns <1000 tokens that cover the same ground as the current ~5000 token CLAUDE.md injection
11. `ccb context tune --validate` accepts weight changes without LoCoMo regression
12. `ccb context gaps` identifies at least one real blind spot in a 20+ session history
13. Net token reduction across 10 sessions: ≥40% fewer context tokens per turn with ≥95% LoCoMo retention

---

## References

| Project | What it contributes | CCB stories |
|---------|-------------------|-------------|
| [CodeGraphContext](https://github.com/CodeGraphContext/CodeGraphContext) | 20-lang tree-sitter, caller/callee edges, dead code, complexity | CCB-015, 016 |
| [Hivemind](https://github.com/activeloopai/hivemind) | Session traces → pattern mining → skill generation, hybrid search | CCB-018, 019, 020 |
| [LoCoMo](https://arxiv.org/abs/2402.17753) | Long-term memory benchmark, retention measurement | CCB-021 |
| [agentic-loop/RALPH](https://github.com/allierays/agentic-loop) | Autonomous code→test→commit loop, failure persistence, /lesson | CCB-028 |
| [BMAD-METHOD](https://github.com/bmad-code-org/BMAD-METHOD) | 12+ persona pipeline, stage-based development, scale-adaptive | CCB-028 pipeline design |
| [Agent-Spec](https://github.com/RaySmith414/Agent-Spec) | Traceability (tests→reqs→stories), typed contracts between stages | CCB-028 validation |
| [OpenSpec](https://github.com/Fission-AI/OpenSpec) | Artifact structure (proposal→spec→design→tasks), iterative SDD | CCB-028 plan output |
| [Nango](https://github.com/NangoHQ/nango) | Unified auth for 800+ APIs, OAuth lifecycle management | Future: MCP/plugin auth |

---

## The Full Cycle (Post-Sprint 6)

```
                    ┌─────────────────────────────────┐
                    │       CONTEXT AUTHORITY          │
                    │    (the Karpathy knowledge base) │
                    │                                  │
                    │  ┌──────┐ ┌───────┐ ┌────────┐  │
                    │  │ Code │ │Expert │ │Memory  │  │
                    │  │Graph │ │Graph  │ │Traces  │  │
                    │  └──┬───┘ └──┬────┘ └───┬────┘  │
                    │     │       │          │        │
                    │  ┌──┴───────┴──────────┴─────┐  │
                    │  │  context_nodes + edges     │  │
                    │  │  + skills + tools + docs   │  │
                    │  └────────────┬───────────────┘  │
                    │              │                   │
                    │  ┌───────────┴────────────────┐  │
                    │  │   Weighted Retrieval        │  │
                    │  │   per turn, per hook        │  │
                    │  └───────────┬────────────────┘  │
                    │              │                   │
                    │  ┌───────────┴────────────────┐  │
                    │  │   LoCoMo Validation        │  │
                    │  │   proves weight changes    │  │
                    │  └───────────┬────────────────┘  │
                    │              │                   │
                    │  ┌───────────┴────────────────┐  │
                    │  │   Gap Detection            │  │
                    │  │   finds blind spots        │  │
                    │  │   suggests skills/experts  │  │
                    │  └───────────────────────────┘  │
                    └─────────────────────────────────┘

Development loop (using ccb plan + ccb build):
  1. ccb plan <story> → interview → sentinel → architect → plan.json
  2. ccb build → implement → gate → validate → commit → next phase
  3. Failures → persist context, retry with lessons
  4. All phases pass → push + PR
  5. Build traces → memory (018-020) → weight tuning (026)
  6. Repeat — the tool builds itself, each sprint better than the last
```
