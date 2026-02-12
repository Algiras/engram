# Solutions

## Session: missing-context-fix (2026-02-12T00:00:00Z) [ttl:never]

**Problem:** Context.md is empty or stale after ingestion

**Solution:** Run regeneration to force context synthesis

```bash
claude-memory regen <project>
```

This re-synthesizes `context.md` from existing knowledge files without re-extracting from conversations.

**Root Cause:** Context generation can fail if:
- Knowledge files are empty
- LLM provider is unavailable
- Previous synthesis was interrupted

## Session: expired-knowledge-cleanup (2026-02-12T00:00:00Z) [ttl:never]

**Problem:** Accumulated expired TTL entries clutter knowledge files

**Solution:** Use the forget command with --expired flag

```bash
claude-memory forget <project> --expired
```

This removes all entries where TTL has elapsed, keeping knowledge files clean.

**Best Practice:** Run this periodically (e.g., weekly) or add to a cron job.

## Session: project-not-found (2026-02-12T00:00:00Z) [ttl:never]

**Problem:** "Project not found" despite having conversations

**Solution:** Project names are derived from Claude Code project directories

Check:
1. List available projects: `claude-memory projects`
2. Verify conversation archives exist: `ls ~/.claude/projects/`
3. Run ingest: `claude-memory ingest`

**Note:** Project names are the basename of the directory path in Claude Code's projects folder.

## Session: hook-not-triggering (2026-02-12T00:00:00Z) [ttl:never]

**Problem:** Hooks not automatically extracting knowledge after sessions

**Solution:** Verify hook installation and settings

```bash
# Check hook status
claude-memory hooks status

# Reinstall if needed
claude-memory hooks install

# Verify in Claude Code settings
cat ~/.claude/settings.json | grep session-end-script
```

**Common Issues:**
- Settings permissions (hook blocked by permission mode)
- Script path incorrect
- Hook execution fails silently

**Debug:** Run hook script manually:
```bash
~/.claude/hooks/session-end.sh <session-id>
```

## Session: memory-not-injected (2026-02-12T00:00:00Z) [ttl:never]

**Problem:** Injected memory not appearing in Claude Code sessions

**Solution:** Check injection path and file permissions

```bash
# Inject for current directory
claude-memory inject

# Verify file created
ls -la .claude/memory/MEMORY.md

# Check content
cat .claude/memory/MEMORY.md
```

**Requirements:**
- Must be in a project directory (or specify `--project`)
- .claude/memory/ directory must exist
- MEMORY.md must have valid content

**Note:** Claude Code reads `.claude/memory/MEMORY.md` automatically for project memory.

## Session: llm-provider-auth (2026-02-12T00:00:00Z) [ttl:never]

**Problem:** "No LLM provider configured" error

**Solution:** Configure authentication for LLM provider

```bash
# Interactive login
claude-memory auth login

# Or set environment variables
export ANTHROPIC_API_KEY=sk-...
export OPENAI_API_KEY=sk-...

# Check status
claude-memory auth status
```

**Supported Providers:**
- Anthropic (Claude) - env: `ANTHROPIC_API_KEY`
- OpenAI (GPT) - env: `OPENAI_API_KEY`
- Ollama (local) - runs on localhost:11434

## Session: pack-registry-not-cloning (2026-02-12T00:00:00Z) [ttl:never]

**Problem:** Registry add fails with git clone error

**Solution:** Verify Git access and URL format

```bash
# Test git access
git ls-remote <registry-url>

# Use GitHub shorthand
claude-memory hive registry add owner/repo

# Or full HTTPS URL
claude-memory hive registry add https://github.com/owner/repo.git

# Check for SSH vs HTTPS issues
```

**Common Causes:**
- Private repo without authentication
- Invalid repository URL
- Git not installed or in PATH
- Network connectivity issues

## Session: pack-not-found-in-recall (2026-02-12T00:00:00Z) [ttl:never]

**Problem:** Installed pack knowledge not appearing in recall

**Solution:** Verify pack installation and knowledge files

```bash
# List installed packs
claude-memory hive list

# Check pack directory
ls ~/memory/packs/installed/<pack-name>/knowledge/

# Force re-read by rerunning recall
claude-memory recall <project>
```

**Requirements:**
- Pack must be installed (not just added to registry)
- Pack must have knowledge files in `knowledge/` directory
- Knowledge files must contain non-empty content
