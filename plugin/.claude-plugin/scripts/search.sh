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

if ! command -v engram &> /dev/null; then
    echo "❌ engram not found in PATH"
    echo "   Install: cargo install --path /path/to/engram"
    exit 1
fi

engram hive search "$QUERY"
