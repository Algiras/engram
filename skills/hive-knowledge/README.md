# Hive Knowledge Skill

Distributed knowledge sharing for engram via the Hive Mind system.

## Installation

### Via skills.sh (when published)

```bash
npx skills add engram/hive-knowledge
```

### Manual Installation

```bash
# Clone
git clone https://github.com/Algiras/engram
cd engram

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

**Required:** engram CLI tool

```bash
# Install engram (auto-detects OS/arch)
curl -fsSL https://raw.githubusercontent.com/Algiras/engram/master/install.sh | sh

# Optional: pin a version
curl -fsSL https://raw.githubusercontent.com/Algiras/engram/master/install.sh | VERSION=v0.3.0 sh

# Fallback: source install
# cargo install --git https://github.com/Algiras/engram

# Verify
engram --version
```

## Quick Start

```bash
# Add the core registry
engram hive registry add Algiras/engram-registry

# Browse available packs
engram hive browse

# Install the meta-knowledge pack
engram hive install engram-core

# Use the knowledge
engram recall <your-project>
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

- Main Repository: https://github.com/Algiras/engram
- Example Registry: https://github.com/Algiras/engram/tree/master/examples/registry
- Example Pack: engram-core (meta-knowledge)
