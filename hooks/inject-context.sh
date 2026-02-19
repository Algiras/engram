#!/bin/bash
# engram SessionStart hook
# Injects project knowledge into Claude Code's memory on session start
# Also ensures the daemon is running (auto-starts if not).
#
# CLAUDE_PROJECT_DIR is set by Claude Code hooks (the project directory path)

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(pwd)}"
PROJECT_NAME="$(basename "$PROJECT_DIR")"

[ -z "$PROJECT_NAME" ] && exit 0

engram inject "$PROJECT_NAME" >/dev/null 2>&1

# Auto-start daemon if not running
DAEMON_PID_FILE="${HOME}/memory/daemon.pid"
_daemon_running=false
if [ -f "$DAEMON_PID_FILE" ]; then
    _pid="$(cat "$DAEMON_PID_FILE" 2>/dev/null)"
    if [ -n "$_pid" ] && kill -0 "$_pid" 2>/dev/null; then
        _daemon_running=true
    fi
fi

if [ "$_daemon_running" = false ]; then
    engram daemon start >/dev/null 2>&1 &
fi

exit 0
