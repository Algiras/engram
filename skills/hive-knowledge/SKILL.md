---
name: hive-knowledge
description: Distributed knowledge sharing for engram via Git-based registries. Discover, install, and share knowledge packs containing patterns, solutions, and workflows. Use when managing knowledge bases, sharing team knowledge, or working with engram's Hive Mind system.
license: MIT
metadata:
  author: engram
  version: "1.0.0"
  repository: https://github.com/Algiras/engram
  triggers:
    - install knowledge pack
    - browse packs
    - hive mind
    - share knowledge
    - knowledge registry
    - distributed memory
---

# Hive Knowledge - Distributed Memory Sharing

Manage and share knowledge packs via engram's Hive Mind system - a Git-based distributed knowledge sharing platform.

## When to Use This Skill

Use this skill when:
- Managing knowledge bases with engram
- Sharing patterns, solutions, and workflows across teams
- Installing community knowledge packs
- Creating distributable knowledge collections
- Working with distributed memory systems
- Setting up knowledge registries

Trigger phrases:
- "install knowledge pack"
- "browse available packs"
- "search for patterns"
- "share knowledge with team"
- "setup knowledge registry"

## Prerequisites

**Required**: engram CLI tool

```bash
# Install via curl (auto-detects OS/arch)
curl -fsSL https://raw.githubusercontent.com/Algiras/engram/master/install.sh | sh

# Or pin a version
curl -fsSL https://raw.githubusercontent.com/Algiras/engram/master/install.sh | VERSION=v0.3.0 sh

# Fallback: install from source
# git clone https://github.com/Algiras/engram
# cd engram
# cargo install --path .

# Verify installation
engram --version
```

## Core Concepts

### Registry
A Git repository containing one or more knowledge packs. Similar to npm/cargo registries.

```bash
# Add a registry (GitHub shorthand)
engram hive registry add owner/repo

# Or full URL
engram hive registry add https://github.com/owner/repo.git

# For local registries
engram hive registry add file:///absolute/path/to/registry
```

### Knowledge Pack
A structured directory containing extracted knowledge:

```
pack-name/
  .pack/
    manifest.json          # Pack metadata
  knowledge/
    patterns.md           # Reusable patterns
    solutions.md          # Problem-solution pairs
    workflows.md          # Step-by-step processes
    decisions.md          # Architectural decisions
  graph/                  # Optional knowledge graph
  README.md              # Documentation
```

### Privacy Model
- Raw conversations NEVER leave your machine
- Only extracted, redacted knowledge is shareable
- Privacy controls in pack manifest
- Automatic secret detection

## Commands

### Registry Management

```bash
# Add a registry
engram hive registry add owner/repo

# List all registries
engram hive registry list

# Update registry (git pull)
engram hive registry update [name]

# Remove a registry
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

# Search across all packs
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

# Update all packs
engram hive update

# Update specific pack
engram hive update <pack-name>

# Uninstall a pack
engram hive uninstall <pack-name>
```

### Integration with Existing Commands

Once packs are installed, their knowledge automatically appears in:

```bash
# Recall includes local + pack knowledge
engram recall <project>

# Search across local and packs
engram search "pattern"

# Lookup in combined knowledge
engram lookup <project> "topic"

# TUI: Press 'p' to browse packs
engram tui
```

## Creating a Knowledge Pack

### Step 1: Create Structure

```bash
mkdir -p my-pack/{.pack,knowledge}
```

### Step 2: Create Manifest

Create `my-pack/.pack/manifest.json`:

```json
{
  "name": "my-pack",
  "version": "1.0.0",
  "description": "Description of your pack",
  "author": {
    "name": "Your Name",
    "email": "your.email@example.com"
  },
  "license": "MIT",
  "keywords": ["rust", "patterns", "async"],
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
  "min_claude_memory_version": "0.1.0"
}
```

### Step 3: Add Knowledge Files

Use session block format in knowledge files:

```markdown
# Patterns

## Session: my-pattern-id (2026-02-12T00:00:00Z) [ttl:30d]

### Pattern Name

**Pattern:** Brief description

Detailed explanation of the pattern...

**Example:**
\`\`\`rust
// Code example
\`\`\`

**Use Cases:**
- When to use this pattern
- Benefits and trade-offs
```

### Step 4: Publish to Git

```bash
cd my-pack
git init
git add .
git commit -m "Initial pack"
git remote add origin https://github.com/user/my-pack
git push -u origin main
```

### Step 5: Share

Users can now install:
```bash
engram hive registry add user/my-pack
engram hive install my-pack
```

## Pack Categories

Available categories for organizing knowledge:

- **Patterns**: Reusable code patterns and best practices
- **Solutions**: Problem-solution pairs and debugging guides
- **Decisions**: Architectural decisions and design trade-offs
- **Workflows**: Step-by-step processes and workflows
- **Preferences**: Tool preferences and coding style

## Example: Core Pack

The engram repository includes a meta-knowledge pack:

