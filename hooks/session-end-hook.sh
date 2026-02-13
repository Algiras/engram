#!/bin/bash
# engram SessionEnd hook
# Runs full knowledge extraction + context regeneration for the current project
# Fires when a Claude Code session terminates (clear, logout, exit)
#
# The next SessionStart will inject the freshly generated context.

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"
PROJECT_NAME="$(basename "$PROJECT_DIR")"
[ -z "$PROJECT_NAME" ] && exit 0

# Full ingest of recent sessions (with LLM extraction, runs in background)
engram ingest --project "$PROJECT_NAME" --since 1d >/dev/null 2>&1 &

exit 0
