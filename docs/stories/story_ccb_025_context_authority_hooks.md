# CCB Story 025: Context Authority — Hook Interception & Structured Retrieval

**Status:** READY
**Priority:** P1 — delivers the token savings
**Sprint:** CCB-6 (Context Authority)
**Feature flag:** `context`
**Depends on:** CCB-024 (unified index), CCB-020 (memory search)

## Narrative
**As a** Claude Code user,
**I want** the context authority to intercept every hook point and inject focused, structured data instead of raw text,
**So that** each turn gets only the context it needs, token costs drop, and model responses improve from relevant-not-noisy input.

## Context

Today's hook landscape:
- **SessionStart:** `session-env.sh` injects env vars. Expert persona loads via `ccb expert query --json`. CLAUDE.md files injected in full by Claude Code itself.
- **PreToolUse:** Empty `[]` — classify exists but isn't wired. No tool-specific context injection.
- **PostToolUse:** `context_monitor.sh` tracks context size. `console-log-warn.sh` checks for console.log.

The context authority replaces this with a unified interception layer:

```
SessionStart → ccb context inject --hook session-start
  Output: top-K nodes by weight for this project
  Replaces: full CLAUDE.md injection (3-5k tokens → ~500-800 tokens)

PreToolUse → ccb context inject --hook pre-tool --tool <name> --input <json>
  Output: relevant expert/skill/rule for this specific tool call
  Replaces: nothing today (gap) → ~100-300 tokens of targeted guidance

PostToolUse → ccb context trace --tool <name> --result <summary>
  Output: none (logging only)
  Feeds: weight update pipeline (CCB-026)
```

### Two-Tier Injection

Not all context can be retrieval-only. Some rules must always be present:

```
Tier 1: Always-inject (irreducible core, ~500 tokens)
  - Contract commitments (honesty, verification-before-reporting)
  - Active safety rules (deny list, auth switching)
  - Active persona identity
  - Current project identifier

Tier 2: Retrieve-on-demand (weight-driven, ~0-800 tokens)
  - Code standards (git.md, code.md, design.md sections)
  - Tool-specific rules
  - Expert domain knowledge
  - Skill instructions
  - Code graph context for current files
```

Tier 1 nodes are marked `always_inject: true` in metadata. They bypass weight thresholds. Everything else is Tier 2 — retrieved by weight × relevance.

### Output Format

The hook outputs structured JSON, not markdown prose:

```json
{
  "tier1": {
    "persona": "planner",
    "core_rules": ["verify before reporting", "never falsify status"],
    "safety": ["gh auth switch before push", "never read ~/.secrets"]
  },
  "tier2": [
    {"kind": "doc_section", "name": "branch-workflow", "weight": 0.88,
     "content": "All changes through feature branch + PR, no direct main pushes."},
    {"kind": "expert", "name": "sentinel", "weight": 0.72,
     "content": "Security review: auth code detected in changed files."},
    {"kind": "skill", "name": "pr-workflow", "weight": 0.65,
     "summary": "PR creation checklist with AC verification."}
  ],
  "tokens_saved": 3200,
  "tokens_injected": 780
}
```

## Architecture

### Hook Scripts

Three thin shell scripts that call `ccb context inject`:

```bash
# ~/.claude/hooks/context-inject-session.sh (SessionStart)
#!/bin/bash
ccb context inject --hook session-start --project "$CCB_PROJECT" --format json

# ~/.claude/hooks/context-inject-tool.sh (PreToolUse)
#!/bin/bash
ccb context inject --hook pre-tool --format json < /dev/stdin

# ~/.claude/hooks/context-trace.sh (PostToolUse)
#!/bin/bash
ccb context trace < /dev/stdin
```

### Retrieval Algorithm

```
fn retrieve(hook_type, tool_context) -> ContextPayload:
  1. Load all Tier 1 nodes (always_inject=true) → core block
  2. Determine topic signals:
     - SessionStart: project name, recent file types, git branch
     - PreToolUse: tool name, tool input paths/patterns
  3. Score Tier 2 nodes: weight × relevance(topic_signals, node)
  4. Sort by score descending
  5. Take top-K where sum(estimated_tokens) < budget
  6. For doc_section nodes: read content from source file at byte offset
  7. For skill nodes: read summary (first paragraph), not full file
  8. Return structured JSON payload
```

