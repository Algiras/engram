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
```

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

## How It Works

1. **Discovery** - Scans `~/.claude/projects/` for JSONL conversation files
2. **Parsing** - Extracts user/assistant turns, tool calls, and metadata
3. **Archival** - Renders conversations as markdown with analytics
4. **Knowledge Extraction** - Uses an LLM to extract decisions, solutions, patterns, and preferences
5. **Synthesis** - Generates a `context.md` per project from accumulated knowledge

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
