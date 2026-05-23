# CCB Architecture

*May 21, 2026*

CCB is a three-layer system. Each layer builds on the one below it. All three layers share a single SQLite database.

---

## Layer 1 — Token Management

**Commands:** `trim`, `fade`, `context`, `cut`, `buzz`, `gain`, `style`

Sits between shell tooling and the LLM context window. Strips noise from build output, lazy-loads skills, monitors context pressure, logs savings.

---

## Layer 2 — Code Intelligence

**Commands:** `graph index`, `graph search`, `graph show`, `graph stats`

Tree-sitter parses the repo into a SQLite code graph. Functions, classes, calls, imports — indexed as nodes and edges. sqlite-vec stores embeddings for semantic search.

---

## Layer 3 — Expert System

**Commands:** `lineup`, `expert activate`, `expert deactivate`, `expert list`

### What an expert is

An expert is a **knowledge graph subset** built from a domain dataset and injected into the model's processing path via pre-tool hooks — in code, before inference, without entering the conversation context.

Not a prompt. Not a token. Not a LoRA. The model is programmatically required to use the knowledge.

### Knowledge graph construction

Each domain dataset is parsed into structured graph nodes with typed edges encoding relationships. Stored in CCB's SQLite DB alongside the code graph. Construction is one-time and offline.

```
expert_graphs/
├── sentinel/    nodes: OWASP rules, vuln patterns, auth contracts, CVEs
├── coder/       nodes: language idioms, reasoning patterns, test structures
├── architect/   nodes: AWS services, config patterns, infra troubleshooting
├── selector/    nodes: HF model capabilities, architecture types, benchmarks
└── debugger/    nodes: bug patterns, diagnosis paths, resolution templates
```

### Pre-tool hook injection

CCB registers pre-tool hooks in the inference pipeline. When a tool is about to execute:

1. Hook fires with task context
2. CCB traverses the active expert's knowledge graph
3. Relevant nodes are identified via embedding similarity
4. Those nodes are injected into the model's processing path **in code**
5. The model is required to use that knowledge before the tool executes

No tokens consumed. No conversation modification. The knowledge woven into the inference path, not the prompt.

### Self-pruning

The knowledge graph starts with all domain nodes populated. Nodes that never activate during the user's actual usage lose relevance weight over time. Eventually they are pruned.

A developer's graph converges toward coder/architect/sentinel. An Alchemy user's graph converges toward vintage/marketplace knowledge. Each user's graph becomes unique to their usage patterns — automatically, without configuration.

The pruning mechanism is disuse. No admin required.

---

## Design philosophy — emergent expertise

The knowledge graph does not get pruned by an algorithm deciding what is relevant. It gets shaped by the same force that shapes human expertise: directed use, feedback, and the reinforcement of what actually matters to this person doing this work.

A surgeon carries dense knowledge of anatomy and procedure. A musician carries dense knowledge of theory and timing. Neither decided to specialize — their knowledge topology emerged from what they did, what they encountered, and what they were corrected on.

The CCB knowledge graph works the same way. The user, their model interactions, and how they harness the model together determine which nodes survive and which fade. Two developers using Atlas for the same general purpose will end up with different graphs because they work differently and push the model in different directions.

The variation is not a side effect. It is the point.

**What guides the graph:**
- **User** — tasks assigned, domains touched, feedback given (good/bad ratings)
- **Model interactions** — which knowledge nodes led to outputs the user found useful
- **Harnessing** — the intentional direction the user applies to the model over time

The result is a knowledge graph that is uniquely theirs — an accurate reflection of their expertise, their domain, and their relationship with the model. Not a generic expert system. Not a one-size-fits-all knowledge base. A living graph that became what it is because of how they used it.

---

## The self-teaching feedback loop (Atlas / Alchemy)

This loop exists independently of CCB but CCB participates in it.

### Weekly weight averaging

1. User rates model output (good / bad) and other interaction signals are collected
2. Weight deltas from those interactions are tracked on-device
3. **Weekly:** weight deltas are averaged and applied to the local model
4. **Monthly (week 4):** the four weekly averages are averaged → stable monthly base update

### The privacy model

```
cloud:  base_weights only  (anonymous aggregate, never personal data)
local:  user_delta         (never uploaded, stays on device forever)

on base update:  user_model = new_base + user_delta
```

Only the aggregated, anonymized base weights ever leave the device. The user's personal adjustments — their delta — accumulates locally and is re-applied every time a new base model is distributed.

