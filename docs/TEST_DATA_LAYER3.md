# Test Data — CCB Layer 3 (Expert/Persona Graph)

This file captures real inputs and outputs for expert commands and the graph walk
traversal. Used for regression testing, hook integration validation, and
verifying JSON output is always valid.

---

## `fixture_expert_build`

**Command:** `ccb expert build sentinel --dataset <path>`

**Input** (sentinel.json):
```json
{
  "persona": "sentinel",
  "description": "Security domain expert",
  "domains": [
    {
      "name": "path_traversal",
      "category": "security",
      "patterns": [
        {"id": "CWE-22", "name": "Path Traversal", "mitigations": ["validate input", "resolve then check root"]},
        {"id": "CWE-36", "name": "Absolute Path Traversal", "mitigations": ["canonicalize path before validation"]}
      ]
    },
    {
      "name": "sql_injection",
      "category": "security",
      "patterns": [
        {"id": "CWE-89", "name": "SQL Injection", "mitigations": ["parameterized queries", "allowlist validation"]}
      ]
    }
  ]
}
```

**Output:**
```
Built persona 'sentinel' with 2 domain(s)
```

**Exit code:** 0

---

## `fixture_expert_query_no_persona`

**Command:** `ccb expert query` (no active persona)

**Output:**
```
{}
```

**Exit code:** 0

---

## `fixture_expert_query_with_persona`

**Command:** `ccb expert query` (sentinel activated)

**Output:**
```json
{
  "active_domains": ["path_traversal", "sql_injection"],
  "patterns": [
    {"id": "CWE-22", "mitigations": ["validate input", "resolve then check root"], "name": "Path Traversal"},
    {"id": "CWE-36", "mitigations": ["canonicalize path before validation"], "name": "Absolute Path Traversal"},
    {"id": "CWE-89", "mitigations": ["parameterized queries", "allowlist validation"], "name": "SQL Injection"}
  ],
  "persona": "sentinel"
}
```

**Token estimate (raw JSON):** ~200 chars → 50 tokens

**Exit code:** 0

**Validation:** `python3 -m json.tool` passes on all output.

---

## `fixture_expert_walk`

**Command:** `ccb expert walk "file read security check"`

**Precondition:** sentinel persona active, 2 domains loaded as above.

**Output:**
```
Walk: file read security check
Active persona: sentinel
Threshold: 0.50

DOMAIN                        CATEGORY   WEIGHT
--------------------------------------------------
path_traversal                security     1.00
sql_injection                 security     1.00

Patterns:
  [CWE-22] Path Traversal (path_traversal)
  [CWE-36] Absolute Path Traversal (path_traversal)
  [CWE-89] SQL Injection (sql_injection)
```

**Exit code:** 0

---

## `fixture_expert_query_human_format`

**Command:** `ccb expert query --format human` (sentinel active)

**Output:**
```
Persona: sentinel
Domains (2)
  - path_traversal
  - sql_injection
Patterns (3)
  [CWE-22] Path Traversal
      - validate input
      - resolve then check root
  [CWE-36] Absolute Path Traversal
      - canonicalize path before validation
  [CWE-89] SQL Injection
      - parameterized queries
      - allowlist validation
```

**Exit code:** 0

---

## `fixture_expert_list_empty`

**Command:** `ccb expert list` (fresh DB, no personas)

**Output:**
```
No personas registered. Run 'ccb expert build' first.
```

**Exit code:** 0

---

## `fixture_expert_list_with_persona`

**Command:** `ccb expert list` (sentinel active)

**Output:**
```
PERSONA              DOMAINS  STATUS
------------------------------------------------
sentinel                  2  ACTIVE
```

**Exit code:** 0

---

## Hook integration — `expert_pretooluse.sh`

The hook script at `hooks/expert_pretooluse.sh` wraps `ccb expert query`.

**Hook input** (Claude Code PreToolUse env):
```
TOOL_NAME=Read
TOOL_INPUT={"path": "/Users/admin/project/src/main.rs"}
```

**Hook output** (when sentinel is active):
```json
{
  "persona": "sentinel",
  "active_domains": ["path_traversal", "sql_injection"],
  "patterns": [...]
}
```

**Hook output** (when no active persona — silent no-op):
```
{}
```

Exit codes match `ccb expert query`:
- 0: always succeeds (no active persona → `{}`)
- non-zero: binary not found or DB error

**Install snippet for `~/.claude/settings.json`:**
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": { "tool_name": ".*" },
      "hooks": [{ "type": "command", "command": "~/.local/bin/ccb expert query --format json" }]
    }]
  }
}
```

---

## Schema (graph.db — Layer 3 tables)

These tables coexist with the Layer 2 code graph tables in `~/.cache/ccb/graph.db`.

```sql
CREATE TABLE IF NOT EXISTS personas (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS domains (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    category    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS persona_domains (
    persona_id  INTEGER NOT NULL REFERENCES personas(id) ON DELETE CASCADE,
    domain_id   INTEGER NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    weight      REAL NOT NULL DEFAULT 1.0,
    PRIMARY KEY (persona_id, domain_id)
);

CREATE TABLE IF NOT EXISTS patterns (
    id          INTEGER PRIMARY KEY,
    domain_id   INTEGER NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
    pattern_id  TEXT NOT NULL,
    name        TEXT NOT NULL,
    mitigations TEXT NOT NULL,
    UNIQUE(domain_id, pattern_id)
);

CREATE TABLE IF NOT EXISTS active_persona (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    persona_id  INTEGER REFERENCES personas(id)
);

CREATE INDEX IF NOT EXISTS idx_persona_domains ON persona_domains(persona_id);
CREATE INDEX IF NOT EXISTS idx_patterns_domain  ON patterns(domain_id);
```

---

## Token Estimation for Layer 3

Layer 3 outputs are structured JSON — much lower token density than prose.

| Command | Output size | Tokens (ceil/4) |
|---------|-------------|-----------------|
| `expert query` (no persona) | 2 bytes `{}` | 1 |
| `expert query` (sentinel) | ~350 bytes | 88 |
| `expert walk` | ~300 bytes | 75 |
| `expert list` | ~50 bytes | 13 |

Compared to Layer 1 (trim/fade): Layer 3 outputs are ~10x smaller because they
are typed data, not prose. The hook injects raw JSON — zero formatting overhead.

---

## Pattern: JSON output validation

Every `ccb expert query` call must produce valid JSON regardless of state:

```bash
ccb expert query --format json | python3 -m json.tool > /dev/null
```

This is enforced by the smoke tests. The no-persona case returns `{}` which
is valid JSON — the hook never fails, it just has nothing to inject.