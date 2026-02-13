# Hive Mind - Complete Guide

## Overview

The Hive Mind is a Git-based distributed knowledge sharing system for engram. It enables teams to create, publish, discover, and install knowledge packs.

## Quick Start (3 Commands)

```bash
# 1. Add a registry
engram hive registry add Algiras/engram-registry

# 2. Install the meta-knowledge pack
engram hive install engram-core

# 3. Use it
engram recall <your-project>
# Knowledge from pack now appears automatically!
```

## Complete Workflow

### For Consumers (Install & Use Knowledge)

```bash
# Add a registry (one-time setup)
engram hive registry add owner/repo

# Browse available packs
engram hive browse
engram hive browse --category patterns
engram hive browse --keyword rust

# Search for specific knowledge
engram hive search "async patterns"

# Install a pack
engram hive install <pack-name>

# Use the knowledge (automatic integration!)
engram recall <project>   # Includes pack knowledge
engram search "query"      # Searches packs too

# Keep packs updated
engram hive update

# Remove if no longer needed
engram hive uninstall <pack-name>
```

### For Creators (Share Your Knowledge)

```bash
# 1. Extract knowledge from your work
engram ingest --project my-project

# 2. Create a pack
engram hive pack create my-pack \
  --project my-project \
  --description "My awesome patterns" \
  --keywords "rust,async,patterns" \
  --categories "patterns,solutions"

# → Security scans automatically
# → Creates manifest, copies knowledge, generates README
# → Output: ./packs/my-pack/

# 3. Review the pack
cd ./packs/my-pack
cat .pack/manifest.json
ls knowledge/

# 4. Validate
engram hive pack validate .

# 5. Publish
engram hive pack publish . \
  --repo https://github.com/user/my-pack \
  --push

# → Re-scans for secrets
# → Initializes git, commits, tags version
# → Pushes to GitHub

# 6. Share with others
# Users can now: engram hive registry add user/my-pack
```

## Commands Reference

### Registry Management

| Command | Description |
|---------|-------------|
| `hive registry add <url>` | Add a knowledge pack registry |
| `hive registry list` | List all configured registries |
| `hive registry update [name]` | Update registry (git pull) |
| `hive registry remove <name>` | Remove a registry |

**URL Formats:**
- GitHub shorthand: `owner/repo`
- Full HTTPS: `https://github.com/owner/repo.git`
- Local: `file:///absolute/path/to/registry`

### Pack Management

| Command | Description |
|---------|-------------|
| `hive pack create <name> --project <proj>` | Create pack from local knowledge |
| `hive pack validate <path>` | Validate pack structure |
| `hive pack publish <path> [--repo <url>] [--push]` | Publish pack to Git |
| `hive pack stats <name>` | Show pack statistics |
| `hive install <pack> [--registry <name>]` | Install a pack |
| `hive uninstall <pack>` | Uninstall a pack |
| `hive list` | List installed packs |
| `hive update [pack]` | Update pack(s) |
| `hive browse [--category <cat>] [--keyword <kw>]` | Browse available packs |
| `hive search <query>` | Search for packs |

### Pack Create Options

| Flag | Description |
|------|-------------|
| `--project <name>` | **Required.** Source project to extract knowledge from |
| `--description <text>` | Pack description |
| `--author <name>` | Author name |
| `--keywords <list>` | Comma-separated keywords |
| `--categories <list>` | Comma-separated: patterns,solutions,decisions,workflows,preferences |
| `--output <dir>` | Output directory (default: `./packs/<name>`) |

## Pack Structure

```
my-pack/
  .pack/
    manifest.json          # Required: Pack metadata
  knowledge/               # Required: At least one .md file
    patterns.md
    solutions.md
    workflows.md
    decisions.md          # Optional
    preferences.md        # Optional
  graph/                  # Optional
    knowledge_graph.json
  README.md              # Recommended
```

### Manifest Schema

```json
{
  "name": "pack-name",
  "version": "1.0.0",
  "description": "Pack description",
  "author": {
    "name": "Your Name",
    "email": "optional@email.com"
  },
  "license": "MIT",
  "keywords": ["keyword1", "keyword2"],
  "categories": ["Patterns", "Solutions"],
  "repository": "https://github.com/user/pack",
  "homepage": "https://optional-homepage.com",
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

### Knowledge File Format

Use session block format:

```markdown
# Patterns

## Session: unique-id (2026-02-12T00:00:00Z) [ttl:30d]

### Pattern Name

**Pattern:** Brief description

Detailed explanation...

**Example:**
\`\`\`rust
// Code example
\`\`\`

**Use Cases:**
- When to use
- Benefits
```

## Categories

- **Patterns**: Reusable code patterns, best practices
- **Solutions**: Problem-solution pairs, debugging guides
- **Decisions**: Architectural decisions, design trade-offs
- **Workflows**: Step-by-step processes, complete guides
- **Preferences**: Tool preferences, coding styles

## Security

### Automatic Secret Detection

Scans for 12 types of secrets:
- API keys (OpenAI, Anthropic, generic)
- Tokens (GitHub, Bearer, Auth)
- Passwords
- Private keys
- AWS credentials
- JWT tokens

**Behavior:**
- Blocks pack creation and publishing if secrets found
- Shows file:line location
- Requires manual removal
- Publishing can skip with `--skip-security` (NOT recommended)

### Privacy Controls

In manifest `privacy` section:
- `share_patterns`: Default true
- `share_solutions`: Default true
- `share_decisions`: Default false (may be project-specific)
- `share_preferences`: Default false (personal)
- `redact_secrets`: Default true (always scan)
- `require_review`: Default true (manual approval)

## Health Monitoring

