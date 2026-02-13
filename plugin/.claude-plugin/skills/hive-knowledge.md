# Hive Knowledge - Distributed Knowledge Sharing

Manage and share knowledge packs via the Hive Mind system.

## What is Hive Mind?

The Hive Mind is a Git-based distributed knowledge sharing system for engram. It allows users to:

- **Discover** knowledge packs from registries
- **Install** packs to access shared knowledge
- **Share** their own knowledge via packs
- **Collaborate** on collective knowledge bases

## Core Concepts

### Registry
A Git repository containing one or more knowledge packs. Similar to npm/cargo registries.

### Knowledge Pack
A structured directory containing:
- `.pack/manifest.json` - Pack metadata
- `knowledge/` - Extracted knowledge files (patterns, solutions, workflows)
- `graph/` - Optional knowledge graph
- `README.md` - Documentation

### Privacy Model
- Raw conversations NEVER leave your machine
- Only extracted, redacted knowledge is shareable
- Privacy controls in pack manifest

## Commands

### Registry Management

```bash
# Add a registry (supports GitHub shorthand)
engram hive registry add owner/repo
engram hive registry add https://github.com/owner/repo

# List registries
engram hive registry list

# Update registry (git pull)
engram hive registry update [name]

# Remove registry
engram hive registry remove <name>
```

### Pack Discovery

```bash
# Browse all available packs
engram hive browse

# Filter by category
engram hive browse --category patterns

# Filter by keyword
engram hive browse --keyword rust

# Search packs
engram hive search "async patterns"
```

### Pack Management

```bash
# Install a pack
engram hive install <pack-name>

# Install from specific registry
engram hive install <pack-name> --registry <registry-name>

# List installed packs
engram hive list

# Update packs
engram hive update              # Update all
engram hive update <pack-name>  # Update specific

# Uninstall pack
engram hive uninstall <pack-name>
```

### Integration with Existing Commands

Once packs are installed, their knowledge is automatically included:

```bash
# Recall includes both local and pack knowledge
engram recall <project>

# Search across local and packs
engram search "pattern"

# Lookup in local and packs
engram lookup <project> "topic"

# TUI: Press 'p' to browse packs
engram tui
```

## Creating a Knowledge Pack

1. **Create pack structure:**
```bash
mkdir -p my-pack/{.pack,knowledge}
```

2. **Create manifest:**
```json
{
  "name": "my-pack",
  "version": "1.0.0",
  "description": "My knowledge pack",
  "author": {"name": "Your Name"},
  "license": "MIT",
  "keywords": ["keyword1", "keyword2"],
  "categories": ["Patterns", "Solutions"],
  "repository": "https://github.com/user/my-pack",
  "created_at": "2026-02-12T00:00:00Z",
  "updated_at": "2026-02-12T00:00:00Z",
  "privacy": {
    "share_patterns": true,
    "share_solutions": true,
    "share_decisions": false,
    "share_preferences": false,
    "redact_secrets": true,
    "require_review": true
  },
  "min_engram_version": "0.1.0"
}
```

3. **Add knowledge files:**
```bash
# Copy from your local knowledge
cp ~/memory/knowledge/<project>/patterns.md my-pack/knowledge/
cp ~/memory/knowledge/<project>/solutions.md my-pack/knowledge/

# Review and redact sensitive information
vim my-pack/knowledge/*.md
```

4. **Publish to Git:**
```bash
cd my-pack
git init
git add .
git commit -m "Initial pack"
git remote add origin https://github.com/user/my-pack
git push -u origin main
```

5. **Share the registry URL:**
Others can now install: `engram hive registry add user/my-pack`

## Example: Installing Core Pack

The engram repository includes a core registry with meta-knowledge:

```bash
# Add the core registry (from local repo)
cd /path/to/engram
engram hive registry add file://$(pwd)/examples/registry

# Or from GitHub (once published)
engram hive registry add Algiras/engram-registry

# Browse available packs
engram hive browse
# Shows: engram-core

# Install it
engram hive install engram-core

# Use the knowledge
engram recall <your-project>
# Now includes patterns, solutions, and workflows about engram itself!
```

## TUI Integration

The TUI includes a Packs screen:

```bash
engram tui

# In the TUI:
# - Press 'p' to switch to Packs screen
# - Navigate with j/k
# - Press 'r' to reload packs
# - Press ESC to return to browser
```

## Pack Categories

- **Patterns**: Reusable code patterns and best practices
- **Solutions**: Problem-solution pairs and debugging guides
- **Decisions**: Architectural decisions and trade-offs
- **Workflows**: Step-by-step workflows and processes
- **Preferences**: Tool preferences and configurations

## When to Use Hive

**Use Hive when:**
- You want to share knowledge with your team
- You've built up valuable patterns/solutions
- You want to learn from others' experiences
- You're working on similar projects across teams

**Core registry use case:**
- Learning how engram itself works
- Understanding knowledge management patterns
- Getting troubleshooting solutions
- Following best practices and workflows

## Storage Locations

- **Registries**: `~/memory/hive/registries/`
- **Installed packs**: `~/memory/packs/installed/`
- **Registry index**: `~/memory/hive/registries.json`
- **Installed packs index**: `~/memory/hive/installed_packs.json`

## Privacy & Security

- Raw conversations stay local (never synced)
- Packs contain only extracted knowledge
- Automatic secret detection before publishing
- Review required by default (privacy.require_review)
- Categories can be excluded (e.g., decisions, preferences)

## Troubleshooting

**Pack not found:**
```bash
# Update registries first
engram hive registry update

# Check if pack exists
engram hive browse | grep <pack-name>
```

**Knowledge not appearing:**
```bash
# Verify pack is installed
engram hive list

# Check pack contents
ls ~/memory/packs/installed/<pack-name>/knowledge/
```

**Registry clone failed:**
```bash
# Use full HTTPS URL if shorthand fails
engram hive registry add https://github.com/owner/repo.git

# For local registries, use file:// protocol
engram hive registry add file:///absolute/path/to/registry
```
