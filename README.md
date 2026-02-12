# claude-memory

Conversation memory system for Claude Code. Archives conversation sessions, extracts structured knowledge using LLMs, and enables full-text search and recall of project context.

## Install

**Quick install** (Linux, macOS):

```bash
curl -fsSL https://raw.githubusercontent.com/Algiras/claude-memory/master/install.sh | sh
```

**From source:**

```bash
git clone https://github.com/Algiras/claude-memory.git
cd claude-memory
cargo install --path .
```

**Prebuilt binaries**: download from [Releases](https://github.com/Algiras/claude-memory/releases) for Linux x86_64, macOS ARM, and Windows x86_64.

## Quick Start

```bash
# Archive all conversations (skip LLM extraction for speed)
claude-memory ingest --skip-knowledge

# Full pipeline with knowledge extraction (requires LLM)
claude-memory ingest

# Search across all memory
claude-memory search "authentication"

# Show project context
claude-memory recall my-project

# List projects
claude-memory projects

# Interactive TUI with fuzzy search
claude-memory tui

# View reinforcement learning progress
claude-memory learn dashboard
```

## MCP Server (Claude Desktop Integration)

`claude-memory` includes MCP (Model Context Protocol) server support for direct integration with Claude Desktop.

**Quick setup:**

1. Add to Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "claude-memory": {
      "command": "claude-memory",
      "args": ["mcp"]
    }
  }
}
```

2. Restart Claude Desktop

Claude can now directly access your memory during conversations! See [MCP_SETUP.md](MCP_SETUP.md) for detailed setup and usage.

## LLM Providers

Supports Anthropic, OpenAI, and Ollama for knowledge extraction. Defaults to Ollama (local) if nothing is configured.

**Precedence:** environment variables > `auth.json` > Ollama fallback

### Configure a provider

```bash
# Interactive login
claude-memory auth login

# Direct login
claude-memory auth login --provider anthropic

# Check status
claude-memory auth status

# List configured providers
claude-memory auth list

# Override per-command
claude-memory ingest --provider ollama

# Or use environment variables
ANTHROPIC_API_KEY=sk-ant-... claude-memory ingest
OPENAI_API_KEY=sk-... claude-memory ingest --provider openai
```

Credentials are stored in `~/.config/claude-memory/auth.json` with `0600` permissions.

## Commands

| Command | Description |
|---------|-------------|
| `ingest` | Parse JSONL conversations, archive as markdown, extract knowledge |
| `search <query>` | Full-text regex search across all memory |
| `recall <project>` | Display project knowledge context |
| `context <project>` | Output context.md to stdout (for piping) |
| `status` | Show memory statistics |
| `projects` | List all discovered projects |
| `auth login` | Configure LLM provider credentials |
| `auth list` | Show configured providers |
| `auth logout <provider>` | Remove provider credentials |
| `auth status` | Show active provider |
| `learn dashboard [project]` | View reinforcement learning progress and metrics |
| `learn optimize <project>` | Apply learned parameter optimizations |
| `learn reset <project>` | Reset learning state to defaults |

## How It Works

1. **Discovery** - Scans `~/.claude/projects/` for JSONL conversation files
2. **Parsing** - Extracts user/assistant turns, tool calls, and metadata
3. **Archival** - Renders conversations as markdown with analytics
4. **Knowledge Extraction** - Uses an LLM to extract decisions, solutions, patterns, and preferences
5. **Synthesis** - Generates a `context.md` per project from accumulated knowledge
6. **Reinforcement Learning** - Automatically optimizes knowledge importance, TTLs, and consolidation strategies based on usage patterns

> **New!** claude-memory now includes a reinforcement learning system that continuously improves itself. See [LEARNING_GUIDE.md](LEARNING_GUIDE.md) for details.

### Output Structure

```
~/memory/
├── conversations/{project}/{session}/   # Full markdown + metadata
├── summaries/{project}/                 # Brief session summaries
├── knowledge/{project}/                 # Decisions, solutions, patterns, context.md
├── knowledge/_global/                   # Cross-project preferences
└── analytics/                           # Usage and activity data
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
        "hooks": [{ "type": "command", "command": "/path/to/hooks/claude-memory-hook.sh" }]
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
| `CLAUDE_MEMORY_LLM_ENDPOINT` | per provider | Override LLM endpoint |
| `CLAUDE_MEMORY_LLM_MODEL` | per provider | Override LLM model |

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
claude-memory hive registry add anthropics/claude-memory

