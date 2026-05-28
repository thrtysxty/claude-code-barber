# CCB Story 028: Plan → Build → Verify Loop

**Status:** READY
**Priority:** P1 — core developer workflow
**Sprint:** CCB-6 (Context Authority) or standalone
**Feature flag:** `loop`
**Depends on:** None (standalone value; enhanced by CCB-018 traces, CCB-024 context)

## Narrative
**As a** developer using CCB with Claude Code,
**I want** a structured plan→build→verify loop that reads a story, detects my repo type, runs the right quality gates, handles failures, and commits on success,
**So that** sprint execution is repeatable, repo-portable, and self-correcting — not a manual chain of commands I have to remember.

## Context

Today's workflow is manual glue. The developer reads a story, implements, remembers to run the right build command, remembers to check ACs, remembers to commit to a branch. Each step is a separate mental task. Failures disappear between attempts. There's no structured retry. Nothing adapts to the repo type.

RALPH (github.com/allierays/agentic-loop) demonstrates the pattern: PRD-driven stories → autonomous code→test→commit loop → failure persistence → lesson capture. But it's hardcoded to one Node.js project with one CI pipeline.

CCB's loop generalizes this across repo types. `Cargo.toml` → Rust gates. `package.json` → TypeScript gates. `Package.swift` → Swift gates. `pyproject.toml` → Python gates. One command set, any project.

### Repo Detection

```
ccb detects project type by walking up from CWD looking for:

Cargo.toml      → Rust   (cargo fmt, clippy, test, build)
package.json    → TS/JS  (tsc, lint, test, build)
Package.swift   → Swift  (swift build, swift test)
pyproject.toml  → Python (pytest, pyright/mypy)
Makefile        → fallback (make test, make build)
```

If multiple manifests exist (monorepo), use the nearest to CWD. If none found, error with guidance.

### Quality Gate Registry

Each repo type has a gate sequence. Gates run in order. Failure at any gate stops the sequence.

```
Rust gates:
  1. cargo fmt --check
  2. cargo clippy --features <detected> -- -D warnings
  3. cargo test --features <detected>
  4. cargo build --features <detected>

TypeScript gates:
  1. npx tsc --noEmit
  2. npm run lint (if script exists)
  3. npm test (if script exists)
  4. npm run build

Swift gates:
  1. swift build
  2. swift test

Python gates:
  1. pyright (if installed) or mypy
  2. pytest
```

Feature detection: for Rust, parse `Cargo.toml` `[features]` section. If a `full` feature exists, use `--features full`. Otherwise use default features.

## Architecture

### Three Commands

```
ccb plan <story-file>
  Reads a story markdown file, parses ACs, detects repo type,
  runs STEP ZERO, generates a phased implementation plan.
  Outputs plan to stdout (JSON) and optionally saves to
  .ccb/plans/<story-slug>.json

ccb build [--plan <plan-file>] [--story <story-file>]
  Executes implementation phases with quality gates.
  If --plan: follows the phased plan.
  If --story: reads story directly, implements without pre-plan.
  Runs repo-detected gates after each phase.
  On failure: persists context, retries (max 3).
  On success: commits to feature branch, moves to next phase.
  After all phases: pushes + opens PR.

ccb lesson <description>
  Captures a failure pattern or process learning.
  Stores in ~/.cache/ccb/lessons/<repo-name>/<slug>.md
  Auto-loaded into build context on subsequent runs.
```

### Plan Output Format

```json
{
  "story": "story_ccb_015_graph_edges.md",
  "repo": "claude-code-barber",
  "repo_type": "rust",
  "branch": "feat/graph-edges",
  "phases": [
    {
      "name": "Schema migration",
      "files": ["src/features/graph.rs"],
      "acs": ["AC1", "AC13"],
      "description": "Add edges table to graph.db schema migration",
      "verify": "cargo build --features graph"
    },
    {
      "name": "Rust edge extraction",
      "files": ["src/features/graph.rs"],
      "acs": ["AC2", "AC3", "AC4"],
      "description": "Extract call, import, implement edges from Rust AST",
      "verify": "cargo test --features graph"
    }
  ],
  "gates": ["cargo fmt --check", "cargo clippy --features full -- -D warnings",
            "cargo test --features full", "cargo build --features full"],
  "step_zero": "git log origin/main --oneline -i --grep='graph.edges'"
}
```

### Failure Persistence

When a quality gate fails, the failure context is saved:

```
~/.cache/ccb/failures/<repo>/<story>_<timestamp>.md

Contents:
  - Which gate failed (e.g. "cargo clippy")
  - Full error output (truncated to 2000 chars)
  - Which phase was in progress
  - Files modified so far
  - Attempt number (1/3, 2/3, 3/3)
```

On retry, the failure file is loaded into context so the next attempt knows what went wrong. After 3 failures on the same gate, the build stops and reports — no infinite retry.

### Lesson Storage

```
~/.cache/ccb/lessons/<repo-name>/<slug>.md

Example: ~/.cache/ccb/lessons/claude-code-barber/clippy-unknown-lints.md

Contents:
  # clippy-unknown-lints
  
  **Learned:** 2026-05-28
  **Repo:** claude-code-barber
  **Context:** Clippy lint `manual_checked_ops` exists in Rust 1.95 but not 1.93
  
  ## Pattern
  When adding clippy allows for lints that may not exist in all Rust versions,
  always pair with `unknown_lints`: `#[allow(unknown_lints, clippy::the_lint)]`
  
  ## Trigger
  cargo clippy failure mentioning "unknown lint"
