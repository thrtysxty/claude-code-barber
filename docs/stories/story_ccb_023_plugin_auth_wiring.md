# CCB Story 023: Plugin Auth Wiring — GitHub MCP + Cloudflare

**Status:** READY
**Priority:** P2 — quality of life, token savings
**Sprint:** Standalone (no dependencies)
**Feature flag:** None (plugin config, not CCB code)

## Narrative
**As a** Claude Code user,
**I want** the GitHub and Cloudflare MCP plugins authenticated and working,
**So that** GitHub operations use the structured MCP interface (fewer tokens) instead of `gh` CLI text output, and Cloudflare infrastructure operations are available directly.

## Context

Audit on 2026-05-28 found 20 plugins enabled, 11 removed (no binary/no project), asana removed (not used). Of the remaining 8 plugins:

- **playwright** — working (23 tools)
- **hookify** — working
- **rust-analyzer, pyright, typescript, swift LSPs** — working when their server binaries are present
- **github** — FAILED every session. Needs `GITHUB_PERSONAL_ACCESS_TOKEN` env var. Currently falls back to `gh` CLI which dumps verbose text into context.
- **cloudflare** — 4 of 5 sub-servers need auth. Only `cloudflare-docs` works (2 tools).

### Why GitHub MCP > gh CLI

The `gh` CLI returns unstructured text that enters the context window raw:
```
gh pr list → multi-line table, headers, formatting → ~200-500 tokens per call
gh pr view 7 → full PR body, comments, checks → ~500-2000 tokens per call
```

The GitHub MCP returns structured tool results — only the fields Claude needs, no parsing overhead, no verbose formatting. Estimated 40-60% token reduction on GitHub operations.

### Plugin MCP configs

**GitHub** (`~/.claude/plugins/cache/claude-plugins-official/github/unknown/.mcp.json`):
```json
{
  "github": {
    "type": "http",
    "url": "https://api.githubcopilot.com/mcp/",
    "headers": {
      "Authorization": "Bearer ${GITHUB_PERSONAL_ACCESS_TOKEN}"
    }
  }
}
```

**Cloudflare** — uses browser-based OAuth via the plugin's `authenticate` / `complete_authentication` tool calls.

**Asana** — removed from `enabledPlugins` on 2026-05-28. Can re-enable if needed.

## Acceptance Criteria

### GitHub MCP
- [ ] **AC1:** Create a GitHub Personal Access Token (PAT) with scopes: `repo`, `read:org`, `workflow`, `delete_repo` (matching current `gh auth` scopes)
- [ ] **AC2:** Add `GITHUB_PERSONAL_ACCESS_TOKEN` to `~/.secrets` (never inline, loaded by `.zshenv`)
- [ ] **AC3:** Verify the token is available in Claude Code sessions: `echo $GITHUB_PERSONAL_ACCESS_TOKEN` should be non-empty
- [ ] **AC4:** Verify GitHub MCP plugin connects: `/mcp` should show `plugin:github:github · ✔ connected`
- [ ] **AC5:** Test: create a test issue via MCP, verify it appears on GitHub, delete it
- [ ] **AC6:** Test: `gh pr list` vs MCP equivalent — compare token counts in context

### Cloudflare MCP
- [ ] **AC7:** Run `authenticate` tool on `cloudflare-api` sub-server, complete browser OAuth flow
- [ ] **AC8:** Run `authenticate` tool on `cloudflare-bindings` sub-server
- [ ] **AC9:** Run `authenticate` tool on `cloudflare-builds` sub-server
- [ ] **AC10:** Run `authenticate` tool on `cloudflare-observability` sub-server
- [ ] **AC11:** Verify all 5 Cloudflare sub-servers show `✔ connected` in `/mcp`
- [ ] **AC12:** Test: query DNS records for thrtysxty.com or bytereactr.com via MCP

### Cleanup
- [ ] **AC13:** Verify total plugin count in `/mcp` — should be 8 plugins, all connected, no failures

## Notes

- AC1-AC3 are human-gated (Jeremy must create the PAT on github.com)
- The PAT goes in `~/.secrets` which is sourced by `.zshenv` and deny-listed from agent reads
- Cloudflare OAuth may need re-auth periodically — document the refresh flow
- Once GitHub MCP works, consider whether `gh` CLI is still needed in sessions or if MCP fully replaces it
- `gh` CLI remains the fallback for edge cases the MCP doesn't cover (e.g. `gh api` raw calls)
