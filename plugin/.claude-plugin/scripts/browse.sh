#!/bin/bash
# Browse available knowledge packs

set -e

echo "🔍 Browsing available knowledge packs..."
echo ""

if ! command -v claude-memory &> /dev/null; then
    echo "❌ claude-memory not found in PATH"
    echo "   Install: cargo install --path /path/to/claude-memory"
    exit 1
fi

# Browse packs with optional filters
if [ $# -eq 0 ]; then
    claude-memory hive browse
elif [ "$1" = "--category" ]; then
    claude-memory hive browse --category "$2"
elif [ "$1" = "--keyword" ]; then
    claude-memory hive browse --keyword "$2"
else
    # Assume it's a search query
    claude-memory hive search "$*"
fi
