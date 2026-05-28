# CCB Story 024: Context Authority — Unified Knowledge Index

**Status:** READY
**Priority:** P1 — architectural keystone
**Sprint:** CCB-6 (Context Authority)
**Feature flag:** `context`
**Depends on:** CCB-015 (edges), CCB-018 (traces)

## Narrative
**As a** Claude Code user,
**I want** all context sources (code, experts, skills, tools, docs, MCP servers) indexed in a single weighted knowledge graph,
**So that** CCB can retrieve focused, relevant context per turn instead of injecting everything into the prompt.

## Context

CCB currently has five disconnected context systems:
- **Code graph** (`graph.rs`): `files` + `symbols` tables, edges coming via CCB-015
- **Expert graph** (`expert.rs`): `personas` + `domains` + `patterns` tables
- **Skills** (`fade.rs`): flat `INDEX.md` file scan, loads raw markdown
- **Classify** (`classify.rs`): static pattern matching on PreToolUse, not wired
- **CLAUDE.md chain**: monolithic prose injected in full every turn (~3-5k tokens)

Each system stores data differently, has its own query path, and none of them share a weight model. The result: bulk injection, wasted tokens, no feedback loop.

The context authority wraps all of these into a unified knowledge graph with weighted nodes. Every indexable thing — a code symbol, an expert persona, a skill, a tool, a section of CLAUDE.md — becomes a node with a weight derived from actual usage data.

### The Karpathy Principle

**Current (injection model):** Shove everything into the prompt. The model sorts it out. Pay 5k+ tokens per turn for rules relevant to 1 in 20 turns.

**Target (retrieval model):** Index everything once. Query per turn for what's relevant. Inject a small structured JSON response. The model gets `{"expert": "sentinel", "relevant_rule": "verify before reporting", "related_symbols": ["validate_token"]}` instead of the full Contract.md.

### CLAUDE.md Decomposition

The biggest token savings come from decomposing monolithic .md files into section-addressable chunks. Each `##` heading becomes a discrete node the system can weight independently:

```
Contract.md → 6 nodes
  "honesty-commitments"      initial_weight: 0.95
  "verification-protocol"    initial_weight: 0.85
  "leadership-commitments"   initial_weight: 0.60
  "violation-record"         initial_weight: 0.35

git.md → N nodes
  "branch-workflow"          initial_weight: 0.88
  "gh-auth-switching"        initial_weight: 0.91
  "worktree-management"      initial_weight: 0.15
```

Initial weights are manually assigned based on historical relevance. The weight feedback loop (CCB-026) makes them adaptive.

## Architecture

```
graph.db schema additions:

  context_nodes (
      id          INTEGER PRIMARY KEY,
      kind        TEXT NOT NULL,       -- 'symbol' | 'skill' | 'expert' | 'tool'
                                       -- | 'mcp_server' | 'doc_section' | 'domain'
      name        TEXT NOT NULL,
      source_ref  TEXT,                -- FK hint: 'symbols:42' | 'personas:3'
                                       --   | 'skills/pr-workflow.md' | 'Contract.md##honesty'
      weight      REAL NOT NULL DEFAULT 0.5,
      metadata    TEXT,                -- JSON: kind-specific attributes
      updated     INTEGER NOT NULL     -- unix timestamp of last weight update
  )

  context_edges (
      id          INTEGER PRIMARY KEY,
      source_id   INTEGER NOT NULL REFERENCES context_nodes(id) ON DELETE CASCADE,
      target_id   INTEGER NOT NULL REFERENCES context_nodes(id) ON DELETE CASCADE,
      kind        TEXT NOT NULL,       -- 'covers' | 'applies_to' | 'activates'
                                       -- | 'provides' | 'requires'
      weight      REAL NOT NULL DEFAULT 1.0
  )

  Indexes:
    idx_ctx_nodes_kind    ON context_nodes(kind)
    idx_ctx_nodes_weight  ON context_nodes(weight DESC)
    idx_ctx_nodes_name    ON context_nodes(name)
    idx_ctx_edges_source  ON context_edges(source_id)
    idx_ctx_edges_target  ON context_edges(target_id)
```

### Node Population Sources