```

Lessons are loaded into `ccb build` context when working in the matching repo. When CCB memory (018-019) ships, lessons migrate into the trace/pattern system automatically.

### STEP ZERO Integration

Before any implementation begins, `ccb plan` runs:

```bash
git log origin/main --oneline -i --grep="<story-slug>"
```

If any commit appears, the story is already implemented. `ccb plan` reports this and exits — no plan generated, no implementation started. This has prevented duplicate work twice already.

## Acceptance Criteria

### Repo Detection
- [ ] **AC1:** `ccb detect` identifies repo type from CWD by manifest file presence
- [ ] **AC2:** Supports: Rust (Cargo.toml), TypeScript (package.json), Swift (Package.swift), Python (pyproject.toml)
- [ ] **AC3:** Walks up from CWD to find nearest manifest (supports running from subdirectories)
- [ ] **AC4:** For Rust: parses Cargo.toml to detect available feature flags, prefers `full` if it exists
- [ ] **AC5:** `ccb detect --format json` outputs: repo_type, manifest_path, detected_features, gate_commands

### Plan Command
- [ ] **AC6:** `ccb plan <story.md>` reads story file, extracts ACs from `- [ ] **AC<N>:**` pattern
- [ ] **AC7:** Generates phased plan grouping related ACs by file scope
- [ ] **AC8:** STEP ZERO: checks git log for prior implementation, exits with warning if found
- [ ] **AC9:** Outputs plan as JSON to stdout
- [ ] **AC10:** `--save` flag writes plan to `.ccb/plans/<slug>.json`
- [ ] **AC11:** Plan includes: story path, repo type, branch name, phases with files/ACs/verify commands, gate commands

### Build Command
- [ ] **AC12:** `ccb build --story <story.md>` runs full plan→implement→verify cycle
- [ ] **AC13:** `ccb build --plan <plan.json>` follows a pre-generated plan
- [ ] **AC14:** Creates feature branch (`feat/<story-slug>`) if not already on one
- [ ] **AC15:** After each phase: runs all quality gates for detected repo type
- [ ] **AC16:** On gate failure: saves failure context to `~/.cache/ccb/failures/`
- [ ] **AC17:** On gate failure: retries with failure context loaded (max 3 attempts per phase)
- [ ] **AC18:** After 3 failures on same phase: stops, reports BLOCKED with failure summary
- [ ] **AC19:** On all gates pass: commits phase to feature branch with descriptive message
- [ ] **AC20:** After all phases complete: runs full gate sequence one final time
- [ ] **AC21:** After final pass: pushes branch, opens PR via `gh pr create`
- [ ] **AC22:** Loads lessons from `~/.cache/ccb/lessons/<repo>/` into build context

### Lesson Command
- [ ] **AC23:** `ccb lesson "description"` creates a lesson file in `~/.cache/ccb/lessons/<repo>/`
- [ ] **AC24:** Lesson file includes: timestamp, repo, context description, pattern, trigger
- [ ] **AC25:** `ccb lesson list` shows all lessons for current repo
- [ ] **AC26:** `ccb lesson list --all` shows lessons across all repos

### Quality Gates
- [ ] **AC27:** Rust gates: `cargo fmt --check` → `cargo clippy` → `cargo test` → `cargo build` (with detected features)
- [ ] **AC28:** TypeScript gates: `tsc --noEmit` → `npm run lint` → `npm test` → `npm run build` (skip missing scripts)
- [ ] **AC29:** Swift gates: `swift build` → `swift test`
- [ ] **AC30:** Python gates: `pyright`/`mypy` → `pytest` (skip missing tools gracefully)
- [ ] **AC31:** `ccb gates` shows the gate sequence for the current repo type
- [ ] **AC32:** `ccb gates --run` runs all gates without the build loop (standalone verification)

### Integration
- [ ] **AC33:** `loop` feature flag added to Cargo.toml
- [ ] **AC34:** `loop` included in `full` feature set
- [ ] **AC35:** Build traces logged to stdout in a format compatible with CCB-018 trace events (future integration)

### Tests
- [ ] **AC36:** Unit tests: repo detection for each supported type
- [ ] **AC37:** Unit tests: AC parsing from story markdown
- [ ] **AC38:** Unit tests: failure persistence write/read cycle
- [ ] **AC39:** Unit tests: lesson storage and retrieval
- [ ] **AC40:** Integration test: `ccb plan` on a fixture story produces valid plan JSON

## Notes

- `ccb build` is the command — it doesn't implement code itself. It's the loop harness that structures the implementation session. The developer (or Claude Code agent) does the actual coding between gate runs.
- The build command is designed to be called from Claude Code slash commands. A thin `/buildloop` wrapper in `~/.claude/commands/` can call `ccb build --story $ARGUMENTS` — but the logic lives in CCB, not in the command file.
- Lesson→memory migration: when CCB-018/019 ship, `ccb lesson` entries should be importable into traces.db as pre-existing patterns. Design the lesson format with this migration in mind.
- Gate commands are stored in the plan, not hardcoded into the build loop. This lets users override gates per-story if needed (e.g. skip lint for a WIP phase).
- The 3-failure limit is from `/systematic-debugging`: "If three different fix attempts fail, stop. This indicates an architectural problem." Same principle applied to the build loop.
- Consider: should `ccb build` support `--dry-run` that shows what it would do without executing? Useful for reviewing the gate sequence before committing to a loop.
- Consider: should failures auto-generate lessons? If the same gate fails across multiple stories with similar error patterns, that's a lesson waiting to be captured. This connects directly to CCB-019 pattern mining.