### Token Budget

Default budget: 1000 tokens for Tier 2 content. Configurable via `CCB_CONTEXT_BUDGET` env var or `~/.config/ccb/context.toml`.

The budget is a ceiling, not a target. If only 200 tokens of relevant content exists, inject 200.

## Acceptance Criteria

### SessionStart Hook
- [ ] **AC1:** `ccb context inject --hook session-start` returns JSON payload with tier1 + tier2 blocks
- [ ] **AC2:** Tier 1 always includes: active persona, core rules (from nodes marked always_inject), safety rules
- [ ] **AC3:** Tier 2 retrieves top-K doc_section and expert nodes by weight, filtered to current project
- [ ] **AC4:** Output includes `tokens_injected` and `tokens_saved` (vs. full CLAUDE.md injection baseline)
- [ ] **AC5:** Hook script `context-inject-session.sh` wired into settings.json SessionStart array

### PreToolUse Hook
- [ ] **AC6:** `ccb context inject --hook pre-tool` reads hook payload from stdin (tool_name, tool_input)
- [ ] **AC7:** Retrieves tool-relevant nodes: skills matching the tool, expert domains matching file paths in tool_input, doc_sections matching tool patterns
- [ ] **AC8:** For `Edit`/`Write` tools: includes code graph context (symbols in target file, callers/callees if edges exist)
- [ ] **AC9:** For `Bash(git *)` tools: includes git workflow rules at elevated weight
- [ ] **AC10:** Hook script `context-inject-tool.sh` wired into settings.json PreToolUse array
- [ ] **AC11:** Exit code 0 (allow) always — this is injection, not blocking. Classify remains separate

### PostToolUse Hook
- [ ] **AC12:** `ccb context trace` reads PostToolUse hook payload from stdin
- [ ] **AC13:** Logs tool call to traces.db (reuses CCB-018 trace_events schema)
- [ ] **AC14:** Records which context_nodes were in the injection payload for this turn (for weight feedback)
- [ ] **AC15:** Hook script `context-trace.sh` wired into settings.json PostToolUse array, async mode

### Token Budget
- [ ] **AC16:** Default Tier 2 budget: 1000 tokens. Configurable via env var `CCB_CONTEXT_BUDGET`
- [ ] **AC17:** Retrieval stops adding nodes when budget would be exceeded
- [ ] **AC18:** Doc section content retrieved from source file on demand (byte offset read), not stored in DB

### Structured Output
- [ ] **AC19:** All hook outputs are valid JSON matching the schema in Architecture section
- [ ] **AC20:** `--format human` option renders a readable summary for debugging (not used in hooks)
- [ ] **AC21:** Content strings are pre-truncated to fit budget — no single node exceeds 50% of budget

### Integration
- [ ] **AC22:** `ccb install` updated to wire all three hook scripts into settings.json
- [ ] **AC23:** Existing hooks (session-env.sh, context_monitor.sh, console-log-warn.sh) preserved — context hooks are additive
- [ ] **AC24:** Classify hook (when wired) remains independent — context authority injects, classify gates

### Tests
- [ ] **AC25:** Unit tests: tier1 always present regardless of weight
- [ ] **AC26:** Unit tests: tier2 respects budget ceiling
- [ ] **AC27:** Unit tests: PreToolUse tool-specific routing (Edit → code context, Bash(git) → git rules)
- [ ] **AC28:** Integration test: full inject cycle with mock hook payloads

## Notes

- The context authority does NOT replace CLAUDE.md — it replaces how CLAUDE.md content gets into the prompt. The .md files remain the source of truth for human reading. The context authority reads them, decomposes them, and serves relevant sections.
- Tier 1 nodes should be few (~5-10). If Tier 1 grows beyond ~500 tokens, something is wrong — reassess what truly must be always-on.
- The `tokens_saved` metric in the output is the primary ROI signal. Track it over sessions.
- PreToolUse injection must be fast (<100ms). If retrieval is slow, the hook blocks every tool call.
- Consider: should the SessionStart hook ALSO output a reduced CLAUDE.md that Claude Code can use instead of the full chain? This requires understanding how Claude Code handles hook output at session start.
