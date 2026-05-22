# Claude Code Barber 💈

> Your AI's context, styled.

**ccb** is a composable token management layer for Claude Code. It compresses noisy command output, lazy-loads skills on demand, monitors your context window, and logs token savings — built as a single Rust binary with optional feature flags so you only ship what you need.

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

## Install

```bash
git clone https://github.com/LitlBitz/claude-code-barber
cd claude-code-barber
cargo build --release
cp target/release/ccb ~/.local/bin/
```

### Build options

```bash
# Default — trim + fade enabled
cargo build --release

# All features (see Optional Features below)
cargo build --release --features full

# Minimal — context awareness and analytics only
cargo build --release --no-default-features
```

## Usage

### Compress command output

Pipe any command through `ccb trim` before its output reaches Claude's context:

```bash
ccb trim git status
ccb trim npm test
ccb trim cargo build
```

Strips boilerplate lines (hints, "Compiling…", "Finished"), deduplicates consecutive identical lines, and logs before/after token counts to `~/.claude/ccb_log.jsonl`.

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

Reads `CCB_CONTEXT_PCT` env var when wired as a hook (see below), or `CCB_CTX_TOKENS` / `CCB_CTX_MAX`.

### Budget inspector

```bash
CCB_CONTEXT_PCT=62 ccb lineup
```

```
╭─────────────────────────────────────────────────────╮
│               CCB — Context Budget                  │
├─────────────────────────────────────────────────────┤
│  window  [██████░░░░] 62%                           │
├──────────────────┬─────────┬───────────────────────┤
│ resource         │  tokens │ path                  │
├──────────────────┼─────────┼───────────────────────┤
│ INDEX (32 skills)│     820 │ ~/.claude/skills/INDEX│
│ CLAUDE.md        │     240 │ ~/.claude/CLAUDE.md   │
│ rules (5 files)  │    1480 │ ~/.claude/rules/      │
├──────────────────┼─────────┼───────────────────────┤
│ ESTIMATED TOTAL  │    2540 │                       │
╰──────────────────┴─────────┴───────────────────────╯
```

### Token savings

```bash
ccb gain
```

```
╭──────────────────────────────────────────────────╮
│               CCB — Token Savings                │
├──────────────┬──────────┬──────────┬────────────┤
│ feature      │ tokens↓  │ tokens↑  │ saved      │
├──────────────┼──────────┼──────────┼────────────┤
│ trim         │    18420 │     3210 │   15210  82%│
│ buzz         │      640 │       88 │     552  86%│
├──────────────┼──────────┼──────────┼────────────┤
│ TOTAL        │    19060 │     3298 │   15762  82%│
╰──────────────┴──────────┴──────────┴────────────╯
  47 operations logged
```

## Hook integration

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

After installing, generate INDEX.md from your existing skills:

```bash
ccb style index-build
# INDEX.md written to /Users/you/.claude/skills/INDEX.md
```

Re-run whenever you add or update a skill.

## Benchmarks

Criterion benchmarks run against real fixture files (git status, pytest output, tsc output):

```bash
cargo bench
# Opens HTML report at target/criterion/compression/report/index.html
```

Evidence is also written to `~/.claude/ccb_log.jsonl` — one JSON line per operation:

```json
{"timestamp":"2026-05-21T14:23:01Z","feature":"trim","command":"git status","tokens_in":840,"tokens_out":142,"bytes_in":3360,"bytes_out":568}
```

## Optional Features

```bash
cargo build --release --features graph   # code symbol graph (SQLite)
cargo build --release --features route   # model router proxy binary
cargo build --release --features full    # everything
```

### Code Graph `[experimental]`

Builds a SQLite-backed symbol index across Rust, Python, TypeScript, and JavaScript files:

```bash
ccb graph index ./src          # index a directory
ccb graph search "compress"    # find symbols by name
ccb graph show src/main.rs     # show all symbols in a file
ccb graph stats                # aggregate counts by language
```

### Model Router `[experimental]`

Routes Claude API calls across multiple backends (aibox, Ollama, Anthropic) based on model tier. Binary: `ccb-route`.

```bash
cargo build --release --features route
./target/release/ccb-route
# listens on :9001 by default
```

## Why not just RTK?

[RTK](https://github.com/reachingforthejack/rtk) is great at one thing: compressing command output. ccb is the full barber shop:

| Capability | RTK | ccb |
|-----------|-----|-----|
| Command output compression | ✓ | ✓ (`trim`) |
| Token savings analytics | ✓ | ✓ (`gain`) |
| Lazy skill/context loading | — | ✓ (`fade`) |
| Context window monitoring | — | ✓ (`context`) |
| Budget inspector | — | ✓ (`lineup`) |
| Hook scripts included | — | ✓ |
| Build without unused features | — | ✓ (feature flags) |

If you only want output compression, `ccb trim` is a drop-in replacement. If you want the rest, it is already here.

## How It Works

```
your command → ccb trim → filtered output → Claude Code context
```

`trim` runs your command, merges stdout + stderr, filters boilerplate, deduplicates consecutive identical lines, then prints the compressed result. Every operation is logged to `~/.claude/ccb_log.jsonl` as a `CompressionEvent` with before/after token counts, so `ccb gain` can report cumulative savings.

`fade` reads the skill index table, resolves the file path, and prints the content — letting you inject domain knowledge (style guides, patterns, prompts) without pre-loading everything.

## Token Estimation

CCB estimates tokens as `ceil(bytes / 4)` — the standard approximation for English text with subword tokenization. This is used for logging and the `lineup` budget display, not for any correctness-critical path.

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
    └── graph.rs       # code symbol graph (--features graph)
hooks/
├── skill_loader.sh    # PreToolUse hook for /skill
└── context_monitor.sh # PostToolUse hook for context checks
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
```

## License

MIT
