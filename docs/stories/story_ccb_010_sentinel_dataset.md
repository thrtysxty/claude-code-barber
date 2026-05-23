# CCB Story 010: Sentinel Knowledge Dataset

**Status:** READY
**Priority:** P1 — first real expert knowledge
**Sprint:** CCB-2 (Expert Knowledge)

## Narrative
**As a** developer using CCB with the `sentinel` expert active,
**I want** a curated OWASP security knowledge dataset shipped with CCB,
**So that** security patterns surface in hook output without me having to author the knowledge base myself.

## Context

The sentinel expert covers OWASP Top 10 and common CWE patterns. This dataset is the first real knowledge in the system — it makes the expert graph useful out of the box. It ships as a YAML file at `datasets/sentinel.yaml` in the repo, and gets installed to `~/.claude/experts/sentinel.yaml` via `ccb style install` (future) or manually.

## Acceptance Criteria

### Dataset File
1. `datasets/sentinel.yaml` created in repo root with the sentinel schema (see CCB-009).
2. Covers at minimum these domains and patterns:

**path_traversal** (CWE-22):
- Mitigations: resolve-then-check-root, reject `..` segments, allowlist directories

**sql_injection** (CWE-89):
- Mitigations: parameterized queries, allowlist validation, least-privilege DB role

**xss** (CWE-79):
- Mitigations: output encoding (HTML/JS/URL context-aware), CSP headers, avoid `innerHTML`

**command_injection** (CWE-78):
- Mitigations: avoid shell=True / execSync with user input, use arg arrays, allowlist commands

**insecure_deserialization** (CWE-502):
- Mitigations: avoid pickle/yaml.unsafe_load/eval on external data, validate before deserializing

**secrets_in_code** (CWE-798):
- Mitigations: use env vars or secret manager, never hardcode, gitignore .env, rotate on exposure

**ssrf** (CWE-918):
- Mitigations: allowlist outbound URLs, block internal ranges (169.254/10.0/172.16/192.168), validate scheme

3. Each pattern has 3–5 concise mitigations (one action per line, imperative form).
4. YAML is valid — `python3 -c "import yaml; yaml.safe_load(open('datasets/sentinel.yaml'))"` passes.

### Installation
5. `README` or `ccb expert ingest` help text references `datasets/sentinel.yaml` as the bundled dataset path.

### Smoke Test
6. `ccb expert activate sentinel && ccb expert ingest sentinel datasets/sentinel.yaml && ccb expert query --format json | python3 -m json.tool` — passes, output contains all 7 domains.

## Files in Scope
- `datasets/sentinel.yaml` (new)

## Blocked By
- CCB-009 (ingest command must exist)

## Blocks
- None — dataset is standalone

## Definition of Done
- [ ] `datasets/sentinel.yaml` exists with 7 domains, valid YAML
- [ ] Smoke test 6 passes end-to-end
- [ ] JSON output contains all 7 domain names
