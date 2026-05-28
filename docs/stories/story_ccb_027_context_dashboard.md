# CCB Story 027: Context Authority — Dashboard UI

**Status:** READY
**Priority:** P2 — observability, nice-to-have but high value
**Sprint:** CCB-6 (Context Authority) or standalone
**Feature flag:** `dashboard`
**Depends on:** CCB-024 (index), CCB-025 (hooks), CCB-026 (feedback)

## Narrative
**As a** CCB user,
**I want** a visual dashboard showing what the context authority is managing, injecting, missing, and costing,
**So that** I can see the system working (or not), spot inflated context, and understand what knowledge exists vs. what's absent.

## Context

The context authority (024-026) makes smart decisions about what to inject. But without visibility, the user has to trust those decisions blindly. A dashboard answers:

- **What's in the graph?** — how many nodes of each kind, what's indexed, what's missing
- **What's being injected?** — which nodes made it into the last N turns, at what weight
- **What's costing tokens?** — which nodes are the biggest, which have the worst weight-to-size ratio
- **What's drifting?** — weight changes over time, which nodes are climbing or falling
- **What's missing?** — gaps detected by CCB-026, domains with no coverage
- **Is it working?** — LoCoMo retention trend, tokens saved per session

### Two Modes

**TUI mode** (`ccb context dash`): Terminal dashboard via ratatui. Quick glance without leaving the terminal. Good for checking state mid-session.

**Web mode** (`ccb context serve`): Local web server on a random port. Richer visualization — knowledge graph as an interactive node map, weight history as time-series charts, token breakdown as treemaps. Opens in browser alongside Claude Code.

Both read from the same data source (graph.db + traces.db + weight_history.jsonl). The TUI is the quick check, the web UI is the deep dive.

## Architecture

### Data Sources (all read-only)

```
graph.db:
  context_nodes  → node inventory, weights, kinds
  context_edges  → relationship map
  symbols        → code symbol counts
  personas       → expert inventory
  
traces.db:
  sessions       → session history
  trace_events   → injection history per turn

~/.cache/ccb/weight_history.jsonl → weight trends over time
~/.claude/skills/                 → skill file inventory
~/.claude/plugins/                → plugin/tool inventory
```

### TUI Layout (ratatui)

```
┌─ CCB Context Dashboard ──────────────────────────────────────┐
│                                                               │
│  ┌─ Knowledge Graph ────────┐  ┌─ Injection (last 10) ─────┐ │
│  │ Symbols    1,247  ██████ │  │ Turn 1: 780 tok (3 nodes)  │ │
│  │ Experts        5  █      │  │ Turn 2: 620 tok (2 nodes)  │ │
│  │ Skills        12  ██     │  │ Turn 3: 890 tok (4 nodes)  │ │
│  │ Tools         23  ███    │  │ Turn 4: 510 tok (2 nodes)  │ │
│  │ MCP Servers    4  █      │  │ ...                        │ │
│  │ Doc Sections  38  ████   │  │ Avg: 700 tok/turn          │ │
│  │ Domains       11  ██     │  │ Saved: ~3,800 tok/turn     │ │
│  │ ─────────────────────    │  │                            │ │
│  │ Total: 1,340 nodes       │  │ Budget: 1000 tok           │ │
│  │ Edges: 2,180             │  │ Tier 1: 480 tok (fixed)    │ │
│  └──────────────────────────┘  └────────────────────────────┘ │
│                                                               │
│  ┌─ Weight Distribution ────┐  ┌─ Gaps & Alerts ────────────┐ │
│  │ 0.9-1.0: ████████  42    │  │ ⚠ sentinel never activated │ │
│  │ 0.7-0.9: ██████    31    │  │ ⚠ no coverage: domain:api  │ │
│  │ 0.5-0.7: ████      24    │  │ ⚠ git.md#worktree weight   │ │
│  │ 0.3-0.5: ███       18    │  │   dropped 0.8→0.1 (verify) │ │
│  │ 0.1-0.3: ██        12    │  │                            │ │
│  │ <0.1:    █          8    │  │ LoCoMo: 94.2% retention    │ │
│  │                          │  │ Trend: ↑ +0.3% last tune   │ │
│  └──────────────────────────┘  └────────────────────────────┘ │
│                                                               │
│  [q]uit  [r]efresh  [w]eights  [g]aps  [n]odes  [s]essions   │
└───────────────────────────────────────────────────────────────┘
```

### Web UI Layout

Single HTML page served from a local Rust HTTP server. No JS framework — vanilla HTML + lightweight chart library (Chart.js or uPlot, bundled as static asset).

**Views:**

1. **Overview** — node counts by kind, edge counts, token savings, LoCoMo score
2. **Knowledge Graph** — interactive node map (d3-force or similar). Nodes colored by kind, sized by weight. Edges show relationships. Click a node to see details + weight history.
3. **Injection Log** — table of recent turns: timestamp, nodes injected, tokens used, tool that triggered it
4. **Weight Explorer** — sortable table of all nodes with weight, kind, source, last updated. Filter by kind. Click for weight-over-time sparkline.
5. **Gap Report** — list of detected gaps with evidence and suggestions
6. **Token Accounting** — treemap showing token usage by kind. "What's eating my context?" at a glance.

