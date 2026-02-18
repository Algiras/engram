# engram

**[Website](https://algiras.github.io/engram/)** | **[Docs](https://algiras.github.io/engram/docs.html)** | **[llms.txt](https://algiras.github.io/engram/llms.txt)**

Conversation memory system for Claude Code. Archives conversation sessions, extracts structured knowledge using LLMs, and enables full-text search and recall of project context.

## Install

**Quick install** (Linux, macOS):

```bash
curl -fsSL https://raw.githubusercontent.com/Algiras/engram/master/install.sh | sh
```

The installer auto-detects your OS/architecture and downloads the matching release asset.

**Manual checksum verification (optional):**

```bash
VERSION=v0.3.0
ASSET=engram-aarch64-apple-darwin.tar.gz   # choose your platform asset

curl -fsSLO https://github.com/Algiras/engram/releases/download/${VERSION}/${ASSET}
curl -fsSLO https://github.com/Algiras/engram/releases/download/${VERSION}/checksums.txt

# macOS
shasum -a 256 ${ASSET}

# Linux
sha256sum ${ASSET}

# Compare output hash with the matching line in checksums.txt
grep " ${ASSET}$" checksums.txt
```

**From source:**

```bash
git clone https://github.com/Algiras/engram.git
cd engram
cargo install --path .
```

**Prebuilt binaries**: download from [Releases](https://github.com/Algiras/engram/releases) for Linux x86_64, macOS ARM, and Windows x86_64.

## Quick Start

```bash
# Archive all conversations (skip LLM extraction for speed)
engram ingest --skip-knowledge

# Full pipeline with knowledge extraction (requires LLM)
engram ingest

# Search across all memory
engram search "authentication"

# Show project context
engram recall my-project

# List projects
engram projects

# Interactive TUI with fuzzy search
engram tui

# View reinforcement learning progress
engram learn dashboard
```

## MCP Server (Claude Desktop Integration)

`engram` includes MCP (Model Context Protocol) server support for direct integration with Claude Desktop.

**Quick setup:**

1. Add to Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "engram": {
      "command": "engram",
      "args": ["mcp"]
    }
  }
}
```

2. Restart Claude Desktop

Claude can now directly access your memory during conversations! See [MCP_SETUP.md](docs/MCP_SETUP.md) for detailed setup and usage.

## LLM Providers

Supports Anthropic, OpenAI, and Ollama for knowledge extraction. Defaults to Ollama (local) if nothing is configured.

**Precedence:** environment variables > `auth.json` > Ollama fallback

### Configure a provider

```bash
# Interactive login
engram auth login

# Direct login
engram auth login --provider anthropic

# Check status
engram auth status

# List configured providers
engram auth list

# Override per-command
engram ingest --provider ollama

