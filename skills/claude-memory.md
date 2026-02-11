---
name: claude-memory
description: Conversation memory system for Claude Code. Archive JSONL conversations to markdown, extract knowledge (decisions, solutions, patterns) via LLMs, track tool usage analytics, and search across all sessions. Supports Anthropic, OpenAI, and Ollama providers.
license: MIT
metadata:
  author: Algiras
  version: "0.1.0"
compatibility: Requires Rust/Cargo for building. Supports Anthropic, OpenAI, or Ollama for LLM knowledge extraction.
---

# claude-memory - Conversation Memory System

Long-term memory for Claude Code. Parses conversation history from `~/.claude/projects/`, extracts useful knowledge, and makes it searchable and reusable.

## Installation

```bash
# Clone and build
git clone https://github.com/Algiras/claude-memory.git
cd claude-memory
cargo install --path .
```

## Quick Start

```bash
# Archive all conversations (fast, no LLM needed)
claude-memory ingest --skip-knowledge

# See what projects you have
claude-memory projects

# Search across all archived conversations
claude-memory search "marketplace"

# Full extraction with LLM knowledge
claude-memory ingest

# View extracted project context
claude-memory recall claudius
```

## LLM Providers

Supports Anthropic, OpenAI, and Ollama. Defaults to Ollama (local) if nothing is configured.

**Precedence:** environment variables > auth.json > Ollama fallback

```bash
# Interactive login
claude-memory auth login

# Direct login
claude-memory auth login --provider anthropic

# Check active provider
claude-memory auth status

# List all configured providers
claude-memory auth list

# Override per-command
claude-memory ingest --provider ollama

# Or use environment variables
ANTHROPIC_API_KEY=sk-ant-... claude-memory ingest
```

Credentials stored in `~/.config/claude-memory/auth.json` with 0600 permissions.

## Commands

### `ingest` - Parse and archive conversations

```bash
claude-memory ingest [OPTIONS]

Options:
  --force              Re-process everything (ignore cache)
  --dry-run            Preview what would be processed
  --project <NAME>     Process only a specific project
  --since <DURATION>   Only recent sessions (e.g., "1d", "2h", "30m")
  --skip-knowledge     Archive only, skip LLM extraction
  --provider <NAME>    LLM provider override (anthropic, openai, ollama)
```

Reads JSONL files from `~/.claude/projects/` and produces:
- `~/memory/conversations/{project}/{session}/conversation.md` - Clean markdown
- `~/memory/conversations/{project}/{session}/meta.json` - Machine-readable metadata
- `~/memory/summaries/{project}/{session}.md` - Brief summaries
- `~/memory/analytics/usage.json` - Tool usage statistics
- `~/memory/analytics/activity.json` - Project activity timeline

With LLM extraction:
- `~/memory/knowledge/{project}/context.md` - Project context summary
- `~/memory/knowledge/{project}/decisions.md` - Technical decisions
- `~/memory/knowledge/{project}/solutions.md` - Problems and solutions
- `~/memory/knowledge/{project}/patterns.md` - Codebase patterns
- `~/memory/knowledge/_global/preferences.md` - User preferences

### `search` - Full-text search

```bash
claude-memory search <QUERY> [OPTIONS]

Options:
  --project <NAME>     Limit to a project
  --knowledge          Search only knowledge/ files
  -c, --context <N>    Lines of context (default: 2)
```

### `recall` / `context` - View project knowledge

```bash
# Pretty-printed
claude-memory recall <PROJECT>

# Raw stdout (for piping)
claude-memory context <PROJECT>

# Pipe into CLAUDE.md or prompts
claude-memory context myproject >> .claude/MEMORY.md
```

### `auth` - Manage LLM providers

```bash
claude-memory auth login [--provider <NAME>] [--set-default]
claude-memory auth list
claude-memory auth logout <PROVIDER>
claude-memory auth status
```

### `status` / `projects` - Overview

```bash
claude-memory status     # Memory stats
claude-memory projects   # List all projects with activity
```

## Configuration

Environment variables:
- `ANTHROPIC_API_KEY` - Anthropic API key (auto-selects Anthropic provider)
- `OPENAI_API_KEY` - OpenAI API key (auto-selects OpenAI provider)
- `CLAUDE_MEMORY_LLM_ENDPOINT` - Override LLM endpoint
- `CLAUDE_MEMORY_LLM_MODEL` - Override LLM model

## Auto-Archiving Hook

Archive conversations in the background using a PostToolUse hook:

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

## Storage Layout

```
~/memory/
  _manifest.json              # Ingestion state
  conversations/              # Archived conversations as markdown
  knowledge/                  # LLM-extracted knowledge per project
  analytics/                  # Tool usage and activity stats
  summaries/                  # Brief session summaries
```
