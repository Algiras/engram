#!/bin/bash
# engram SessionStart hook
# Injects project knowledge into Claude Code's memory on session start
#
# CLAUDE_PROJECT_DIR is set by Claude Code hooks (the project directory path)

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"
PROJECT_NAME="$(basename "$PROJECT_DIR")"

[ -z "$PROJECT_NAME" ] && exit 0

engram inject "$PROJECT_NAME" >/dev/null 2>&1

exit 0