| Source | Kind | How Populated |
|--------|------|---------------|
| `symbols` table | `symbol` | Sync after `ccb graph index` — one context_node per symbol |
| `personas` table | `expert` | Sync after `ccb expert build/ingest` |
| `domains` table | `domain` | Sync after expert build |
| `~/.claude/skills/*.md` | `skill` | File scan of skills directory |
| Plugin tool definitions | `tool` | Parse `~/.claude/plugins/` registry |
| MCP server configs | `mcp_server` | Parse `~/.claude/.mcp.json` |
| CLAUDE.md sections | `doc_section` | Heading-level decomposition of .md chain |

### Cross-Domain Edge Examples

```
expert:sentinel  ──covers──→    domain:security
domain:security  ──applies_to──→ symbol:validate_token
skill:pr-workflow ──activates──→ tool:Bash(git)
doc:gh-auth-switching ──requires──→ tool:Bash(gh)
mcp:github       ──provides──→  domain:version-control
```

## Acceptance Criteria

### Schema & Core
- [ ] **AC1:** `context_nodes` table created in graph.db schema migration with all columns
- [ ] **AC2:** `context_edges` table created with foreign keys and ON DELETE CASCADE
- [ ] **AC3:** All indexes created for fast weight-ordered retrieval and edge traversal
- [ ] **AC4:** `ccb context` module added behind `#[cfg(feature = "context")]` feature flag

### Indexing — Code
- [ ] **AC5:** `ccb context sync` populates context_nodes from `symbols` table (kind=symbol, source_ref=symbols:{id})
- [ ] **AC6:** Sync is idempotent — re-running doesn't duplicate nodes

### Indexing — Experts & Domains
- [ ] **AC7:** `ccb context sync` populates context_nodes from `personas` and `domains` tables
- [ ] **AC8:** `persona_domains` relationships mapped to context_edges (expert→domain, kind=covers)

### Indexing — Skills
- [ ] **AC9:** `ccb context sync` scans `~/.claude/skills/` directory, creates one node per .md file
- [ ] **AC10:** Skill metadata includes: filename, size, tags (parsed from INDEX.md if present)

### Indexing — Tools & MCP Servers
- [ ] **AC11:** `ccb context sync` parses `~/.claude/plugins/` to discover installed plugin tools
- [ ] **AC12:** `ccb context sync` parses `~/.claude/.mcp.json` to discover MCP server configs
- [ ] **AC13:** Each tool/server becomes a context_node with connection status in metadata

### Indexing — Documentation
- [ ] **AC14:** `ccb context sync` decomposes CLAUDE.md chain into section nodes at `##` heading boundaries
- [ ] **AC15:** Each doc_section node stores: source file, heading text, content hash, initial weight
- [ ] **AC16:** `@include` references (e.g. `@Contract.md`) are followed and decomposed recursively
- [ ] **AC17:** Section content is NOT stored in the node — only the heading, source path, and byte offset for retrieval

### Weight Model
- [ ] **AC18:** All nodes created with initial weight (configurable default: 0.5)
- [ ] **AC19:** `ccb context set-weight <node-id> <weight>` for manual weight override
- [ ] **AC20:** `ccb context weights [--kind <kind>]` lists nodes sorted by weight descending

### Query
- [ ] **AC21:** `ccb context query <topic>` returns top-K nodes by weight × name relevance, JSON output
- [ ] **AC22:** Query supports `--kind` filter (e.g. `--kind expert,skill` to only search those types)
- [ ] **AC23:** Query supports `--top N` (default 10) to control result count

### CLI & Output
- [ ] **AC24:** `ccb context stats` shows node counts by kind, edge counts by kind, weight distribution
- [ ] **AC25:** All commands support `--format human|json`
- [ ] **AC26:** `context` feature flag added to Cargo.toml, included in `full` feature set

### Tests
- [ ] **AC27:** Unit tests: sync from each source type (code, expert, skill, tool, doc)
- [ ] **AC28:** Unit tests: query with weight ordering, kind filtering, top-K limiting
- [ ] **AC29:** Integration test: full sync → query cycle on a fixture project

## Notes

- `context_nodes.source_ref` is a soft reference, not a foreign key — the underlying tables may have different schemas and IDs
- Doc section content is retrieved on demand from the source file using byte offset, not stored in the node — keeps the table lean
- Initial weights are best-guess. The feedback loop (CCB-026) replaces guesswork with data
- The `sync` command should be safe to run repeatedly and fast enough to run at session start
- Plugin tool discovery reads the plugin cache directory structure, not the Claude Code process — it's a file scan
- Consider: should `ccb context sync` run automatically as part of `ccb graph index`? Probably yes, but keep them separable for testing
