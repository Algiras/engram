# Hive Knowledge Skill

Distributed knowledge sharing for claude-memory via the Hive Mind system.

## Installation

### Via skills.sh (when published)

```bash
npx skills add claude-memory/hive-knowledge
```

### Manual Installation

```bash
# Clone
git clone https://github.com/Algiras/claude-memory
cd claude-memory

# Copy to Claude skills directory
cp -r skills/hive-knowledge ~/.claude/skills/
```

## What It Does

This skill teaches Claude Code how to:
- Manage Git-based knowledge registries
- Discover and install knowledge packs
- Share knowledge across teams
- Use the Hive Mind distributed memory system

## Prerequisites

**Required:** claude-memory CLI tool

```bash
# Install claude-memory
cargo install --git https://github.com/Algiras/claude-memory

# Verify
claude-memory --version
```

## Quick Start

```bash
# Add the core registry
claude-memory hive registry add Algiras/claude-memory

# Browse available packs
claude-memory hive browse

# Install the meta-knowledge pack
claude-memory hive install claude-memory-core

# Use the knowledge
claude-memory recall <your-project>
```

## Features

- 🔍 **Discovery**: Browse and search knowledge packs
- 📦 **Installation**: One-command pack installation
- 🔄 **Updates**: Keep packs up to date
- 🎯 **Integration**: Automatic recall/search integration
- 🔒 **Privacy**: Only extracted knowledge shared
- 📊 **TUI**: Interactive pack browser

## Documentation

See `SKILL.md` for complete documentation including:
- All hive commands
- Creating knowledge packs
- Publishing workflows
- Troubleshooting guide
- Best practices

## License

MIT

## Learn More

- Main Repository: https://github.com/Algiras/claude-memory
- Core Registry: https://github.com/Algiras/claude-memory/tree/master/registry
- Example Pack: claude-memory-core (meta-knowledge)
