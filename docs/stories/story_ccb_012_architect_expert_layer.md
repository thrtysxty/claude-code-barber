# Story 733 — CCB Architect Expert Layer (AWS WAF + Kubernetes + Terraform)

**Status:** READY
**Type:** Feature
**Epic:** CCB Knowledge Graph
**Priority:** P2
**Blocks:** —
**Blocked By:** —

## Context

CCB has 5 expert personas: sentinel, coder, architect, debugger, selector. The architect persona has no
dataset — minimax confirmed no suitable HuggingFace dataset exists under 500MB. Generate the dataset from
training knowledge across three domains: AWS Well-Architected Framework (5 pillars), Kubernetes/container
orchestration, and Terraform/IaC best practices. These map directly to the Copernicus and hph-server infra.

The expert ingest format is JSON:
```json
{
  "persona": "architect",
  "description": "...",
  "domains": [
    {
      "name": "domain-name",
      "category": "category",
      "patterns": [
        { "id": "ARCH-001", "name": "Pattern Name", "mitigations": ["step 1", "step 2"] }
      ]
    }
  ]
}
```

Command to ingest: `ccb expert build architect --dataset data/architect.json`
Working directory for CCB: `/Users/dadmin/Projects/claude-code-barber`

## Acceptance Criteria

1. Generate the architect expert dataset from training knowledge — no external fetching required. Cover
   these three domains:
   - **AWS Well-Architected Framework**: all 5 pillars (operational excellence, security, reliability,
     performance efficiency, cost optimization) — at least 5 patterns per pillar
   - **Kubernetes / container orchestration**: pod design, resource limits, health probes, RBAC, namespace
     isolation, rolling deployments, PodDisruptionBudgets — at least 8 patterns
   - **Terraform / IaC**: module structure, remote state, workspace isolation, input validation, provider
     pinning, drift detection, least-privilege IAM — at least 8 patterns
   - **Local AI Infra**: GPU inference server setup (llama-server/CUDA), Anthropic-compatible API proxy
     routing, local model lifecycle (download → quantize → serve → monitor), Docker deployment for
     Flask/React apps, tmux-based agent orchestration, aibox ↔ orchestrator network topology — at least
     8 patterns drawn from this specific stack

2. Extract ≥48 architecture patterns total across the 4 domains. Each pattern must have:
   - A unique `id` (format: `ARCH-NNN`)
   - A descriptive `name`
   - 2–4 concrete `mitigations` (actionable steps, not vague advice)

3. Write the dataset to `/Users/dadmin/Projects/claude-code-barber/data/architect.json` — valid JSON,
   matching the schema above exactly.

4. Run `ccb expert build architect --dataset data/architect.json` from
   `/Users/dadmin/Projects/claude-code-barber` and confirm it exits 0.

5. Run `ccb expert query architect "single point of failure"` and confirm it returns at least one pattern.

## Gate

```bash
cd /Users/dadmin/Projects/claude-code-barber
ccb expert build architect --dataset data/architect.json && echo "INGEST OK"
ccb expert query architect "single point of failure" | grep -c "ARCH-" | grep -v "^0$" && echo "QUERY OK"
```

## Notes

- `ccb` binary is at `~/.local/bin/ccb` — may need `~/.local/bin/ccb` if not in PATH
- CCB must be built with `--features expert` (check with `ccb --version`)
- If `ccb expert` subcommand is missing, build first: `cargo build --features expert` from CCB dir
- Focus on concrete, actionable patterns — not abstract principles
