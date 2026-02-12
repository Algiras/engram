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

if ! command -v claude-memory &> /dev/null; then
    echo "❌ claude-memory not found in PATH"
    echo "   Install: cargo install --path /path/to/claude-memory"
    exit 1
fi

# Install the pack
if claude-memory hive install "$PACK_NAME" "$@"; then
    echo ""
    echo "✅ Pack installed successfully!"
    echo ""
    echo "💡 Access the knowledge with:"
    echo "   claude-memory recall <project>"
fi
