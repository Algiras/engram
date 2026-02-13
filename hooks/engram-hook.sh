#!/bin/bash
# engram PostToolUse hook
# Spawns a background ingest of recent conversations (debounced)
#
# To install, add to ~/.claude/settings.json:
# {
#   "hooks": {
#     "PostToolUse": [
#       { "matcher": "", "hooks": [{ "type": "command", "command": "/path/to/engram-hook.sh" }] }
#     ]
#   }
# }

LOCKFILE="/tmp/engram-hook.lock"
DEBOUNCE_SECONDS=300  # 5 minutes

# Quick exit if lock exists and is recent (debounce)
if [ -f "$LOCKFILE" ]; then
    LOCK_AGE=$(( $(date +%s) - $(stat -f %m "$LOCKFILE" 2>/dev/null || echo 0) ))
    if [ "$LOCK_AGE" -lt "$DEBOUNCE_SECONDS" ]; then
        exit 0
    fi
fi

# Create/touch lockfile
touch "$LOCKFILE"

# Run ingest in background (archive only, no LLM — fast and silent)
engram ingest --skip-knowledge --since 5m >/dev/null 2>&1 &

exit 0