The knowledge graph follows the same model: base graph is distributed, user's pruned/weighted version stays local.

### What CCB adds to the loop

CCB's pre-tool hook injection means the model's weight changes occur under expert knowledge guidance. When a user's interaction is rated good, the weight pattern that led to that output is reinforced — and that pattern was shaped by the expert knowledge graph. Over time, the graph's most useful nodes become more deeply woven into the model's behavior.

The Karpathy self-learning knowledge tree: the graph is not a static lookup. It is a living tree whose nodes' relevance weights update from the model's own operation. The model and the graph co-evolve toward the user's actual domain.

---

## Distribution model

| Target | Org | License | Ships in |
|--------|-----|---------|---------|
| `ccb` (Rust) | thrtysxty | Apache-2.0 | Open source — proves the architecture, builds community |
| `ccb-swift` (Swift) | ByteReactr | Proprietary | Premium — Atlas, Alchemy, Apple platform products |

The Rust implementation is the public proof. The Swift implementation is the commercial product. Same architecture, same datasets, same knowledge graphs — platform and license are the only differences.

---

## Swift platform (Atlas / Apple)

| Target | Runtime | Inference | Platforms |
|--------|---------|-----------|-----------|
| `ccb` (Rust) | Linux / server | llama.cpp | 9020 (RTX 3060), CI |
| `ccb-swift` (Swift) | Apple silicon | Core ML + Foundation Models | macOS, iOS, Atlas, Alchemy |

### Atlas integration

CCB Swift is the intelligence module for Atlas. `AtlasCCB` shares one SQLite DB with the code graph layer. Pre-tool hooks integrate with Atlas's inference pipeline directly.

---

## Cross-product scope

| Product | Active expert graphs |
|---------|---------------------|
| Copernicus / Atlas | sentinel, coder, architect, debugger, selector |
| Alchemy | vintage, marketplace (built from filtered FineWeb + flat icons) |

Each user's graph self-prunes to their actual usage. A Copernicus developer and an Alchemy seller end up with completely different local graphs built on the same base.

---

## Datasets

Domain datasets are the **knowledge graph construction source** — not training data. Each dataset is parsed into graph nodes and edges stored in SQLite.

### Expert knowledge graphs

| Expert | Dataset | Rows | Domain |
|--------|---------|------|--------|
| `sentinel` | [Fenrir-v2.1](https://huggingface.co/datasets/AlicanKiraz0/Cybersecurity-Dataset-Fenrir-v2.1) | 99,870 | OWASP, MITRE ATT&CK, auth hardening, crypto hygiene |
| `coder` | [CodeX-2M-Thinking](https://huggingface.co/datasets/Modotte/CodeX-2M-Thinking) | 2M | Implementation + chain-of-thought reasoning |
| `architect` | [solutions-architect-hf](https://huggingface.co/datasets/VishaalY/solutions-architect-hf-dataset) | — | AWS architecture, service config, infra troubleshooting |
| `selector` | [hf_model_metadata](https://huggingface.co/datasets/davanstrien/hf_model_metadata) | — | HF model ecosystem — capabilities, architectures, benchmarks |
| `debugger` | [hf-issues-dataset-with-comments](https://huggingface.co/datasets/selfishark/hf-issues-dataset-with-comments) | 4,370 | Model bug triage and resolution patterns |
| `thinker` | [GLM-5.1-Reasoning-1M](https://huggingface.co/datasets/Jackrong/GLM-5.1-Reasoning-1M-Cleaned) | 1M | Deep reasoning — deferred |

### Alchemy knowledge graphs (deferred)

| Dataset | Use |
|---------|-----|
| [HuggingFaceFW/fineweb](https://huggingface.co/datasets/HuggingFaceFW/fineweb) | Filtered for marketplace/vintage content → listing copy graph |
| [andrewburns/hf_flat_icons](https://huggingface.co/datasets/andrewburns/hf_flat_icons) | Semantic icon tagging → visual scan graph |

---

## CCB database

`~/.cache/ccb/graph.db` — shared across all three layers.

```sql
-- Layer 2
code_nodes(id, file, symbol, kind, line)
code_edges(src, dst, edge_type)

-- Layer 3
expert_nodes(id, expert, content, embedding BLOB, relevance_weight REAL)
expert_edges(src, dst, relationship)
activation_log(ts, expert, node_id, outcome)
```

`relevance_weight` on each node is updated from the activation log. Low weight → pruning candidate. High weight → injected more frequently.

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
