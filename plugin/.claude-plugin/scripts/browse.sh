#!/bin/bash
# Browse available knowledge packs

set -e

echo "🔍 Browsing available knowledge packs..."
echo ""

if ! command -v engram &> /dev/null; then
    echo "❌ engram not found in PATH"
    echo "   Install: cargo install --path /path/to/engram"
    exit 1
fi

# Browse packs with optional filters
if [ $# -eq 0 ]; then
    engram hive browse
elif [ "$1" = "--category" ]; then
    engram hive browse --category "$2"
elif [ "$1" = "--keyword" ]; then
    engram hive browse --keyword "$2"
else
    # Assume it's a search query
    engram hive search "$*"
fi
