# CCB Story 029: Router Tier Resolution — In-Session Model Dispatch

**Status:** READY
**Priority:** P1 — unlocks non-Anthropic subagent dispatch without tmux
**Sprint:** Standalone (can ship anytime; enhances all subsequent sprint work)
**Feature flag:** `route`
**Depends on:** None (router already exists, providers.toml already has tier config)

## Narrative
**As a** developer using Claude Code with ccb-route,
**I want** the Agent tool's `model: "sonnet"` / `"haiku"` to route through my tier preferences in providers.toml,
**So that** I can dispatch subagents to MiniMax, Ollama, or aibox without leaving my session or needing tmux.

## Context

### The Problem Chain

Claude Code's Agent tool accepts three model tiers: `opus`, `sonnet`, `haiku`. When you specify `model: "sonnet"`, Claude Code maps it to a full Anthropic model ID like `claude-sonnet-4-20250514` and sends the API request to `ANTHROPIC_BASE_URL`.

When `ANTHROPIC_BASE_URL=http://localhost:9001` (ccb-route), the router receives that request. The routing chain today:

1. **Prefix strip** (ccb-route.rs:389): strips `claude-` → `sonnet-4-20250514`
2. **Exact match** (providers.rs:267): no provider has model `sonnet-4-20250514` → miss
3. **Case-insensitive match** (providers.rs:276): same → miss
4. **Tier keyword fallback** (providers.rs:286): `"sonnet-4-20250514".contains("sonnet")` → `Tier::Sonnet`
5. **First match wins** (providers.rs:297): iterates `HashMap<String, Provider>` — **non-deterministic order**
6. If Anthropic iterates first → routes to Anthropic (defeating the purpose)
7. If it somehow picks a non-Anthropic provider, the prefix-strip check (step 0) may still revert to the original name

### Why "models under opus get routed as opus"

Multiple compounding failures:
- HashMap iteration order means tier resolution is provider-order-dependent
- The `claude-` prefix strip logic checks `pname != "anthropic"` — if the tier match lands on Anthropic first, it keeps the original model name and the request goes to Anthropic as a passthrough
- No explicit tier routing table — tiers are an emergent property of which provider's model happens to iterate first
- When the fallback chain exhausts, the request falls through to Anthropic passthrough, which uses whatever model name was in the original request

### What providers.toml Already Has

The config is well-structured with per-model tiers:
```toml
# MiniMax opus-tier
{ id = "MiniMax-M2.7", tier = "opus" }
# MiniMax sonnet-tier
{ id = "MiniMax-M2.5", tier = "sonnet" }
# Ollama sonnet-tier
{ id = "glm-5.1", tier = "sonnet" }
# Ollama haiku-tier
{ id = "gemma4:31b", tier = "haiku" }
# aibox haiku-tier
{ id = "qwopus3.5-9b-v3", tier = "haiku" }
```

The tier metadata exists. The resolution logic just doesn't use it properly.

## Architecture

### Tier Routing Table

Add an explicit `[tier_routing]` section to providers.toml:

```toml
[tier_routing]
# When Agent tool sends a tier, try these models in order.
# Each entry is a model ID from the [providers.*] model lists.
# The router resolves the model → provider automatically.
# First available model wins.
opus   = ["MiniMax-M2.7", "deepseek-v4-pro", "claude-opus-4-7"]
sonnet = ["MiniMax-M2.5", "glm-5.1", "claude-sonnet-4-6"]
haiku  = ["qwopus3.5-9b-v3", "gemma4:31b", "claude-haiku-4-5-20251001"]

# Override: force ALL tier requests to a single model (testing/cost mode)
# override_all = "qwopus3.5-9b-v3"
```

This replaces the non-deterministic HashMap iteration with an explicit, user-ranked model preference per tier. No provider-level indirection — the user picks exact models in exact order.

### Resolution Flow (New)

```
Request arrives: model = "claude-sonnet-4-20250514"

1. Exact match in any provider's model list?
   → YES: route to that provider (existing behavior, works fine)
   → NO: continue

2. Prefix-based explicit override? (e.g. "ollama:gemma4")
   → Route directly (existing behavior, always honored)

3. Is this a tier request? (model name contains opus/sonnet/haiku)
   → Extract tier: "sonnet"
   → Look up [tier_routing].sonnet = ["MiniMax-M2.5", "glm-5.1", "claude-sonnet-4-6"]
   → For each model in the list:
     a. Resolve model → provider (existing resolve_model exact match)
     b. Is provider reachable? (optional: health check)
     c. Route to that provider with the model's backend_id
   → If no model resolves: fall through to Anthropic passthrough

4. Default: Anthropic passthrough with original model name
```

### Tier Detection Improvement

Current tier detection uses `model_id.contains("sonnet")`. This works for Anthropic model names but is fragile. Improve to:

```rust
fn extract_tier(model_id: &str) -> Option<Tier> {
    let mid = model_id.to_lowercase();
    // Strip claude- prefix and version suffixes for cleaner matching
    let clean = mid.strip_prefix("claude-").unwrap_or(&mid);
    
    if clean.starts_with("opus") || clean.contains("-opus-") {
        Some(Tier::Opus)
    } else if clean.starts_with("sonnet") || clean.contains("-sonnet-") {
        Some(Tier::Sonnet)
    } else if clean.starts_with("haiku") || clean.contains("-haiku-") {
        Some(Tier::Haiku)
    } else {
        None
    }
}
```