# Or use environment variables
ANTHROPIC_API_KEY=sk-ant-... engram ingest
OPENAI_API_KEY=sk-... engram ingest --provider openai
```

Credentials are stored in `~/.config/engram/auth.json` with `0600` permissions.

## Commands

| Command | Description |
|---------|-------------|
| `ingest` | Parse JSONL conversations, archive as markdown, extract knowledge |
| `search <query>` | Full-text regex search across all memory |
| `recall <project>` | Display project knowledge context (includes installed packs) |
| `lookup <project> <query>` | Search knowledge entries by content |
| `context <project>` | Output context.md to stdout (for piping) |
| `inject [project]` | Write combined knowledge to Claude Code MEMORY.md |
| `add <project> <category> <content>` | Manually add a knowledge entry |
| `forget <project> <session-id>` | Remove a specific knowledge entry |
| `status` | Show memory statistics |
| `projects` | List all discovered projects |
| `doctor [--fix]` | Health check for knowledge files and packs |
| `export <project> [markdown\|json\|html]` | Export project knowledge to various formats |
| `tui` | Interactive terminal UI (browse, search, packs, analytics, health, learning) |
| `auth login` | Configure LLM provider credentials |
| `auth list` | Show configured providers |
| `auth logout <provider>` | Remove provider credentials |
| `auth status` | Show active provider |
| `learn dashboard [project]` | View reinforcement learning progress and metrics |
| `learn optimize <project>` | Apply learned parameter optimizations |
| `learn simulate <project>` | Run learning simulation |
| `learn feedback <project>` | Provide explicit feedback signal |
| `learn reset <project>` | Reset learning state to defaults |
| `hive browse` | Browse available knowledge packs |
| `hive search <query>` | Search for packs across registries |
| `hive install <pack>` | Install a knowledge pack |
| `hive list` | List installed packs |
| `hive registry add <url>` | Add a pack registry |
| `daemon start [--interval N]` | Start background ingest daemon (polls every N minutes, default 15) |
| `daemon stop` | Stop the running daemon |
| `daemon status` | Show daemon status and PID |
| `daemon logs [-f]` | View daemon log output (use `-f` to follow) |

See [HIVE_GUIDE.md](docs/HIVE_GUIDE.md) for full hive commands. See [LEARNING_GUIDE.md](docs/LEARNING_GUIDE.md) for the learning system. See [DAEMON_GUIDE.md](docs/DAEMON_GUIDE.md) for background ingest.

## How It Works

1. **Discovery** - Scans `~/.claude/projects/` for JSONL conversation files
2. **Parsing** - Extracts user/assistant turns, tool calls, and metadata
3. **Archival** - Renders conversations as markdown with analytics
4. **Knowledge Extraction** - Uses an LLM to extract decisions, solutions, patterns, and preferences
5. **Synthesis** - Generates a `context.md` per project from accumulated knowledge
6. **Reinforcement Learning** - Automatically optimizes knowledge importance, TTLs, and consolidation strategies based on usage patterns

**Detailed Guides:**
- [HIVE_GUIDE.md](docs/HIVE_GUIDE.md) - Distributed knowledge sharing
- [LEARNING_GUIDE.md](docs/LEARNING_GUIDE.md) - Reinforcement learning system
- [ANALYTICS_GUIDE.md](docs/ANALYTICS_GUIDE.md) - Usage analytics
- [GRAPH_GUIDE.md](docs/GRAPH_GUIDE.md) - Knowledge graph
- [EMBEDDINGS_GUIDE.md](docs/EMBEDDINGS_GUIDE.md) - Semantic search
- [SYNC_GUIDE.md](docs/SYNC_GUIDE.md) - Knowledge synchronization
- [EXPORT_GUIDE.md](docs/EXPORT_GUIDE.md) - Export formats
- [PUBLISHING.md](docs/PUBLISHING.md) - Release and publishing checklist

### Output Structure

```
~/memory/
├── conversations/{project}/{session}/   # Full markdown + metadata
├── summaries/{project}/                 # Brief session summaries
├── knowledge/{project}/                 # Decisions, solutions, patterns, context.md
├── knowledge/_global/                   # Cross-project preferences
├── analytics/                           # Usage and activity data
├── packs/installed/                     # Installed hive knowledge packs
├── hive/registries/                     # Registry clones
├── daemon.pid                           # Daemon PID (present when running)
└── daemon.log                           # Daemon output log
```

## Claude Code Hook

Auto-archive conversations in the background using a PostToolUse hook:

```json
// ~/.claude/settings.json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "",
        "hooks": [{ "type": "command", "command": "/path/to/hooks/engram-hook.sh" }]
      }
    ]
  }
}
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ANTHROPIC_API_KEY` | - | Anthropic API key (auto-selects Anthropic provider) |
| `OPENAI_API_KEY` | - | OpenAI API key (auto-selects OpenAI provider) |
| `ENGRAM_LLM_ENDPOINT` | per provider | Override LLM endpoint |
| `ENGRAM_LLM_MODEL` | per provider | Override LLM model |

## License

MIT

## Hive Mind: Distributed Knowledge Sharing

Share knowledge packs across teams via Git-based registries.

### Overview

The Hive Mind system enables you to:
- **Create** knowledge packs from your extracted knowledge
- **Publish** packs to Git repositories
- **Discover** community packs from registries
- **Install** packs to access shared knowledge
- **Contribute** to collective knowledge bases

**Privacy:** Only extracted knowledge is shared. Raw conversations NEVER leave your machine.

### Quick Start

```bash
# Add a registry
engram hive registry add Algiras/engram-registry

