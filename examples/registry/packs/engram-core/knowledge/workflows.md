# Workflows

## Session: initial-setup-workflow (2026-02-12T00:00:00Z) [ttl:never]

### First-Time Setup Workflow

**Complete setup for new users:**

```bash
# 1. Configure LLM provider
engram auth login
# Select provider, enter API key, set as default

# 2. Extract knowledge from existing conversations
engram ingest
# Processes all JSONL archives in ~/.claude/projects/

# 3. View extracted knowledge
engram projects          # List all projects
engram recall <project>  # View knowledge for a project

# 4. Install hooks for automatic extraction
engram hooks install
# Now knowledge extracts automatically after each session

# 5. Inject into Claude Code (optional)
cd /path/to/your/project
engram inject
# Creates .claude/memory/MEMORY.md
```

## Session: daily-workflow (2026-02-12T00:00:00Z) [ttl:never]

### Daily Development Workflow

**Typical daily usage with hooks installed:**

```bash
# Work with Claude Code normally...
# (hooks automatically extract knowledge after sessions)

# Periodically review accumulated knowledge
engram recall <current-project>

# Search for specific information
engram search "error handling"
engram lookup <project> "authentication"

# Clean up expired knowledge (weekly)
engram forget <project> --expired

# Update learning optimizations (monthly)
engram learn optimize <project>
```

## Session: knowledge-sharing-workflow (2026-02-12T00:00:00Z) [ttl:never]

### Sharing Knowledge via Hive Mind

**Creating and publishing a knowledge pack:**

```bash
# 1. Create pack structure
mkdir -p my-pack/{.pack,knowledge}

# 2. Create manifest
cat > my-pack/.pack/manifest.json << 'EOF'
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
EOF

# 3. Add knowledge files
cp ~/memory/knowledge/<project>/patterns.md my-pack/knowledge/
cp ~/memory/knowledge/<project>/solutions.md my-pack/knowledge/

# 4. Review and redact sensitive info
vim my-pack/knowledge/*.md

# 5. Push to Git
cd my-pack
git init
git add .
git commit -m "Initial pack"
git remote add origin https://github.com/user/my-pack
git push -u origin main

# 6. Share the registry URL
# Users can now: engram hive registry add user/my-pack
```

## Session: consuming-packs-workflow (2026-02-12T00:00:00Z) [ttl:never]

### Using Shared Knowledge Packs

**Installing and using community packs:**

```bash
# 1. Add a registry
engram hive registry add rust-lang/rust-patterns

# 2. Browse available packs
engram hive browse
engram hive browse --category patterns

# 3. Search for specific knowledge
engram hive search "async"

# 4. Install a pack
engram hive install rust-best-practices

# 5. Use the knowledge
engram recall my-rust-project
# Now includes knowledge from installed packs!

# 6. Keep packs updated
engram hive update          # Update all packs
engram hive update rust-best-practices  # Update specific pack

# 7. Remove if no longer needed
engram hive uninstall rust-best-practices
```

## Session: multi-project-workflow (2026-02-12T00:00:00Z) [ttl:never]

### Managing Multiple Projects

**Best practices for multi-project setups:**

```bash
# Extract knowledge for specific projects
engram ingest --project frontend
engram ingest --project backend

# View all projects
engram projects

# Compare knowledge across projects
engram recall frontend > /tmp/frontend.txt
engram recall backend > /tmp/backend.txt
diff /tmp/frontend.txt /tmp/backend.txt

# Search across all projects
engram search "authentication"  # Searches all

# Project-specific search
engram search "authentication" --project backend

# Consolidate duplicate knowledge
engram consolidate <project> --threshold 0.9

# Global preferences (shared across all projects)
# Stored in ~/memory/knowledge/_global/preferences.md
```

## Session: debugging-workflow (2026-02-12T00:00:00Z) [ttl:never]

### Debugging Knowledge Issues

**When knowledge isn't extracting or appearing correctly:**

```bash
# 1. Check system health
engram doctor [project]
engram doctor --verbose  # Detailed diagnostics

# 2. Verify conversation archives
ls -la ~/.claude/projects/
# Should contain JSONL files

# 3. Test ingestion with dry-run
engram ingest --dry-run --project <name>

# 4. Force re-ingestion
engram ingest --force --project <name>

# 5. Check knowledge files directly
ls -la ~/memory/knowledge/<project>/
cat ~/memory/knowledge/<project>/patterns.md

# 6. View analytics for usage patterns
engram analytics [project] --detailed

# 7. Check learning metrics
engram learn dashboard

# 8. Verify LLM provider
engram auth status

# 9. Test with minimal example
echo '{"role": "user", "content": "test"}' | \
  engram ingest --project test-debug
```

## Session: migration-workflow (2026-02-12T00:00:00Z) [ttl:never]

### Migrating to New Machine

**Moving engram to a new computer:**

```bash
# On old machine: Export knowledge
cd ~/memory
tar -czf engram-backup.tar.gz knowledge/ hive/

# Transfer file to new machine
scp engram-backup.tar.gz user@newmachine:~/

# On new machine: Install engram
cargo install --path /path/to/engram

# Restore knowledge
cd ~
mkdir -p memory
cd memory
tar -xzf ~/engram-backup.tar.gz

# Reconfigure authentication
engram auth login

# Reinstall hooks
engram hooks install

# Verify
engram projects
engram hive list
```

**Note:** Conversation archives in `~/.claude/projects/` should sync automatically via Claude Code.
