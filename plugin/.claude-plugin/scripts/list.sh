#!/bin/bash
# List installed knowledge packs

set -e

echo "📚 Installed knowledge packs:"
echo ""

if ! command -v engram &> /dev/null; then
    echo "❌ engram not found in PATH"
    echo "   Install: cargo install --path /path/to/engram"
    exit 1
fi

engram hive list
