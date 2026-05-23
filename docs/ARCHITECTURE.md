# CCB Architecture

*May 22, 2026*

CCB is a three-layer system. Each layer builds on the one below it. All three layers share a single SQLite database.

---

## The core problem

Models have tools. They don't have knowledge.

A model can read a file, write code, run a test. It has hands. What it lacks is the structured understanding that tells it *why* this line is wrong, *what* pattern it should follow, *how* this function relates to the three others it will break. It acts on syntax. It needs to reason on semantics.

The Scarecrow's problem. Plenty of capability. No brain.

CCB is the brain. Not injected into context — wired into the reasoning path before any tool fires. The model doesn't *read* the knowledge, it *has* it, the way a senior engineer doesn't look up why you validate input before a DB write. They just know.

---

## One graph

CCB maintains a single unified knowledge graph. Not separate expert graphs, not a skills index, not a tools registry — one graph with typed nodes and typed edges.

Every kind of knowledge is a node:

| NodeKind | Examples |
|----------|---------|
| `CodeSymbol` | `stories.py:route_create`, `terminal-handlers.ts:manage_container` |
| `Skill` | `flask-route-testing`, `tdd-pattern`, `read-then-write` |
| `Tool` | `pytest`, `cargo`, `tsc`, `npx` |
| `MCP` | `coverage`, `playwright`, `filesystem` |
| `Persona` | `backend-developer`, `security-reviewer`, `ios-engineer` |
| `Domain` | `owasp-a03-injection`, `sql-parameterization`, `jwt-auth` |
| `Pattern` | `validate-before-insert`, `fail-fast-config`, `typed-ipc` |

Every relationship is a typed edge:

```
REQUIRES, USES, SERVES, PREFERS, VIOLATES, IMPLEMENTS,
INFORMED_BY, RELATED_TO, BLOCKS, FIXES, AUTHORED_BY
```

The edge types encode semantics. The graph topology encodes understanding.

---

## The repo is the root

Layer 2 tree-sitter indexing feeds directly into the unified graph. Every symbol, every call site, every import from every repo is a node. Typed edges connect code to knowledge:

```
stories.py:route_create
  ├── [CALLS] db.execute()
  │     └── [IMPLEMENTS] sql-parameterization
  │           └── [SHOULD_FOLLOW] domain:owasp-a03-injection
  │                 └── [FIX_SKETCH] pattern:validate-before-insert
  ├── [IN_FILE] stories.py
  │     └── [BLUEPRINT] flask:stories_bp
  │           └── [SKILL] flask-route-testing
  └── [AUTHORED_BY] persona:backend-developer
        └── [PREFERS] pytest + explicit assertions
```

OWASP rule A03 isn't abstract. It's attached to `stories.py:23`. The domain knowledge is specific to *this codebase*, *this line*, *this developer*.

---

## Traversal and threshold

When a tool is about to fire, CCB walks the graph from the current task context:

1. Identify the root nodes — current file, current symbol, task description
2. Traverse typed edges outward, accumulating node relevance scores
3. Apply threshold — nodes above threshold are activated, below are dormant
4. Activated nodes are injected into the model's reasoning path **before** the tool executes

The model doesn't receive a file. It doesn't receive a prompt. It receives a small, precise subgraph of everything relevant to the decision it's about to make.

Token cost: near zero. Reasoning quality: shaped by the full accumulated knowledge of the graph.

---

## Context limitation as the design constraint

The graph solves context pressure by design. The question is never "what do I pre-load?" — it's "what does this task walk to?" The threshold is the dial. Tighten it and the model gets only the highest-confidence knowledge. Loosen it and it gets broader context. The user never manages this manually.

`lineup` becomes a live view of what the graph activated for the current session — not a static inventory of what's loaded, but a real-time readout of what the brain surfaced.

---

## Self-pruning

The graph starts fully populated. Every node has a `relevance_weight`. When a node activates and the outcome is rated good, its weight rises. When it never activates, its weight decays.

The pruning mechanism is disuse. No algorithm decides what's relevant. The graph converges toward what this user, doing this work, in this codebase, actually needs — automatically, without configuration.

Two developers on the same codebase end up with different graphs. A backend developer's graph is thick with Flask patterns, pytest conventions, and SQL rules. An iOS developer's graph is thick with SwiftUI patterns, Keychain usage, and Core ML integration. Neither configured this. It emerged from use.

The variation is not a side effect. It is the point.

---

## Layer 1 — Token Management

**Commands:** `trim`, `fade`, `context`, `cut`, `buzz`, `gain`, `style`, `lineup`

Sits between shell tooling and the LLM context window. Strips noise from build output, lazy-loads skills, monitors context pressure, logs savings.

`fade` is the manual predecessor to graph-driven injection. It works today. The graph replaces it at Layer 3.