# Browse available packs
engram hive browse

# Install a pack
engram hive install engram-core

# Use the knowledge (automatic!)
engram recall <project>
# Now includes knowledge from installed packs
```

### Creating a Knowledge Pack

```bash
# Extract knowledge from your conversations first
engram ingest --project my-project

# Create a pack
engram hive pack create my-pack \
  --project my-project \
  --description "My awesome patterns" \
  --keywords "rust,async,patterns" \
  --categories "patterns,solutions"

# Security scan runs automatically!
# Validates pack structure
# Creates manifest, copies knowledge, generates README

# Output: ./packs/my-pack/
```

### Publishing a Pack

```bash
# Validate before publishing
engram hive pack validate ./packs/my-pack

# Publish to Git
engram hive pack publish ./packs/my-pack \
  --repo https://github.com/user/my-pack \
  --push

# This will:
# - Re-scan for secrets
# - Initialize git if needed
# - Commit changes
# - Set up remote
# - Push to GitHub
# - Create version tag
```

### Registry Management

```bash
# Add a registry (GitHub shorthand)
engram hive registry add owner/repo

# Or full URL
engram hive registry add https://github.com/owner/repo.git

# For local development
engram hive registry add file:///absolute/path/to/registry

# List registries
engram hive registry list

# Update registries (git pull)
engram hive registry update [name]

# Remove a registry
engram hive registry remove <name>
```

### Pack Discovery

```bash
# Browse all packs
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

# From specific registry
engram hive install <pack-name> --registry <registry-name>

# List installed packs
engram hive list

# View pack statistics
engram hive pack stats <pack-name>

# Update packs
engram hive update                # All packs
engram hive update <pack-name>    # Specific pack

# Uninstall
engram hive uninstall <pack-name>
```

### Pack Structure

A knowledge pack is a directory with this structure:

```
my-pack/
  .pack/
    manifest.json          # Pack metadata
  knowledge/
    patterns.md           # Reusable patterns
    solutions.md          # Problem-solution pairs
    workflows.md          # Step-by-step processes
    decisions.md          # Architectural decisions (optional)
    preferences.md        # Tool preferences (optional)
  graph/                  # Knowledge graph (optional)
    knowledge_graph.json
  README.md              # Documentation