```bash
# Check all projects + packs
engram doctor

# Auto-fix issues
engram doctor --fix

# Verbose output
engram doctor --verbose
```

**Pack Health Checks:**
- Manifest exists and valid
- Knowledge directory present
- At least one knowledge file
- Registry still exists (orphan detection)

**Auto-Fix Capabilities:**
- Re-download corrupted packs
- Remove orphaned packs

## TUI (Interactive Browser)

```bash
engram tui

# In Packs screen (press 'p'):
# j/k       - Navigate
# Enter     - View details
# /         - Search
# n/N       - Next/previous match
# u         - Update pack
# d         - Uninstall pack
# r         - Reload
# ESC       - Back to browser
# q         - Quit
```

## Integration

### Automatic Knowledge Aggregation

Once packs are installed, their knowledge appears in:

```bash
# Recall shows local + pack knowledge
engram recall <project>

# Search across all sources
engram search "pattern"

# Lookup in combined knowledge
engram lookup <project> "topic"
```

### Claude Code Integration

**Skill** (auto-loaded on triggers):
- "install knowledge pack"
- "browse packs"
- "search for patterns"

**Plugin** (slash commands):
- `/hive-browse`
- `/hive-install <pack>`
- `/hive-list`
- `/hive-search <query>`

## Example Packs

### engram-core (Meta-Knowledge)

Included in this repository:

```bash
# Add core registry
cd /path/to/engram
engram hive registry add file://$(pwd)/examples/registry

# Install
engram hive install engram-core

# Contains:
# - How knowledge extraction works
# - How to use the hive system
# - Common problems & solutions
# - Complete workflows
```

### Creating Your Own

```bash
# 1. Work on your project, extract knowledge
engram ingest --project my-rust-proj

# 2. Create pack
engram hive pack create rust-patterns \
  --project my-rust-proj \
  --description "Rust async patterns" \
  --keywords "rust,async,tokio" \
  --categories "patterns,solutions"

# 3. Validate & publish
engram hive pack validate ./packs/rust-patterns
engram hive pack publish ./packs/rust-patterns \
  --repo https://github.com/user/rust-patterns \
  --push
```

## Best Practices

1. **Naming**: Use descriptive names (`typescript-react-patterns` not `patterns`)
2. **Keywords**: Add 5-10 searchable keywords
3. **Categories**: Choose appropriate categories
4. **Versions**: Follow semver (1.0.0, 1.1.0, 2.0.0)
5. **Documentation**: Include comprehensive README
6. **Focus**: One topic per pack
7. **Review**: Always check for sensitive data before publishing
8. **Updates**: Keep packs current

## Troubleshooting

### Pack Not Found

```bash
# Update registries first
engram hive registry update

# Check if pack exists
engram hive browse | grep <pack-name>
```

### Knowledge Not Appearing

```bash
# Verify installation
engram hive list

# Check files
ls ~/memory/packs/installed/<pack>/knowledge/

# Check health
engram doctor
```

### Registry Clone Failed

```bash
# Use full HTTPS URL
engram hive registry add https://github.com/owner/repo.git

# Test git access
git ls-remote <url>

# For private repos, ensure SSH keys are set up
```

## Storage Locations

- **Registries**: `~/memory/hive/registries/` (shallow Git clones)
- **Installed packs**: `~/memory/packs/installed/`
- **Registry index**: `~/memory/hive/registries.json`
- **Pack index**: `~/memory/hive/installed_packs.json`
- **Local knowledge**: `~/memory/knowledge/` (never synced)

## Privacy Model

**What Stays Local (Never Shared):**
- Raw conversation JSONL files
- Analytics and usage events
- Learning state and metrics
- Personal preferences
- Absolute file paths
- Session summaries

**What Can Be Shared (In Packs):**
- Extracted patterns and solutions
- Workflows and best practices
- Knowledge graph relationships
- Public decisions (opt-in only)
- Redacted, reviewed knowledge

**How It's Protected:**
- Automatic secret detection
- Privacy controls per category
- Review requirement before publishing
- No raw conversation data in packs
- User controls what gets shared

## Advanced Features

### Pack Statistics

```bash
engram hive pack stats <pack-name>

# Shows:
# - Entry counts per category
# - Total size
# - Installation date
# - Registry source
```

### Health Monitoring

```bash
# Check everything
engram doctor

# Checks:
# - Manifest validity
# - Knowledge files exist
# - Registry still exists
# - Pack integrity

# Auto-fix issues
engram doctor --fix
```

### Validation

```bash
engram hive pack validate <path>

# Checks:
# - Manifest exists and valid JSON
# - Knowledge directory exists
# - At least one knowledge file present
# - Proper structure
```

## Performance

- **Registry cloning**: Shallow clone (--depth 1) for speed
- **Pack installation**: File copy, no LLM processing
- **Search**: Fast fuzzy matching
- **Updates**: Incremental git pull
- **Health checks**: < 1 second per pack

## Limitations & Future

**Current Limitations:**
- No automatic conflict resolution (manual review required)
- No dependency management between packs
- No versioning constraints
- No pack signing/verification

**Potential Future:**
- Dependency resolution
- Semantic versioning constraints
- Digital signatures
- Automatic conflict resolution
- Pack mirrors/CDN

## Contributing

Contribute packs to the community:

1. Create high-quality knowledge packs
2. Publish to GitHub
3. Share registry URL
4. Submit to engram community registry (planned)

## License

MIT - Same as engram

## Learn More

- Main Repository: https://github.com/Algiras/engram
- Example Registry: https://github.com/Algiras/engram/tree/master/examples/registry
- Issues: https://github.com/Algiras/engram/issues
