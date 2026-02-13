# Hive Knowledge Plugin for Claude Code

Distributed knowledge sharing via the Hive Mind system.

## Installation

### From Directory

```bash
# Clone or copy this plugin
cp -r /path/to/hive-plugin ~/.claude/plugins/hive-knowledge

# Or use Claude Code to install
claude plugin add /path/to/hive-plugin
```

### From GitHub (when published)

```bash
claude plugin add anthropics/hive-knowledge-plugin
```

## Quick Start

1. **Install engram:**
```bash
cargo install --path /path/to/engram
```

2. **Add a registry:**
```bash
engram hive registry add Algiras/engram-registry
```

3. **Browse and install packs:**
```bash
/hive-browse
/hive-install engram-core
```

## Features

- 🔍 Browse knowledge packs from registries
- 📦 Install packs with one command
- 🔎 Search across available packs
- 📚 List installed packs
- 🎯 Automatic integration with recall/search

## Commands

- `/hive-browse` - Browse available packs
- `/hive-install <name>` - Install a pack
- `/hive-list` - Show installed packs
- `/hive-search <query>` - Search for packs

## Documentation

See `plugin.md` and `skills/hive-knowledge.md` for complete documentation.

## License

MIT
