#!/usr/bin/env bash
# ccb context monitor hook — PostToolUse
# Checks context window usage after each tool call and warns on threshold breach.
#
# Install in ~/.claude/settings.json:
#   "hooks": { "PostToolUse": [{"hooks": [{"type": "command", "command": "~/.claude/hooks/context_monitor.sh"}]}] }
#
# Set threshold env vars or defaults apply (compact=70, clear=85):

set -euo pipefail

CCB_BIN="${CCB_BIN:-ccb}"
command -v "$CCB_BIN" &>/dev/null || exit 0

COMPACT_THRESHOLD="${CCB_COMPACT_THRESHOLD:-70}"
CLEAR_THRESHOLD="${CCB_CLEAR_THRESHOLD:-85}"

"$CCB_BIN" context compact "$COMPACT_THRESHOLD" 2>/dev/null || true
"$CCB_BIN" context clear   "$CLEAR_THRESHOLD"   2>/dev/null || true
