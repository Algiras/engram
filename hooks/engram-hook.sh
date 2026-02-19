#!/bin/bash
# engram PostToolUse hook
# 1. Fast path: pipe tool data to engram observe (non-blocking, no LLM)
# 2. Debounced ingest: archive recent conversations every 5 minutes
#
# To install, run: engram hooks install

# Fast path: capture observation (reads stdin, exits immediately)
# We tee stdin so the debounced ingest can still proceed
STDIN_DATA=$(cat)

echo "$STDIN_DATA" | engram observe >/dev/null 2>&1 &

# Debounced ingest
LOCKFILE="/tmp/engram-hook.lock"
DEBOUNCE_SECONDS=300  # 5 minutes

if [ -f "$LOCKFILE" ]; then
    LOCK_AGE=$(( $(date +%s) - $(stat -f %m "$LOCKFILE" 2>/dev/null || echo 0) ))
    if [ "$LOCK_AGE" -lt "$DEBOUNCE_SECONDS" ]; then
        exit 0
    fi
fi

touch "$LOCKFILE"

# Run ingest in background (archive only, no LLM — fast and silent)
engram ingest --skip-knowledge --since 5m >/dev/null 2>&1 &

exit 0