### Server

```
ccb context serve [--port PORT]
  → starts HTTP server on localhost:PORT (default: random high port)
  → opens browser automatically
  → serves static HTML + JSON API endpoints
  → reads graph.db + traces.db (read-only, no mutations)
  → shuts down on Ctrl+C
```

API endpoints (JSON):
```
GET /api/nodes          → all context_nodes with weights
GET /api/nodes/:id      → single node + edges + weight history
GET /api/edges          → all context_edges
GET /api/injection-log  → recent injection events from traces
GET /api/weights        → weight distribution summary
GET /api/gaps           → current gap report
GET /api/stats          → overview stats (counts, savings, LoCoMo)
```

## Acceptance Criteria

### TUI Dashboard
- [ ] **AC1:** `ccb context dash` renders a ratatui terminal dashboard
- [ ] **AC2:** Knowledge graph panel: node counts by kind with bar visualization
- [ ] **AC3:** Injection panel: last N injection events with token counts
- [ ] **AC4:** Weight distribution panel: histogram of weights across all nodes
- [ ] **AC5:** Gaps panel: active gap alerts from CCB-026 gap detection
- [ ] **AC6:** LoCoMo retention score displayed with trend indicator (↑/↓/→)
- [ ] **AC7:** Token savings displayed: average tokens/turn vs. baseline (full CLAUDE.md injection)
- [ ] **AC8:** Keyboard navigation: [q]uit, [r]efresh, tab between panels
- [ ] **AC9:** Auto-refresh on configurable interval (default 5s) or manual [r]efresh

### Web Dashboard
- [ ] **AC10:** `ccb context serve` starts a local HTTP server, prints URL to stdout
- [ ] **AC11:** `--port <PORT>` flag for explicit port selection (default: random available)
- [ ] **AC12:** Single HTML page with all views (tabs or scroll sections)
- [ ] **AC13:** Overview panel: node/edge counts, token savings, LoCoMo score
- [ ] **AC14:** Injection log table: sortable, filterable by hook type and node kind
- [ ] **AC15:** Weight explorer: sortable table of all nodes, click for weight history sparkline
- [ ] **AC16:** Gap report view: gap list with evidence and suggestion buttons
- [ ] **AC17:** Token accounting treemap: visual breakdown of token usage by kind
- [ ] **AC18:** JSON API: `/api/nodes`, `/api/edges`, `/api/injection-log`, `/api/weights`, `/api/gaps`, `/api/stats`
- [ ] **AC19:** All data read-only — dashboard never mutates graph.db or traces.db
- [ ] **AC20:** Server shuts down cleanly on Ctrl+C

### Knowledge Graph Visualization (web, stretch goal)
- [ ] **AC21:** Interactive node-link diagram: nodes colored by kind, sized by weight
- [ ] **AC22:** Click node to see: name, kind, weight, source_ref, connected edges, weight history
- [ ] **AC23:** Filter by node kind (toggle symbol/expert/skill/tool/doc checkboxes)
- [ ] **AC24:** Search box to find nodes by name

### Integration
- [ ] **AC25:** `dashboard` feature flag added to Cargo.toml (deps: `ratatui`, `crossterm`, `axum` or `tiny_http`)
- [ ] **AC26:** `dashboard` NOT included in `full` feature set — opt-in only (keeps binary small)
- [ ] **AC27:** Web UI static assets (HTML, CSS, JS) embedded in binary via `include_str!` or `rust-embed`

### Tests
- [ ] **AC28:** Unit tests: API endpoint JSON output matches expected schema
- [ ] **AC29:** Unit tests: TUI rendering doesn't panic on empty database
- [ ] **AC30:** Integration test: serve → fetch /api/stats → verify response

## Notes

- The TUI is the "quick check" and the web UI is the "deep dive." Both are useful but if only one ships first, the TUI is more aligned with CCB's CLI nature.
- The web UI should be dead simple — one HTML file with embedded CSS/JS, no build step, no npm. Think single-page report, not an app.
- Knowledge graph visualization (AC21-24) is a stretch goal. The rest of the web UI delivers value without it. d3-force is heavy; a simple table view may be enough for v1.
- `dashboard` is opt-in (not in `full`) because ratatui + axum are non-trivial deps. Users who don't want the dashboard shouldn't pay the compile cost.
- The injection log is the most immediately useful view — "what did the context authority actually do on the last 10 turns?" answers whether the system is working.
- Token accounting treemap answers the user's exact question: "what is inflating my context?" — the biggest box is the biggest cost.
- Consider: should `ccb status` (the statusline) pull summary data from the context authority? E.g. show "ctx: 780/1000 tok" in the statusline alongside model and cost info.