```bash
# Clone the repository
git clone https://github.com/Algiras/engram
cd engram

# Add the core registry
engram hive registry add file://$(pwd)/registry

# Install the core pack
engram hive install engram-core

# This pack contains:
# - 15 patterns about knowledge extraction
# - 10 solutions for common problems
# - 7 complete workflows
# - Meta-knowledge: system documents itself!
```

## TUI Integration

The TUI includes a Packs browser:

```bash
# Launch TUI
engram tui

# Keyboard shortcuts:
# - 'p' : Switch to Packs screen
# - 'j'/'k' : Navigate up/down
# - 'r' : Reload packs
# - ESC : Return to browser
# - 'q' : Quit
```

## Common Workflows

### Installing Community Knowledge

```bash
# 1. Add a registry
engram hive registry add community/knowledge-packs

# 2. Browse what's available
engram hive browse

# 3. Search for specific topics
engram hive search "rust patterns"

# 4. Install what you need
engram hive install rust-best-practices

# 5. Use immediately
engram recall my-rust-project
# Knowledge from pack is now included!
```

### Sharing Your Knowledge

```bash
# 1. Extract knowledge from conversations
engram ingest --project my-project

# 2. Review extracted knowledge
engram recall my-project

# 3. Create pack structure
mkdir -p my-knowledge/{.pack,knowledge}

# 4. Copy knowledge files
cp ~/memory/knowledge/my-project/*.md my-knowledge/knowledge/

# 5. Create manifest (see Step 2 above)

# 6. Review for sensitive data
vim my-knowledge/knowledge/*.md

# 7. Publish to Git
cd my-knowledge
git init && git add . && git commit -m "Initial" && git push
```

## Storage Locations

- **Registries**: `~/memory/hive/registries/`
- **Installed packs**: `~/memory/packs/installed/`
- **Registry index**: `~/memory/hive/registries.json`
- **Pack index**: `~/memory/hive/installed_packs.json`
- **Local knowledge**: `~/memory/knowledge/` (private)

## Troubleshooting

### Pack Not Found

```bash
# Update registries to get latest packs
engram hive registry update

# Verify pack exists
engram hive browse | grep <pack-name>
```

### Knowledge Not Appearing

```bash
# Verify pack is installed
engram hive list

# Check pack directory
ls ~/memory/packs/installed/<pack-name>/knowledge/

# Verify knowledge files exist
cat ~/memory/packs/installed/<pack-name>/knowledge/patterns.md
```

### Registry Clone Failed

```bash
# Use full HTTPS URL
engram hive registry add https://github.com/owner/repo.git

# For local development
engram hive registry add file:///absolute/path

# Check Git access
git ls-remote <registry-url>
```

## Integration Points

The Hive Mind integrates with:
- **Recall**: Aggregates local + pack knowledge
- **Search**: Searches local + pack content
- **Lookup**: Searches local + pack files
- **TUI**: Browse packs with 'p' key
- **Learning**: Can learn from pack usage patterns
- **Analytics**: Tracks pack usage events

## Security & Privacy

**What Stays Private:**
- Raw conversations (JSONL archives)
- Analytics and usage events
- Learning state and metrics
- Personal preferences
- Absolute file paths

**What Can Be Shared:**
- Extracted patterns and solutions
- Workflows and best practices
- Knowledge graph relationships
- Public decisions (opt-in)

**Automatic Protection:**
- Secret detection (API keys, tokens, passwords)
- Review requirement before publishing
- Configurable privacy policies per category
- No raw conversation data in packs

## Performance

- **Registry cloning**: Shallow clone (--depth 1) for speed
- **Pack installation**: File copy, no processing
- **Knowledge loading**: Lazy loading on recall
- **Search**: Indexed search across all sources
- **Updates**: Incremental git pull

## Best Practices

1. **Use descriptive pack names**: `rust-async-patterns` not just `patterns`
2. **Add comprehensive keywords**: Helps discovery
3. **Use proper categories**: Organize knowledge logically
4. **Set appropriate TTLs**: Use "never" for stable knowledge, "30d" for volatile
5. **Review before sharing**: Check for sensitive data
6. **Keep packs focused**: One topic per pack
7. **Update regularly**: Pull pack updates monthly
8. **Version semantically**: Follow semver for pack versions

## Related Commands

```bash
# Knowledge extraction
engram ingest [--project <name>]

# View knowledge
engram recall <project>
engram search <query>
engram lookup <project> <topic>

# Manage knowledge
engram add <project> <category> <content>
engram forget <project> [--expired]

# Analytics
engram analytics [project]
engram learn dashboard

# Health checks
engram doctor [project]
```

## Examples

See the engram-core pack in the repository for a complete example of:
- Proper manifest structure
- Well-formatted knowledge files
- Comprehensive documentation
- Real-world patterns and solutions

## Learn More

- Repository: https://github.com/Algiras/engram
- Core Registry: https://github.com/Algiras/engram/tree/master/registry
- Issues: https://github.com/Algiras/engram/issues
