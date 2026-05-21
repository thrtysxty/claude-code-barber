# Claude Code Barber 💈

> Your AI's context, styled.

**ccb** is a composable token management layer for Claude Code and AI coding agents. Where other tools solve one problem, ccb gives you the full menu — build with only what you need.

## Features

| Command | Feature flag | What it does |
|---------|-------------|--------------|
| `ccb trim` | `trim` | Compress noisy command output (git, pytest, tsc) before it hits the context window |
| `ccb fade` | `fade` | Lazy-load skills, personas, and MCP resources on demand from an index |
| `ccb context` | always on | Monitor context window usage, suggest `/clear` or `/compact` |
| `ccb gain` | always on | Show token savings analytics from the session log |
| `ccb style index-build` | always on | Generate `~/.claude/skills/INDEX.md` from your skills directory |
| `ccb buzz` | `full` | Nuclear option — all features at maximum |

## Install

```bash
cargo install ccb
# or build from source:
git clone https://github.com/LitlBitz/claude-code-barber
cd claude-code-barber
cargo build --release
cp target/release/ccb ~/.local/bin/
```

## Build options

```bash
# Default (trim + fade)
cargo build --release

# All features
cargo build --release --features full

# Minimal — just context awareness and analytics
cargo build --release --no-default-features
```

## Usage

```bash
# Compress git output before Claude sees it
ccb trim git status

# Load a skill on demand (used by PreToolUse hook)
ccb fade read-then-write

# Check context window usage
ccb context show

# Warn if context exceeds 80%
ccb context clear 80

# Rebuild your skills index
ccb style index-build

# See how many tokens you've saved
ccb gain
```

## Hook integration

Add to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Skill",
        "hooks": [{ "type": "command", "command": "ccb fade $HOOK_TOOL_INPUT_SKILL" }]
      }
    ],
    "PostToolUse": [
      {
        "matcher": ".*",
        "hooks": [{ "type": "command", "command": "ccb context compact 60" }]
      }
    ]
  }
}
```

## Benchmarks

```bash
cargo bench
# Opens HTML report at target/criterion/compression/report/index.html
```

Evidence logs are written to `~/.claude/ccb_log.jsonl` — one JSON line per operation with before/after token counts.

## Comparison

| Tool | What it solves |
|------|---------------|
| **ccb trim** | Same as RTK — command output compression |
| **ccb fade** | Unique — lazy-load context, not pre-injected |
| **ccb context** | Unique — context budget awareness + auto-suggest |
| **ccb gain** | Same as `rtk gain` — token savings analytics |

## License

MIT
