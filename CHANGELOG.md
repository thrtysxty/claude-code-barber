# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

#### Model Router — Gateway Discovery & Multi-Provider `/model` Integration

- **Gateway discovery**: The router now serves a `/v1/models` endpoint that aggregates models from all configured providers. Claude Code's gateway model discovery fetches this list at startup and populates the `/model` picker with every available model.
- **Claude-prefix trick**: All model IDs returned by `/v1/models` are prefixed with `claude-` to pass Claude Code's gateway filter (`/^(claude|anthropic)/i`). The router strips the prefix on incoming requests before forwarding to the backend.
- **Auto-discovery**: Providers with `discover = true` (e.g., Ollama) auto-populate from the backend's model list. Pull a new model in Ollama and it appears in the picker on the next `claude` launch — no config changes needed. Static entries in `providers.toml` serve as metadata overrides (display name, tier) matched via `backend_id`.
- **Tier auto-assignment**: Discovered models without a static override get auto-assigned tiers based on parameter count: 200B+ Opus, 30B+ Sonnet, 10B+ Haiku, <10B Local.
- **Auth mode preservation**: Router works with `ANTHROPIC_AUTH_TOKEN` (OAuth) instead of `ANTHROPIC_API_KEY`, keeping Claude Code in subscription mode with the full model picker. OAuth tokens are forwarded as `Authorization: Bearer` to Anthropic.
- **Prefix routing in `pick()`**: The routing function is now async and resolves `claude-`-prefixed model IDs through both static catalog and discovery backends.
- **GET request logging**: `/v1/models` requests are now logged to stderr for diagnostics.
- **Shell setup**: Documentation updated — router env vars must go in `~/.zshenv` (not `.zprofile`) because interactive non-login shells (terminals, VS Code) only source `.zshenv` and `.zshrc`.

## [0.1.0] — 2026-05-24

### Added

#### Layer 1 — Token Management

- **`trim`** — Compress noisy command output (git, pytest, tsc, npm) before it hits the context window. Strips boilerplate lines, deduplicates consecutive identical lines, and logs before/after token counts to `~/.claude/ccb_log.jsonl`.
- **`fade [skill]`** — Lazy-load a skill from `INDEX.md` on demand — or list the full skills index.
- **`context show`** — Display current context window usage with an ASCII progress bar.
- **`context clear [threshold]`** — Warn when context exceeds threshold (default 80%).
- **`context compact [threshold]`** — Warn when context exceeds threshold (default 60%).
- **`lineup`** — Show everything loaded in the context budget with per-file token estimates.
- **`cut`** — Run context check and lineup in a single command.
- **`buzz`** — Nuclear mode: strip all INDEX.md overhead, prune log to last 500 events.
- **`gain`** — Token savings analytics from `~/.claude/ccb_log.jsonl`. Reports per-feature tokens in, tokens out, and savings percentage.
- **`style index-build`** — Scan `~/.claude/skills/` and regenerate `INDEX.md`.
- **`style show`** — Print current config from `~/.claude/ccb.toml`.
- **`install`** — Wire context monitor and skill loader hooks into `~/.claude/settings.json`. Supports `--auto` and `--dry-run`.

#### Layer 2 — Code Symbol Graph

- **`graph index [path]`** — Build a SQLite-backed symbol index across Rust, Python, TypeScript, and JavaScript files using tree-sitter AST parsing.
- **`graph search <query>`** — Find symbols by name across the indexed codebase.
- **`graph show <file>`** — Display all extracted symbols (functions, structs, enums, constants) for a given file with line numbers.
- **`graph stats`** — Aggregate symbol counts by language.

#### Layer 3 — Expert Personas & Knowledge Graph

- **`expert ingest --dataset <file.yaml>`** — Load a YAML persona dataset into the knowledge graph.
- **`expert build <name> --dataset <file>`** — Same as ingest, but names the expert explicitly.
- **`expert list`** — List all registered experts with active status.
- **`expert activate <name>`** — Set the active persona (persists across sessions).
- **`expert deactivate`** — Clear the active persona.
- **`expert walk "<task>"`** — Traverse the graph and print matched knowledge nodes for a given task description.
- **`expert query [--tool <name>`** — Hook-facing: emit context for the active persona at tool time.

#### Safety & Routing

- **`classify`** — Two-tier safety classifier for `PreToolUse` hooks. Tier 1 is instant local pattern matching. Tier 2 sends ambiguous actions to an LLM via OpenRouter. Integrates expert context and graph-aware Read hints.
- **`route`** — Model router proxy binary (`ccb-route`) that routes Claude API calls across multiple backends (local Ollama, aibox, Anthropic) based on model tier. Listens on `:9001` by default.

### Features

#### Default Features

The following features are enabled by default: `trim`, `fade`, `route`.

#### Opt-In Features

The following features must be enabled with `--features`:

| Feature | Enable flag | Notable dependencies |
|---------|-------------|---------------------|
| Code symbol graph | `--features graph` | tree-sitter (Rust, Python, TS, JS), rusqlite, walkdir, ignore |
| Expert personas | `--features expert` | rusqlite, serde_yaml |
| Classify | `--features classify` | reqwest, regex-lite |
| Sandbox | `--features sandbox` | — |
| Terse | `--features terse` | — |

#### Full Build

```bash
# Everything (trim, fade, sandbox, terse, graph, route, expert, classify)
cargo build --features full

# Install with full features
cargo install ccb --features full
```

### Comparison Links

[0.1.0]: https://github.com/thrtysxty/claude-code-barber/releases/tag/v0.1.0