### Provider Health (Optional/Future)

Track provider availability:
- On successful response: mark provider healthy
- On connection failure: mark unhealthy, skip in tier preference chain
- Auto-recover after configurable timeout (default 60s)

This prevents routing to a dead aibox or unavailable Ollama instance.

### Logging

Every tier-routed request logs:
```
tier_route: sonnet → minimax/MiniMax-M2.5 (pref 1/3)
tier_route: haiku → aibox/qwopus3.5-9b-v3 (pref 1/3)
tier_route: sonnet → ollama/glm-5.1 (pref 2/3, minimax unavailable)
```

This makes routing transparent and debuggable.

## Acceptance Criteria

### Tier Routing Table
- [ ] **AC1:** `[tier_routing]` section parsed from providers.toml with per-tier model preference lists
- [ ] **AC2:** Each tier entry is an ordered array of model IDs that exist in `[providers.*].models`
- [ ] **AC3:** Default tier routing when section is absent: first model per tier found across all providers (preserving current behavior)
- [ ] **AC4:** `override_all` key routes ALL tier requests to a single model (for testing/cost optimization)
- [ ] **AC5:** Invalid model IDs in tier_routing produce a startup warning (not a crash)

### Resolution Logic
- [ ] **AC6:** Tier resolution walks the model preference list in order, not HashMap iteration order
- [ ] **AC7:** Each model in the list is resolved to its provider via existing exact-match logic
- [ ] **AC8:** `claude-sonnet-4-20250514` extracts tier "sonnet", walks sonnet preference list, routes to first resolvable model
- [ ] **AC9:** `claude-opus-4-7` extracts tier "opus", routes to first model in opus list (e.g. `MiniMax-M2.7`)
- [ ] **AC10:** `claude-haiku-4-5-20251001` routes to `qwopus3.5-9b-v3` when it's first in haiku list
- [ ] **AC11:** Direct model request (e.g. `MiniMax-M2.7` or `ollama:gemma4`) bypasses tier routing entirely
- [ ] **AC12:** If a model in the preference list can't be resolved (provider missing/misconfigured), skip to next
- [ ] **AC13:** If all models in the preference list exhausted, fall through to Anthropic passthrough

### Agent Tool Integration
- [ ] **AC14:** Agent tool `model: "sonnet"` dispatches subagent through tier routing (no tmux needed)
- [ ] **AC15:** Agent tool `model: "haiku"` dispatches to the user's preferred haiku-tier model
- [ ] **AC16:** Agent tool `model: "opus"` routes through tier preference like any other tier
- [ ] **AC17:** Parent session on Opus + subagent on `model: "sonnet"` → subagent uses tier-preferred model, NOT opus

### Configuration UX
- [ ] **AC18:** `ccb route tiers` shows current tier routing table with resolved model → provider mappings
- [ ] **AC19:** `ccb route tiers --test sonnet` shows which model and provider would handle a sonnet-tier request
- [ ] **AC20:** Changing providers.toml tier_routing is picked up on next request (no router restart needed)

### Logging & Transparency
- [ ] **AC21:** Every tier-routed request logs: tier, chosen model, provider, preference position (e.g. "1/3")
- [ ] **AC22:** Tier routing decisions visible in `ccb-route` stderr output
- [ ] **AC23:** Failed tier resolution (no model available) logs a warning with the full preference list tried

### Tests
- [ ] **AC24:** Unit test: tier extraction from Anthropic model names (`claude-sonnet-4-20250514` → Sonnet)
- [ ] **AC25:** Unit test: tier routing walks preference list in order (first resolvable model wins)
- [ ] **AC26:** Unit test: `override_all` routes every tier to the specified model
- [ ] **AC27:** Unit test: direct model match bypasses tier routing
- [ ] **AC28:** Unit test: missing `[tier_routing]` falls back to current behavior
- [ ] **AC29:** Integration test: end-to-end request with tier routing resolves to correct provider + model

## Notes

- This does NOT require changes to Claude Code or the Agent tool — it's entirely within ccb-route's routing layer. The Agent tool sends the model name, ccb-route decides where it goes.
- The `[tier_routing]` lists are model IDs, not provider names. The router resolves model → provider through the existing catalog. This eliminates both the HashMap ordering problem AND the "which model within this provider?" ambiguity.
- Provider health tracking is listed as optional/future — the core value is deterministic tier routing. Ship without health checks first.
- When Context Authority (CCB-024-026) ships, tier preferences could be per-project or per-task: "use cheap models for boilerplate, opus for architecture." That's a future story.
- The `override_all` key is a power tool for cost control: `override_all = "qwopus3.5-9b-v3"` means zero API cost for all subagent work.
- Consider: should tier routing also apply to the main session's `/model` picker? Probably not — `/model` is explicit user choice. Tier routing is for automated dispatch (Agent tool, build loops).
