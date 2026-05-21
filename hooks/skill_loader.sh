#!/usr/bin/env bash
# ccb skill loader hook — PreToolUse on Skill tool
# When Claude invokes a /skill, this hook reads SKILL.md via `ccb fade`
# and returns the content as hook feedback (lazy context injection).
#
# Install in ~/.claude/settings.json:
#   "hooks": { "PreToolUse": [{"matcher": {"tool_name": "Skill"}, "hooks": [{"type": "command", "command": "~/.claude/hooks/skill_loader.sh"}]}] }

set -euo pipefail

TOOL_NAME="${TOOL_NAME:-}"
TOOL_INPUT="${TOOL_INPUT:-}"

[[ "$TOOL_NAME" == "Skill" ]] || exit 0
[[ -n "$TOOL_INPUT" ]] || exit 0

SKILL_NAME=$(python3 -c "import sys,json; d=json.loads('$TOOL_INPUT'); print(d.get('skill',''))" 2>/dev/null || echo "")
[[ -n "$SKILL_NAME" ]] || exit 0

CCB_BIN="${CCB_BIN:-ccb}"
command -v "$CCB_BIN" &>/dev/null || exit 0

"$CCB_BIN" fade "$SKILL_NAME" 2>/dev/null || true
