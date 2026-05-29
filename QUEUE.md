# Projects Queue

**Last updated:** 2026-05-29 (PR #12 sprint 6 merged, all tests green 222/222)

### Discussion Presence
Stories live in their repos (`*/docs/stories/`). Discussion, planning, and exploration live here in `Projects/QUEUE.md`. Every item below was discussed or is fair game for next session.

### Readiness Summary
| Category | Count | Notes |
|----------|-------|-------|
| Agent-ready (CCB, Atlas, Copernicus, SHxTLxST unblocked) | ~31 | Pick up immediately |
| Human-only (infra + gates) | ~15 | flagged below |
| **Total stories** | **~71** | |

---

## 0 — Human Infrastructure Setup

*All blocking on Jeremy — no agent can do these.*

### Domain & Email (launch-plan.md G0c DONE)
- [x] Cloudflare Email Routing: bytereactr.com → dmac DONE
- [x] Cloudflare Email Routing: thrtysxty.com → dmac DONE

### Cloudflare DNS + Domains
- [ ] Add bytereactr.com to Cloudflare DNS (delegate thrtysxty.com → Cloudflare nameservers)
- [ ] Create Cloudflare tunnel for SHxTLxST from bizinfra → byteReactr.com subdomain
- [ ] Register shxtlist.com + atlasIDE.com

### GitHub Orgs
- [ ] Create github.com/ByteReactr org
- [ ] Create github.com/thrtysxty org (if not already created)
- [ ] Transfer repos: thrift-skynet, copernicus, claude-code-barber → thrtysxty org

### ByteReactr App Infra
- [ ] Create dev@bytereactr.com Cloudflare email routing
- [x] Enroll Apple Developer Program — Individual, seller name "ByteReactr", $99/yr ENROLLED

### Apple Developer Portal (active account — need to complete)
- [ ] Download + import Developer ID certificate (for notarization)
- [ ] Create App Store Connect API key (for altool/notarytool CI)
- [ ] Register bundle ID `com.bytereactr.shxtlxst`
- [ ] Enable Sign in with Apple capability
- [ ] Enable App Groups capability (`group.com.bytereactr.shxtlxst`)
- [ ] Add TestFlight beta testers + build configuration
- [ ] Verify Xcode can see DEVELOPMENT_TEAM in project.pbxproj

---

## 0.5 — CCB: Sprint 0 — Plan → Build → Verify Loop `@any`

*Verified against git 2026-05-28: PR #9 merged `ccb factory` commands (story file + `loop_cmd.rs` 707 lines) to main. Ships `ccb factory new/plan/build/advance/kickback/escalate/approve/status/list/show` with repo-type detection, quality gates, failure persistence, and lesson capture.*

### Story
- [x] story_ccb_028_plan_build_loop.md — **SHIPPED: PR #9 merged** — `ccb factory` story loop commands

---

## 1 — CCB: Sprint 4 — Full Code Graph `@any`

*Verified against git 2026-05-28: PR #9 merged graph edges; PR #10 merged graph watch. All shipped to main.*

### Stories — COMPLETED (all merged to main)
- [x] story_ccb_015_graph_edges.md — `edges` table, extract call/import/inherit for Rust/Python/TS/JS — **PR #9 merged**
- [x] story_ccb_016_graph_traversal.md — `callers`, `callees`, `chain`, `impact`, `dead`, `complexity` CLI — **shipped in PR #9**
- [x] story_ccb_017_graph_watch.md — `ccb graph watch` live re-index via `notify` crate — **PR #10 merged**

---

## 2 — CCB: Sprint 5 — Memory + LoCoMo Benchmark `@any`

*Verified against git 2026-05-28: PR #10 merged all sprint 5 work to main. `memory` feature flag added.*

### Stories — COMPLETED (all merged to main)
- [x] story_ccb_018_memory_traces.md — `traces.db`, session + event capture, hook integration — **PR #10 merged**
- [x] story_ccb_019_memory_patterns.md — frequency-based pattern mining → auto-generated skills — **PR #10 merged**
- [x] story_ccb_020_memory_search.md — FTS5 hybrid search + `ccb memory recall` context injection — **PR #10 merged**
- [x] story_ccb_021_locomo_benchmark.md — `ccb gain --locomo` compression quality measurement — **PR #10 merged**

---

## 2.5 — CCB: Sprint 6 — Context Authority `@any`

*Shipped: PR #12 merged 2026-05-29. 222/222 tests pass.*

### Stories — COMPLETED (merged to main)
- [x] story_ccb_024_context_authority_index.md — Unified knowledge index: `context_nodes` + `context_edges`, CLAUDE.md decomposition, weighted nodes — **PR #12 merged**
- [x] story_ccb_025_context_authority_hooks.md — Hook interception: SessionStart/PreToolUse/PostToolUse structured injection, two-tier model, token budget — **PR #12 merged**
- [x] story_ccb_026_context_authority_feedback.md — Weight feedback via EMA, LoCoMo validation gate, gap detection, auto skill/expert suggestions — **PR #12 merged**
- [x] story_ccb_027_context_dashboard.md — TUI + web dashboard: node inventory, injection log, weight explorer, gap report, token treemap — **PR #12 merged**

### Cross-Sprint Dependencies
```
015 (edges) ──→ 024 (context needs code data)
018 (traces) ──→ 024 (context needs trace schema)
019 (patterns) ──→ 026 (gap detection uses mined patterns)
020 (search) ──→ 025 (hook retrieval uses hybrid search)
021 (LoCoMo) ──→ 026 (validation gate for weight tuning)
```

---

## 3 — CCB: Completed Work (Sprint 1-3)

*Verified against git 2026-05-28. All merged to main.*

- [x] story_ccb_001 — Expert feature flag COMPLETE
- [x] story_ccb_002 — Expert graph schema COMPLETE
- [x] story_ccb_003 — Expert CLI COMPLETE
- [x] story_ccb_004 — Expert hook query COMPLETE
- [x] story_ccb_005 — Test trim COMPLETE
- [x] story_ccb_006 — Test graph COMPLETE
- [x] story_ccb_007 — Test expert COMPLETE
- [x] story_ccb_008 — Route command COMPLETE
- [x] story_ccb_009 — Expert ingest COMPLETE
- [x] story_ccb_010 — Sentinel dataset COMPLETE
- [x] story_ccb_011 — LoRA pipeline COMPLETE
- [x] story_ccb_012 — Architect expert layer COMPLETE
- [x] story_ccb_013 — Telemetry A/B test COMPLETE
- [x] story_ccb_014 — YASR statusline COMPLETE
- [x] ccb_release_changelog — COMPLETE
- [x] ccb_release_ci — COMPLETE (5 checks green)
- [x] ccb_release_license — COMPLETE
- [x] ccb_release_unit_tests — COMPLETE
- [x] Gateway discovery + multi-provider routing — COMPLETE (PR #7 merged)
- [x] Expert query + knowledge graph walk — COMPLETE

---

## 4 — CCB: Plugin Auth Wiring — GitHub MCP + Cloudflare

**State:** GitHub MCP plugin fails every session (needs `GITHUB_PERSONAL_ACCESS_TOKEN`). Cloudflare has 4/5 sub-servers unauthenticated. Asana removed. 20 → 8 plugins after audit.

- [ ] story_ccb_023_plugin_auth_wiring.md — Create PAT, wire GitHub MCP, authenticate Cloudflare (13 ACs, partly human-gated)

---

## 4.5 — CCB: Router Tier Resolution `@any`

Run from: `/Users/dadmin/Projects/claude-code-barber`

**Context:** The Agent tool only accepts `sonnet`/`opus`/`haiku` as model tiers, which get mapped to Anthropic model IDs. ccb-route has tier metadata in providers.toml but the resolution logic uses HashMap iteration (non-deterministic) and collapses back to Anthropic. This story adds an explicit `[tier_routing]` table of ranked model IDs per tier so subagents can dispatch to MiniMax, Ollama, or aibox in-session without tmux.

- [ ] story_ccb_029_router_tier_resolution.md — Deterministic tier → model routing for in-session subagent dispatch (29 ACs)

---

## 5 — CCB: Statusline Visual Fixes & Re-wire `@any`

Run from: `/Users/dadmin/Projects/claude-code-barber`

**State (updated 2026-05-28):** PR #8 re-wires the `status` feature flag, commits all status source files, and restructures token display from session/daily rows to input/output rows. ACs 7-8, 10, 15-16, 19-23 done. Remaining: visual bug fixes (AC1-6), subcommands (AC9, 12-14), clippy/CI (AC17-18), add to `full` (AC11).

- [ ] story_ccb_022_statusline_visual_fixes.md — Fix visual bugs, finish subcommands, pass clippy (23 ACs, 10 done via PR #8)

---

## 6 — Copernicus: CI Unblock `@any`

Run from: `/Users/dadmin/Projects/copernicus/copernicus`

**CI State (verified 2026-05-25):** tsc + pytest in pr-validation.yml. 706 is the only remaining work.

- [ ] story_706_ci_pytest_coverage_gate.md — add `--cov` + `--cov-fail-under=80` to pr-validation.yml
- [x] story_707_ci_tsc_zero_error_gate.md — DONE in pr-validation.yml
- [x] story_708_ci_playwright_e2e_on_push.md — DONE in e2e.yml

---

## 7 — Thrift SkyNet: Production Infra

Run from: `/Users/dadmin/Projects/thrift-skynet`

**Wave 1 complete (verified 2026-05-28):** S056, dockerfiles, CI, observability, db_backup, secrets all merged (PRs 1-8). All branches pruned.

### Wave 2 — OAuth + Deploy
- [ ] docs/stories/ready/hph_prod_oauth.md — OAuth wiring to Etsy/eBay/Pinterest (blocked by backend deploy)
- [ ] docs/stories/ready/S057_deploy_production.md — Production deploy (human: PostgreSQL/DNS/SSL)

---

## 8 — Atlas: Sprint 2 `@any`

Run from: `/Users/dadmin/Projects/Atlas`

**State (verified 2026-05-28):** s1_001 through s1_007 + xmcp_000 through xmcp_006 all merged to main. CI workflow active. Remote repo is byteReactr/Atlas-IDE (private, requires `gh auth switch --user byteReactr`).

**Sub-queue:** `Atlas/queue.md` — sync from there for fine-grained status.

### Completed (merged to main)
- [x] atlas_s1_001 — Xcode project via XcodeGen
- [x] atlas_s1_003 — SPM dependencies (GRDB, SwiftTerm, KeychainAccess, swift-tree-sitter)
- [x] atlas_s1_004 — CI workflow
- [x] atlas_s1_005 — Architecture doc
- [x] atlas_s1_006 — @main App entry + WindowGroup
- [x] atlas_s1_007 — 5 test targets
- [x] atlas_xmcp_000 through xmcp_006 — All MCP tools (onboarding, build, test, sim, run, project, sign)

### Ready to start
- [ ] atlas_ce_010_project_wizard.md — Project Wizard (MarrowScript)
- [ ] atlas_ce_014_ccb_settings.md — CCB Integration Settings
- [ ] atlas_ce_015_supply_chain_security.md — Supply Chain Security
- [ ] atlas_ce_001_mcp_serve.md — blocking ce_002 through ce_009

### Human-gated
- [ ] atlas_s1_008_direct_distro.md — Dev ID cert + web hosting
- [ ] atlas_s1_009_payment.md — Stripe/Paddle account

---

## 9 — SHxTLxST: Pre-Ship Blockers

Run from: `/Users/dadmin/Projects/SHxTLxST`

**State (verified 2026-05-28):** Rebrand complete, CI merged, default branch set to main. Remote is byteReactr/SHxTLxST (private, requires `gh auth switch --user byteReactr`). Duplicate lowercase clone removed.

### Complete
- [x] prereq_backend_deploy — Dockerfiles, health endpoint, CORS, rate limiting
- [x] prereq_privacy_policy — policy text + HTML page
- [x] prereq_story_tracking — file reorganization
- [x] S051_shxtlxst_rebrand.md — bundle/code renamed to SHxTLxST
- [x] S055_replace_fastvlm_with_qwopus.md — code changes done
- [x] CI wiring — iOS build workflow merged

### Blocked (backend deploy done → oauth unblocked after human deploy)
- [ ] shxtlxst_prereq_oauth_wiring.md — OAuth routes, deep links (blocked by human deploy of backend)

### Human gates
- [x] shxtlxst_prereq_apple_enrollment.md — $99/yr Individual + Team ID ENROLLED
- [ ] S053_app_store_prep.md — screenshots + ASC entry (blocked by apple certs/bundles)
- [ ] S054_testflight_beta.md — signing + TestFlight (blocked by certs)

---

## 10 — Server & Storage Infrastructure

*All human-gated — ssh access to bizinfra, cable management, drive operations.*

**Source of truth:** `disk-inventory/DELETION_MIGRATION_PLAN.json` (phases 1-3) and `INFRA.md` (bizinfra + aibox specs).

### bizinfra Docker Host (Dell R210ii) — 192.168.1.185
- [x] Linux sudo via docker group
- [ ] Capture container state before any rebuild pass:
  - `docker ps --format "{{.Names}} {{.Image}}"` — inventory all 5 running containers
  - `docker inspect <container> --format '{{json .Mounts}}'` — capture volume mounts
  - `docker exec highplains-db pg_dump -U postgres > /tmp/hph_db_backup.sql` — live DB backup
  - Copy backups to external drive
- [ ] Static IP reservation: 192.168.1.185
- [ ] Deploy SHxTLxST docker-compose.prod.yml on bizinfra
- [ ] Configure nginx: dual-backend routing (Flask + Axum)
- [ ] Cloudflare tunnel: expose SHxTLxST from bizinfra under byteReactr.com subdomain
- [ ] Verify git push/pull works aibox ↔ bizinfra

### Disk Consolidation (disk-inventory/DELETION_MIGRATION_PLAN.json)
- [ ] Phase 1 — Safe deletions (31.9 GB)
- [ ] Phase 2 — Review before delete (50.5 GB)
- [ ] Phase 3 — Server migration prep: scan remaining drives

### Repo Migration to bizinfra
- [ ] Migrate active repos → bizinfra storage
- [ ] Set up git remotes on bizinfra for each repo
- [ ] Verify git push/pull works from aibox → bizinfra

### ML Models → bizinfra
- [ ] Move ML models from local disk to bizinfra
- [ ] Document current model locations
- [ ] Verify model loading works from bizinfra storage path

### Database on bizinfra
- [ ] PostgreSQL backup validation
- [ ] Verify backup restore works
- [ ] Document connection strings

### S3 / Object Storage Evaluation
- [ ] Assess bizinfra capacity for object storage
- [ ] If insufficient: evaluate Cloudflare R2, Backblaze B2, or local disk upgrade
- [ ] Document model serving strategy

---

## Agent Instructions

1. Read QUEUE.md. Pick any unchecked `[ ]` item whose dependencies are met.
2. Open the task file, find `## Working directory`, cd there.
3. Create a feature branch: `git checkout -b feat/<story-slug>`.
4. Complete the task. Commit to the branch.
5. Push and open a PR. Do NOT merge — orchestrator merges after review.
6. Mark `[x]` when PR is open.
7. Report: `DONE: [task-file] — [one-line result] — PR: [url]`. Then pick next eligible item.

If a gate fails: mark `[!]` and note the failure, skip to next eligible item.

## Backends

All `@any` — pick whatever agent is free. All providers route through `ccb-route` at `localhost:9001`.

| Model | Provider | Cost | Best for |
|-------|----------|------|----------|
| claude-sonnet-4-20250514 | Anthropic (OAuth) | Per-token | Orchestration, complex reasoning |
| qwopus3.5-9b-v3 | aibox (local GPU) | Free | Fast local, Copernicus |
| MiniMax-M2.7 | MiniMax via ccb-route | $20/mo flat | Implementation, Rust/Python, CI |
| GLM-5.1-Distill-9B | Ollama via ccb-route | $20/mo flat | Research, analysis, planning |
| deepseek-v4-pro | Ollama via ccb-route | Per-token | Frontier reasoning |

Select via `/model` picker or `claude --model <id>`. Gateway discovery active.

Flags: `--yolo` = skip permissions, `--resume ID` = resume session

---

## Backlog / Explorations

*Discussed but not yet in repo-level story files.*

### MarrowScript — Declarative Backend Compiler
**Source:** https://github.com/Doorman11991/MarrowScript
**Atlas fit:** ce_010 (Project Wizard) is the MarrowScript compiler rebuilt in Swift.

### BoneScript — Declarative Backend Language
**Source:** https://github.com/Doorman11991/BoneScript
**Atlas fit:** 7-stage pipeline as architectural reference for ce_010.

### SmallCode — Local LLM Coding Agent
**Source:** https://github.com/Doorman11991/smallcode
**Atlas fit:** ce_011 (SmallCode Patterns for Qwopus), ce_012 (Budget-Aware CCB).

### Bumblebee — Package Inventory + Supply Chain Scanner
**Source:** https://github.com/perplexityai/bumblebee
**Atlas fit:** ce_015 (Supply Chain Security) — evaluate direct integration before rebuilding.

### Budget-Aware MCP — Token-Budget Graph Retrieval
**Source:** https://github.com/Doorman11991/budget-aware-mcp
**CCB fit:** Direct reference for CCB-016 (graph traversal) — BFS walks with token budget caps. Also relevant to ce_012 (Budget-Aware CCB).

### Hivemind — Shared Memory for AI Coding Agents
**Source:** https://github.com/activeloopai/hivemind
**CCB fit:** Direct reference for CCB-018/019/020 (memory features). Auto-capture traces, mine patterns into SKILL.md, hybrid search (semantic + BM25). Has Claude Code plugin. Decision: integrate as provider or build native (local-first, zero-cost).

### LoCoMo — Long-Term Conversational Memory Benchmark
**Source:** https://arxiv.org/abs/2402.17753
**CCB fit:** Direct reference for CCB-021. 300-turn, 35-session conversations. QA + summarization + multimodal evaluation. Validates compression *quality* not just quantity. Hivemind benchmarked against LoCoMo showing 25% cheaper, 1.7x fewer tokens.

### CodeGraphContext — Full Code Graph Implementation
**Source:** https://github.com/CodeGraphContext/CodeGraphContext
**CCB fit:** Direct reference for CCB-015/016. 20-language tree-sitter parsing, caller/callee edges, import/inherit tracking, dead code detection, cyclomatic complexity. Multiple graph DB backends (KuzuDB, FalkorDB, Neo4j).

### Agentic Loop / RALPH — Autonomous Plan→Build→Verify Reference
**Source:** https://github.com/allierays/agentic-loop
**What it is:** PRD-driven autonomous coding loop for Claude Code. Two-terminal workflow: plan in Terminal 1, RALPH executes in Terminal 2. 5-stage verification pipeline, failure persistence, `/lesson` learning capture.
**CCB fit:** Direct reference for CCB-028 (Sprint 0). Adapted to multi-repo, multi-stack with repo-detected quality gates.

### BMAD-METHOD — Persona Pipeline for Agent Development
**Source:** https://github.com/bmad-code-org/BMAD-METHOD
**What it is:** 12+ specialized agent personas, 34+ workflows, stage-based development lifecycle. AI as expert collaborators at each stage, not autonomous decision-makers. Scale-adaptive depth.
**CCB fit:** Direct reference for CCB-028 pipeline design. Persona-per-stage model (interview→sentinel→architect→coder→tester→validator) adapted for CCB's expert system.

### Agent-Spec — Requirements Traceability & Test Generation
**Source:** https://github.com/RaySmith414/Agent-Spec
**What it is:** CrewAI-based pipeline: Analysis→Requirements→Test Generation. EARS-formatted specs. Type-safe contracts between stages. Full traceability: tests→requirements→stories.
**CCB fit:** Traceability model for CCB-028 validator stage. AC verification against code, gap detection for untested ACs.

### OpenSpec — Lightweight Spec-Driven Development
**Source:** https://github.com/Fission-AI/OpenSpec
**What it is:** Artifact-structured SDD: proposal→spec→design→tasks. Lightweight, iterative. Works with 25+ AI tools via slash commands.
**CCB fit:** Artifact structure for `ccb plan` output. proposal→spec→design→tasks maps to story→plan.json→phases→commits.

### Nango — Unified Auth & API Integration Platform
**Source:** https://github.com/NangoHQ/nango
**What it is:** Open source platform that handles OAuth, API keys, and token refresh for 800+ APIs. Self-hostable. TypeScript. Manages credential lifecycle, retries, rate limiting.
**General fit:** Any time we need authenticated connections to external APIs — MCP servers, GitHub, Cloudflare, Asana, future integrations. One tool for auth instead of per-service wiring.

### Continuity v2 — Haustorium12 Project
**Source:** https://github.com/Haustorium12/continuity-v2
**Status:** Not yet reviewed — needs read and summary.

### Context Authority — Karpathy Knowledge Base (DECIDED, Sprint 6)
- Unified weighted knowledge graph over ALL context sources: code, experts, skills, tools, MCP servers, CLAUDE.md
- Replaces bulk prompt injection with per-turn weighted retrieval
- Self-tunes via session traces + LoCoMo validation gate
- Gap detection: identifies unused-but-important knowledge, suggests new skills/experts
- CLAUDE.md decomposition: section-level retrieval instead of full-file injection
- Sprint 6 stories: CCB-024, 025, 026, 027, 028 (152 total ACs)
- Dashboard (027) provides observability: what's indexed, what's injected, what's missing, what's costing tokens
- Plan→Build→Verify loop (028) provides repo-detected sprint execution with failure persistence and quality gates

### CCB + YASR Integration — DECIDED
- YASR ported into CCB as `src/features/status/` (Rust). Integration done, visual bugs remain.
- Standalone release NOT needed — CCB owns this now. Delete `~/Projects/yasr` after status feature ships.

### Atlas Futures (not yet in sub-queue)
- `atlas_se_013_neural_drift.md` — Kinetix neural drift detection
- `atlas_xmcp_008_submit` — TestFlight upload (blocked until certs)
- Atlas direct distro: Sparkle auto-update infrastructure

### HP ProLiant ML350 G9 Commissioning
- Dual E5-2696 CPU, 700GB DDR4, up to 4x GPU
- Stages 1-3 planned: RAM equalization → CPU upgrade → power backplane → 4 GPUs
- ~$150 for server + 3x RTX 3060 already purchased
- Biggest gate: power backplane
- When online: multi-model inference cluster, LoRA training, vision pipeline, large-context RAG
