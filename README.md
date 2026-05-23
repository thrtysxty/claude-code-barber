<div align="center">

# Claude Code Barber

<h1>💈</h1>

<em>"Just take a little off the top." — Claude Code, probably</em>

<p>
  <a href="https://github.com/thrtysxty/claude-code-barber/actions"><img src="https://github.com/thrtysxty/claude-code-barber/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-2021-orange.svg" alt="Rust 2021"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
</p>

</div>

> Your AI's context, styled.

**ccb** is a composable token management layer for Claude Code. It compresses noisy command output, lazy-loads skills on demand, monitors your context window, and logs token savings — built as a single Rust binary with optional feature flags so you only ship what you need.

---

## Table of Contents

- [About](#about)
- [Getting Started](#getting-started)
- [Commands](#commands)
- [Usage](#usage)
- [Hook Integration](#hook-integration)
- [Optional Features](#optional-features)
- [Benchmarks](#benchmarks)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)

---

## About

Claude Code dumps a lot of noise into its context window — `Compiling` lines, npm warnings, pytest headers, git hints. Every token of noise is a token not spent on reasoning.

ccb sits between your shell and Claude Code. It filters, compresses, and monitors so the model sees signal instead of static.

### Built With

- [Rust 2021](https://www.rust-lang.org) — single binary, no runtime
- [clap](https://github.com/clap-rs/clap) — CLI
- [rusqlite](https://github.com/rusqlite/rusqlite) — code graph and knowledge graph (optional features)
- [tree-sitter](https://tree-sitter.github.io) — symbol indexing (optional)
- [sqlite-vec](https://github.com/asg017/sqlite-vec) — embedding search (optional)

---

## Getting Started

### Prerequisites

- Rust toolchain ([install](https://rustup.rs))

### Installation

```bash
git clone https://github.com/thrtysxty/claude-code-barber
cd claude-code-barber
cargo build --release
cp target/release/ccb ~/.local/bin/
```

### Build options

```bash
# Default — trim + fade enabled
cargo build --release

# All features
cargo build --release --features full

# Minimal — context awareness and analytics only
cargo build --release --no-default-features
```

---

## Commands

| Command | What it does |
|---------|-------------|
| `ccb trim <cmd>` | Compress noisy output (git, pytest, tsc) before it hits the context window |
| `ccb fade [skill]` | Lazy-load a skill from `INDEX.md` — or list the index |
| `ccb context show` | Display current context window usage with progress bar |
| `ccb context clear [threshold]` | Warn when context exceeds threshold (default 80%) |
| `ccb context compact [threshold]` | Warn when context exceeds threshold (default 60%) |
| `ccb lineup` | Show what is loaded in the context budget with token estimates |
| `ccb cut` | Run context check + lineup in one shot |
| `ccb buzz` | Nuclear: strip INDEX.md overhead, prune log to last 500 events |
| `ccb gain` | Token savings analytics from `~/.claude/ccb_log.jsonl` |
| `ccb style index-build` | Scan `~/.claude/skills/` and regenerate `INDEX.md` |
| `ccb style show` | Print current config (`~/.claude/ccb.toml`) |

---

## Usage

### Compress command output

Pipe any command through `ccb trim` before its output reaches Claude's context:

```bash
ccb trim git status
ccb trim npm test
ccb trim cargo build
```

Strips boilerplate lines (hints, "Compiling…", "Finished"), deduplicates consecutive identical lines, and logs before/after token counts to `~/.claude/ccb_log.jsonl`.

#### Real compression examples

**`cargo build` with a type error — 50% reduction**

```
# Before (90 tokens)
   Compiling serde v1.0.197
   Compiling serde_derive v1.0.197
   Compiling anyhow v1.0.86
   Compiling ccb v0.1.0 (/home/user/ccb)
error[E0308]: mismatched types
 --> src/main.rs:42:18
  |
42|     let x: u32 = "hello";
  |            ---   ^^^^^^^ expected `u32`, found `&str`
error: aborting due to 1 previous error
   Finished dev [unoptimized + debuginfo] target(s) in 3.14s

# After (45 tokens)
error[E0308]: mismatched types
 --> src/main.rs:42:18
  |
42|     let x: u32 = "hello";
  |            ---   ^^^^^^^ expected `u32`, found `&str`
error: aborting due to 1 previous error
```

**`npm install` clean — 94% reduction**

```
# Before (92 tokens)
npm warn deprecated inflight@1.0.6: This module is not supported
npm warn deprecated glob@7.2.3: Glob versions prior to v9 are no longer supported
npm warn deprecated rimraf@3.0.2: Rimraf versions prior to v4 are no longer supported
added 312 packages, audited 313 packages in 8s
3 packages are looking for funding
  run `npm fund` for details
found 0 vulnerabilities

# After (6 tokens)
found 0 vulnerabilities
```

**`pytest` with failures — 54% reduction**

```
# Before (122 tokens)
============================= test session starts ==============================
platform darwin -- Python 3.11.8, pytest-8.1.1, pluggy-1.4.0
rootdir: /Users/user/project
configfile: pyproject.toml
plugins: anyio-4.3.0, cov-5.0.0
collecting ...
collected 47 items

FAILED tests/test_api.py::test_create_story - AssertionError: 404
FAILED tests/test_api.py::test_update_story - AssertionError: 500

============================== 2 failed, 45 passed in 1.23s ==============================

# After (56 tokens)
FAILED tests/test_api.py::test_create_story - AssertionError: 404
FAILED tests/test_api.py::test_update_story - AssertionError: 500

============================== 2 failed, 45 passed in 1.23s ==============================
```

| command | tokens before | tokens after | saved | reduction |
|---------|-------------:|-------------:|------:|----------:|
| `cargo build` (type error) | 90 | 45 | 45 | **50%** |
| `npm install` (clean) | 92 | 6 | 86 | **94%** |
| `pytest` (2 failures) | 122 | 56 | 66 | **54%** |

### Lazy-load skills

Instead of injecting all skill files into every session:

```bash
# List available skills (reads INDEX.md)
ccb fade

# Load a specific skill on demand
ccb fade read-then-write
ccb fade hookify
```

Pair with the PreToolUse hook (below) so skills load automatically when invoked.

### Monitor context window

```bash
ccb context show
# context: 73% [██████████████░░░░░░] 🟡

ccb context clear 80
# ⚠️  ccb context: 85% used (threshold 80%) — consider /clear

ccb context compact 60
# ⚠️  ccb context: 73% used (threshold 60%) — consider /compact
```

Reads `CCB_CONTEXT_PCT` env var when wired as a hook, or `CCB_CTX_TOKENS` / `CCB_CTX_MAX`.

### Budget inspector

```bash
CCB_CONTEXT_PCT=62 ccb lineup
```

**window: `[██████░░░░] 62%`**

| resource | tokens | path |
|----------|-------:|------|
| INDEX (32 skills) | 820 | `~/.claude/skills/INDEX` |
| CLAUDE.md | 240 | `~/.claude/CLAUDE.md` |
| rules (5 files) | 1,480 | `~/.claude/rules/` |
| **ESTIMATED TOTAL** | **2,540** | |

### Token savings

```bash
ccb gain
```

| feature | tokens in | tokens out | saved | % |
|---------|----------:|-----------:|------:|--:|
| trim | 18,420 | 3,210 | 15,210 | 82% |
| buzz | 640 | 88 | 552 | 86% |
| **TOTAL** | **19,060** | **3,298** | **15,762** | **82%** |

*47 operations logged*

---

## Hook Integration

Wire ccb into Claude Code via `~/.claude/settings.json`. A reference config is in `config/hooks.json`.

### Lazy skill loading (PreToolUse)

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": { "tool_name": "Skill" },
        "hooks": [
          { "type": "command", "command": "~/.claude/hooks/skill_loader.sh" }
        ]
      }
    ]
  }
}
```

Copy `hooks/skill_loader.sh` to `~/.claude/hooks/skill_loader.sh`. The hook reads the skill name from `TOOL_INPUT`, calls `ccb fade <name>`, and returns the SKILL.md content as feedback — skills load on demand instead of being pre-injected.

### Context monitoring (PostToolUse)

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "hooks": [
          { "type": "command", "command": "~/.claude/hooks/context_monitor.sh" }
        ]
      }
    ]
  }
}
```

Copy `hooks/context_monitor.sh` to `~/.claude/hooks/`. Warns after every tool call when compact (70%) or clear (85%) thresholds are breached. Thresholds are env-configurable: `CCB_COMPACT_THRESHOLD`, `CCB_CLEAR_THRESHOLD`.

### Build the skills index

```bash
ccb style index-build
# INDEX.md written to /Users/you/.claude/skills/INDEX.md
```

Re-run whenever you add or update a skill.

---

## Optional Features

```bash
cargo build --release --features graph    # code symbol graph (SQLite + tree-sitter)
cargo build --release --features expert   # unified knowledge graph (Layer 3)
cargo build --release --features route    # model router proxy binary
cargo build --release --features full     # everything
```

### Code Graph

Builds a SQLite-backed symbol index across Rust, Python, TypeScript, and JavaScript files:

```bash
ccb graph index ./src          # index a directory
ccb graph search "compress"    # find symbols by name
ccb graph show src/main.rs     # show all symbols in a file
ccb graph stats                # aggregate counts by language
```

### Knowledge Graph (Expert System)

A unified graph of all knowledge — skills, tools, MCPs, personas, domain rules, code symbols. Traversed before tool execution to surface relevant context without pre-loading anything.

```bash
ccb expert list                          # list all nodes by kind
ccb expert activate backend-developer    # set active persona
ccb graph walk "fix auth validation"     # traverse graph, print activated nodes
```

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full design.

### Model Router

Routes Claude API calls across multiple backends (local Ollama, aibox, Anthropic) based on model tier. Binary: `ccb-route`.

```bash
cargo build --release --features route
./target/release/ccb-route
# listens on :9001 by default
```

---

## Benchmarks

Criterion benchmarks run against real fixture files (git status, pytest output, tsc output):

```bash
cargo bench
# Opens HTML report at target/criterion/compression/report/index.html
```

Every operation is also logged to `~/.claude/ccb_log.jsonl`:

```json
{"timestamp":"2026-05-21T14:23:01Z","feature":"trim","command":"git status","tokens_in":840,"tokens_out":142,"bytes_in":3360,"bytes_out":568}
```

---

## Why not just RTK?

[RTK](https://github.com/reachingforthejack/rtk) is great at one thing: compressing command output. ccb is the full barber shop:

| Capability | RTK | ccb |
|-----------|-----|-----|
| Command output compression | ✓ | ✓ (`trim`) |
| Token savings analytics | ✓ | ✓ (`gain`) |
| Lazy skill/context loading | — | ✓ (`fade`) |
| Context window monitoring | — | ✓ (`context`) |
| Budget inspector | — | ✓ (`lineup`) |
| Knowledge graph (Layer 3) | — | ✓ (`expert`) |
| Hook scripts included | — | ✓ |
| Build without unused features | — | ✓ (feature flags) |

---

## Roadmap

- [x] Layer 1 — Token management (`trim`, `fade`, `context`, `buzz`, `gain`)
- [x] Layer 2 — Code symbol graph (`graph index`, `graph search`)
- [x] Layer 3 foundation — unified knowledge graph schema + CLI
- [ ] Layer 3 — domain dataset ingest (sentinel, coder, architect)
- [ ] Layer 3 — pre-tool hook traversal in production
- [ ] `ccb-swift` — Apple platform port for Atlas/Alchemy

---

## Contributing

1. Fork the repo
2. Create a feature branch (`git checkout -b feature/your-feature`)
3. Make your changes — `cargo test` must pass, `cargo clippy -- -D warnings` must be clean
4. Commit (`git commit -m 'feat: your feature'`)
5. Push and open a PR against `main`

CI runs on every PR: `cargo test`, `cargo clippy`, `cargo fmt --check`.

---

## Project Structure

```
src/
├── main.rs            # entry point, command dispatch
├── cli.rs             # clap definitions
├── config.rs          # ccb.toml loading
├── log.rs             # token estimation, CompressionEvent, JSONL logging
├── analytics.rs       # ccb gain — aggregate savings from log
├── utils.rs           # shared utilities (progress bar)
└── features/
    ├── trim.rs        # command output compression + tests
    ├── fade.rs        # lazy skill loading + index lookup
    ├── context.rs     # context window monitoring
    ├── lineup.rs      # context budget report
    ├── buzz.rs        # nuclear mode cleanup + tests
    ├── cut.rs         # all-in-one compression
    ├── index.rs       # skills index generator
    ├── expert.rs      # unified knowledge graph (--features expert)
    └── graph.rs       # code symbol graph (--features graph)
hooks/
├── skill_loader.sh        # PreToolUse hook for /skill
├── context_monitor.sh     # PostToolUse hook for context checks
└── expert_pretooluse.sh   # PreToolUse hook for knowledge graph traversal
docs/
├── ARCHITECTURE.md        # three-layer design + knowledge graph spec
├── TEST_DATA.md           # real trim fixture inputs/outputs
└── TEST_DATA_LAYER3.md    # real expert graph fixture inputs/outputs
```

## Configuration

`~/.claude/ccb.toml` (auto-created with defaults on first use):

```toml
terse = false
conversation_style = false

[features]
trim = true
fade = true
sandbox = false
terse = false
graph = false
expert = false
```

---

## License

MIT — see [LICENSE](LICENSE) for details.
