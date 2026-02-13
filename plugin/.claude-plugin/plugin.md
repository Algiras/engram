# Hive Knowledge Plugin

Distributed knowledge sharing for engram via the Hive Mind system.

## Overview

This plugin provides easy access to the Hive Mind knowledge sharing system, allowing you to discover, install, and use knowledge packs from Git-based registries.

## Commands

### `/hive-browse`
Browse all available knowledge packs across all configured registries.

**Options:**
- `--category <category>` - Filter by category (patterns, solutions, decisions, workflows, preferences)
- `--keyword <keyword>` - Filter by keyword

**Examples:**
```bash
/hive-browse
/hive-browse --category patterns
/hive-browse --keyword rust
```

### `/hive-install <pack-name>`
Install a knowledge pack from a registry.

**Arguments:**
- `pack-name` - Name of the pack to install

**Options:**
- `--registry <name>` - Install from specific registry

**Examples:**
```bash
/hive-install rust-patterns
/hive-install typescript-guide --registry official
```

### `/hive-list`
List all installed knowledge packs with metadata.

**Example:**
```bash
/hive-list
```

### `/hive-search <query>`
Search for knowledge packs by keyword or description.

**Arguments:**
- `query` - Search query

**Examples:**
```bash
/hive-search "async patterns"
/hive-search rust
```

## Prerequisites

1. **Install engram:**
```bash
cargo install --path /path/to/engram
```

2. **Configure a registry:**
```bash
# Add official registry
engram hive registry add Algiras/engram-registry

# Or add local registry
engram hive registry add file:///path/to/registry
```

## Getting Started

1. **Browse available packs:**
```bash
/hive-browse
```

2. **Install a pack:**
```bash
/hive-install engram-core
```

3. **Use the knowledge:**
The installed pack's knowledge is now automatically included when you use:
- `engram recall <project>`
- `engram search <query>`
- `engram lookup <project> <topic>`

## Creating Your Own Packs

See the hive-knowledge skill for detailed instructions on creating and publishing knowledge packs.

## Integration

This plugin integrates with:
- **engram CLI** - All hive commands
- **TUI** - Press 'p' in TUI to browse packs
- **Recall system** - Installed packs automatically included

## Support

- Repository: https://github.com/Algiras/engram
- Issues: https://github.com/Algiras/engram/issues
- Documentation: See hive-knowledge skill

## License

MIT