---

## Layer 2 — Code Intelligence

**Commands:** `graph index`, `graph search`, `graph show`, `graph stats`

Tree-sitter parses the repo into the unified graph. Functions, classes, calls, imports — indexed as typed `CodeSymbol` nodes. sqlite-vec stores embeddings for semantic traversal. This layer feeds the repo-rooted subgraph that Layer 3 traverses from.

---

## Layer 3 — Knowledge Graph

**Commands:** `expert activate`, `expert deactivate`, `expert list`, `graph walk`

The unified graph. All knowledge types, all repos, all personas — one structure. Pre-tool hooks traverse it before every model action.

### Knowledge sources

| NodeKind | Source |
|----------|--------|
| `CodeSymbol` | tree-sitter repo index (Layer 2) |
| `Skill` | `~/.claude/skills/` parsed into nodes |
| `Tool` | tool registry + usage history |
| `MCP` | MCP server manifests |
| `Persona` | user-defined + inferred from usage |
| `Domain` | dataset ingest (see below) |
| `Pattern` | extracted from Domain + CodeSymbol edges |

### Domain datasets

| Domain | Dataset | Rows |
|--------|---------|------|
| `sentinel` | [Fenrir-v2.1](https://huggingface.co/datasets/AlicanKiraz0/Cybersecurity-Dataset-Fenrir-v2.1) | 99,870 |
| `coder` | [CodeX-2M-Thinking](https://huggingface.co/datasets/Modotte/CodeX-2M-Thinking) | 2M |
| `architect` | [solutions-architect-hf](https://huggingface.co/datasets/VishaalY/solutions-architect-hf-dataset) | — |
| `selector` | [hf_model_metadata](https://huggingface.co/datasets/davanstrien/hf_model_metadata) | — |
| `debugger` | [hf-issues-dataset-with-comments](https://huggingface.co/datasets/selfishark/hf-issues-dataset-with-comments) | 4,370 |

Datasets are not training data. They are parsed into graph nodes and edges. Construction is one-time and offline.

---

## The feedback loop (Atlas / Alchemy)

The graph participates in Atlas's self-teaching loop.

```
user rates output (good / bad)
  → activation_log records which nodes were active
  → good outcome: relevance_weight rises on active nodes
  → bad outcome: relevance_weight falls
  → weekly: weight deltas averaged → local model update
  → monthly: four weekly averages → stable base update
```

Privacy model:
```
cloud:  base_weights only  (anonymous aggregate, never personal data)
local:  user_delta + graph  (never uploaded, stays on device forever)
```

The model and the graph co-evolve. Knowledge nodes that consistently lead to good outcomes become more deeply woven into the model's behavior. The graph is not a static lookup — it is a living structure whose topology reflects this user's expertise.

---

## Upgrade the brain, not the model

The key architectural property: you can give a small local model frontier-quality reasoning on a specific domain by giving it the right graph — without retraining, without a larger model, without more tokens.

qwopus (9B) plus this graph, for *your* codebase, knows your security rules, your patterns, your failure modes, your conventions. It reasons well on your work because it has the knowledge your work requires. The graph is the differentiator. The model is the executor.

This is why the Rust implementation ships open source. The architecture is the proof. Any model, any platform, same graph — same result.

---

## Distribution

| Target | Org | License | Platforms |
|--------|-----|---------|-----------|
| `ccb` (Rust) | thrtysxty | Apache-2.0 | Linux, server, CI — proves the architecture |
| `ccb-swift` (Swift) | ByteReactr | Proprietary | macOS, iOS — Atlas, Alchemy |

Same graph schema. Same datasets. Same traversal logic. Platform and license are the only differences.

---

## Database schema

`~/.cache/ccb/graph.db` — shared across all three layers.

```sql
-- Layer 2: code graph
code_nodes(id, file, symbol, kind, line)
code_edges(src, dst, edge_type)

-- Layer 3: unified knowledge graph
nodes(id, kind, content, source, embedding BLOB, relevance_weight REAL)
edges(src, dst, edge_type, weight REAL)
activation_log(ts, node_id, task_context, outcome)

-- Layer 3: persona
personas(id, name, description)
persona_edges(persona_id, node_id, affinity REAL)
```

`relevance_weight` decays on disuse. `activation_log` is the feedback source. `persona_edges` encode per-persona node affinity — the graph knows which nodes matter to which user profile.

---

## Build feature flags

```toml
[features]
default = ["trim", "fade"]
trim    = []
fade    = []
graph   = ["dep:tree-sitter", "dep:rusqlite", "dep:walkdir", "dep:ignore"]
expert  = ["dep:rusqlite", "dep:sqlite-vec", "dep:hf-hub", "graph"]
full    = ["trim", "fade", "graph", "expert"]
```
