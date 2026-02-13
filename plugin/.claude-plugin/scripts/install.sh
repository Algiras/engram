#!/bin/bash
# Install a knowledge pack

set -e

if [ $# -lt 1 ]; then
    echo "Usage: $0 <pack-name> [--registry <name>]"
    exit 1
fi

PACK_NAME="$1"
shift

echo "📦 Installing knowledge pack: $PACK_NAME"
echo ""

if ! command -v engram &> /dev/null; then
    echo "❌ engram not found in PATH"
    echo "   Install: cargo install --path /path/to/engram"
    exit 1
fi

# Install the pack
if engram hive install "$PACK_NAME" "$@"; then
    echo ""
    echo "✅ Pack installed successfully!"
    echo ""
    echo "💡 Access the knowledge with:"
    echo "   engram recall <project>"
fi
