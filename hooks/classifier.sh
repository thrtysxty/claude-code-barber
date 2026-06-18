#!/usr/bin/env bash
# ccb classifier PreToolUse hook — two-tier safety classification
#
# Tier 1: instant local pattern matching (no API call)
# Tier 2: sends ambiguous actions to an LLM for evaluation (via OpenRouter)
#
# Install in ~/.claude/settings.json:
#   "hooks": {
#     "PreToolUse": [{
#       "matcher": ".*",
#       "hooks": [{ "type": "command", "command": "~/.claude/hooks/classifier.sh" }]
#     }]
#   }

set -euo pipefail

CCB_BIN="${CCB_BIN:-${HOME}/.local/bin/ccb}"
command -v "$CCB_BIN" &>/dev/null || {
    echo "ccb not found at $CCB_BIN — install with: cargo install --path . --features classify" >&2
    exit 1
}

exec "$CCB_BIN" classify
