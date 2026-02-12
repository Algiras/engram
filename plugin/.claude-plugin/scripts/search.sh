#!/bin/bash
# Search for knowledge packs

set -e

if [ $# -lt 1 ]; then
    echo "Usage: $0 <search-query>"
    exit 1
fi

QUERY="$*"

echo "🔎 Searching for: $QUERY"
echo ""

if ! command -v claude-memory &> /dev/null; then
    echo "❌ claude-memory not found in PATH"
    echo "   Install: cargo install --path /path/to/claude-memory"
    exit 1
fi

claude-memory hive search "$QUERY"