```

**Manifest Example:**

```json
{
  "name": "my-pack",
  "version": "1.0.0",
  "description": "My knowledge pack",
  "author": {"name": "Your Name", "email": "you@example.com"},
  "license": "MIT",
  "keywords": ["rust", "patterns", "async"],
  "categories": ["Patterns", "Solutions"],
  "repository": "https://github.com/user/my-pack",
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

### Integration with Existing Commands

Once packs are installed, their knowledge automatically appears:

```bash
# Recall includes local + pack knowledge
engram recall <project>

# Search across local and packs
engram search "pattern"

# Lookup searches both sources
engram lookup <project> "topic"
```

### TUI: Interactive Pack Browser

```bash
engram tui

# Keyboard shortcuts:
# 'p' - Switch to Packs screen
# 'j'/'k' - Navigate
# Enter - View pack details
# 'u' - Update pack
# 'd' - Uninstall pack
# 'r' - Reload
# ESC - Back to browser
```

### Security

**Automatic Secret Detection:**

The system scans for 12 types of secrets before pack creation/publishing:
- API keys (OpenAI, Anthropic, generic)
- Tokens (GitHub, Bearer, Auth)
- Passwords
- Private keys (RSA, SSH, EC)
- AWS credentials
- JWT tokens

**If secrets are detected:**
- Pack creation/publishing is BLOCKED
- Shows exact location (file:line)
- Lists secret type
- Requires manual removal

**Skip security check** (NOT RECOMMENDED):
```bash
engram hive pack publish ./pack --skip-security
```

### Health Checks

The `doctor` command now checks pack health:

```bash
engram doctor

# Checks:
# - Manifest validity
# - Knowledge directory exists
# - Knowledge files present
# - Registry still exists (detects orphans)

# Auto-fix:
engram doctor --fix
# - Re-downloads corrupted packs
# - Removes orphaned packs
```

### Core Registry

This repository includes a meta-knowledge pack and an example registry structure in `examples/registry/`:

```bash
# Add the core registry (from local clone)
cd /path/to/engram
engram hive registry add file://$(pwd)/examples/registry

# Install the core pack
engram hive install engram-core

# This pack contains:
# - 15 patterns about knowledge management
# - 10 solutions for common problems
# - 7 complete workflows
# - Documentation about engram itself!
```

See `examples/registry/` for a reference implementation of a custom registry.

### Privacy Model

**What Stays Private:**
- Raw conversations (JSONL archives)
- Analytics and usage data
- Learning state
- Personal preferences
- Absolute file paths

**What Can Be Shared (in packs):**
- Extracted patterns and solutions
- Workflows and best practices
- Knowledge graph relationships
- Public decisions (opt-in)
- Redacted, reviewed knowledge only

**Controls:**
- `privacy` settings in manifest
- Automatic secret detection
- Review requirement before publishing
- Per-category sharing controls

### Best Practices

1. **Pack Naming:** Use descriptive names (`rust-async-patterns` not just `patterns`)
2. **Keywords:** Add comprehensive keywords for discovery
3. **Categories:** Use appropriate PackCategory values
4. **Versions:** Follow semantic versioning
5. **Security:** Always review before publishing
6. **Updates:** Keep packs current with `hive update`
7. **Documentation:** Include comprehensive README
8. **Focus:** One topic per pack (don't mix unrelated knowledge)

### Example: Complete Workflow

```bash
# 1. Extract knowledge from your work
engram ingest --project my-rust-project

# 2. Create a pack
engram hive pack create rust-patterns \
  --project my-rust-project \
  --description "Rust async patterns and best practices" \
  --keywords "rust,async,patterns,tokio" \
  --categories "patterns,solutions"

# 3. Validate
engram hive pack validate ./packs/rust-patterns

# 4. Publish
engram hive pack publish ./packs/rust-patterns \
  --repo https://github.com/user/rust-patterns \
  --push

# 5. Share with team
# Team members can now:
# engram hive registry add user/rust-patterns
# engram hive install rust-patterns
```

### Storage Locations

- **Registries:** `~/memory/hive/registries/` (Git clones)
- **Installed packs:** `~/memory/packs/installed/`
- **Registry index:** `~/memory/hive/registries.json`
- **Pack index:** `~/memory/hive/installed_packs.json`
- **Local knowledge:** `~/memory/knowledge/` (never synced)

### Troubleshooting

**Pack not found:**
```bash
# Update registries
engram hive registry update

# Verify pack exists
engram hive browse | grep <pack-name>
```

**Knowledge not appearing:**
```bash
# Verify pack is installed
engram hive list

# Check pack contents
ls ~/memory/packs/installed/<pack-name>/knowledge/

# Check pack health
engram doctor
```

**Registry clone failed:**
```bash
# Use full HTTPS URL
engram hive registry add https://github.com/owner/repo.git

# Test git access
git ls-remote <url>
```