# Browse available packs
claude-memory hive browse

# Install a pack
claude-memory hive install claude-memory-core

# Use the knowledge (automatic!)
claude-memory recall <project>
# Now includes knowledge from installed packs
```

### Creating a Knowledge Pack

```bash
# Extract knowledge from your conversations first
claude-memory ingest --project my-project

# Create a pack
claude-memory hive pack create my-pack \
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
claude-memory hive pack validate ./packs/my-pack

# Publish to Git
claude-memory hive pack publish ./packs/my-pack \
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
claude-memory hive registry add owner/repo

# Or full URL
claude-memory hive registry add https://github.com/owner/repo.git

# For local development
claude-memory hive registry add file:///absolute/path/to/registry

# List registries
claude-memory hive registry list

# Update registries (git pull)
claude-memory hive registry update [name]

# Remove a registry
claude-memory hive registry remove <name>
```

### Pack Discovery

```bash
# Browse all packs
claude-memory hive browse

# Filter by category
claude-memory hive browse --category patterns

# Filter by keyword  
claude-memory hive browse --keyword rust

# Search packs
claude-memory hive search "async patterns"
```

### Pack Management

```bash
# Install a pack
claude-memory hive install <pack-name>

# From specific registry
claude-memory hive install <pack-name> --registry <registry-name>

# List installed packs
claude-memory hive list

# View pack statistics
claude-memory hive pack stats <pack-name>

# Update packs
claude-memory hive update                # All packs
claude-memory hive update <pack-name>    # Specific pack

# Uninstall
claude-memory hive uninstall <pack-name>
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
  "min_claude_memory_version": "0.1.0"
}
```

### Integration with Existing Commands

Once packs are installed, their knowledge automatically appears:

```bash
# Recall includes local + pack knowledge
claude-memory recall <project>

# Search across local and packs
claude-memory search "pattern"

# Lookup searches both sources
claude-memory lookup <project> "topic"
```

### TUI: Interactive Pack Browser

```bash
claude-memory tui

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
claude-memory hive pack publish ./pack --skip-security
```

### Health Checks

The `doctor` command now checks pack health:

```bash
claude-memory doctor

# Checks:
# - Manifest validity
# - Knowledge directory exists
# - Knowledge files present
# - Registry still exists (detects orphans)

# Auto-fix:
claude-memory doctor --fix
# - Re-downloads corrupted packs
# - Removes orphaned packs
```

### Core Registry

This repository includes a meta-knowledge pack:

```bash
# Add the core registry (from local clone)
cd /path/to/claude-memory
claude-memory hive registry add file://$(pwd)/registry

# Install the core pack
claude-memory hive install claude-memory-core

# This pack contains:
# - 15 patterns about knowledge management
# - 10 solutions for common problems
# - 7 complete workflows
# - Documentation about claude-memory itself!
```

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
claude-memory ingest --project my-rust-project

# 2. Create a pack
claude-memory hive pack create rust-patterns \
  --project my-rust-project \
  --description "Rust async patterns and best practices" \
  --keywords "rust,async,patterns,tokio" \
  --categories "patterns,solutions"

# 3. Validate
claude-memory hive pack validate ./packs/rust-patterns

# 4. Publish
claude-memory hive pack publish ./packs/rust-patterns \
  --repo https://github.com/user/rust-patterns \
  --push

# 5. Share with team
# Team members can now:
# claude-memory hive registry add user/rust-patterns
# claude-memory hive install rust-patterns
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
claude-memory hive registry update

# Verify pack exists
claude-memory hive browse | grep <pack-name>
```

**Knowledge not appearing:**
```bash
# Verify pack is installed
claude-memory hive list

# Check pack contents
ls ~/memory/packs/installed/<pack-name>/knowledge/

# Check pack health
claude-memory doctor
```

**Registry clone failed:**
```bash
# Use full HTTPS URL
claude-memory hive registry add https://github.com/owner/repo.git

# Test git access
git ls-remote <url>
```

