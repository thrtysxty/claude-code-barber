#!/usr/bin/env bash
# ccb expert PreToolUse hook — surfaces domain knowledge to Claude Code
#
# When Claude invokes a tool (Read, Bash, etc.), this script queries the
# active persona for relevant domains and patterns. The JSON output is
# consumed by Claude Code's hook system and injected as structured data.
#
# Install in ~/.claude/settings.json:
#   "hooks": {
#     "PreToolUse": [{
#       "matcher": { "tool_name": ".*" },
#       "hooks": [{ "type": "command", "command": "~/.local/bin/ccb expert query --format json" }]
#     }]
#   }

set -euo pipefail

CCB_BIN="${CCB_BIN:-${HOME}/.local/bin/ccb}"
command -v "$CCB_BIN" &>/dev/null || {
    echo "ccb not found at $CCB_BIN — install with: cargo install --path . --features expert" >&2
    exit 1
}

exec "$CCB_BIN" expert query --tool "${TOOL_NAME:-}" --format json
