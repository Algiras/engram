#!/bin/bash
# List installed knowledge packs

set -e

echo "📚 Installed knowledge packs:"
echo ""

if ! command -v claude-memory &> /dev/null; then
    echo "❌ claude-memory not found in PATH"
    echo "   Install: cargo install --path /path/to/claude-memory"
    exit 1
fi

claude-memory hive list
