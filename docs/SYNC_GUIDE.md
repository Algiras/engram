# Sync Guide

Share and version your knowledge using GitHub Gists or Git repositories.

## Quick Start

### GitHub Gists (Recommended)

```bash
# 1. Set GitHub token
export GITHUB_TOKEN='your-token-here'

# 2. Push knowledge to a private gist
engram sync push my-project

# 3. On another machine, pull the knowledge
engram sync pull my-project <gist-id>
```

### Git Repository (Multi-agent collaboration)

```bash
# 1. Initialize a shared memory repo
engram sync init-repo ~/shared-memory

# 2. Push knowledge
engram sync push-repo my-project ~/shared-memory

# 3. Other agents pull changes
engram sync pull-repo my-project ~/shared-memory
```

## GitHub Gists

### Setup

1. **Create a GitHub Personal Access Token:**
   - Go to https://github.com/settings/tokens
   - Click "Generate new token" → "Generate new token (classic)"
   - Select scopes: `gist`
   - Generate and copy the token

2. **Set environment variable:**
   ```bash
   export GITHUB_TOKEN='ghp_...'
   # Add to ~/.bashrc or ~/.zshrc for persistence
   ```

### Commands

#### Push Knowledge

Create or update a private gist:

```bash
# First push (creates new gist)
engram sync push my-project

# Update existing gist
engram sync push my-project --gist-id abc123

# Custom description
engram sync push my-project --description "My Team's Project Knowledge"
```

#### Pull Knowledge

Download knowledge from a gist:

```bash
# Pull to existing project
engram sync pull my-project abc123

# Overwrite local knowledge
engram sync pull my-project abc123 --force

# Clone to new project
engram sync clone abc123 new-project
```

#### List Gists

Find gists for a project:

```bash
engram sync list my-project
```

#### Version History

View and restore previous versions:

```bash
# Show all versions
engram sync history abc123

# View specific version
engram sync history abc123 --version 1234567890abcdef

# Restore a version
engram sync pull my-project 1234567890abcdef
```

## Git Repository Sync

### When to Use Git Repos

- **Multi-agent collaboration**: Multiple AI agents working on the same project
- **Self-hosted**: Keep knowledge on your own infrastructure
- **Branch-based**: Use git branches for different knowledge versions
- **Audit trail**: Full git history with commits and authors

### Setup

```bash
# Create a shared memory repository
mkdir -p ~/shared-memory
cd ~/shared-memory
git init
git config user.name "AI Agent"
git config user.email "agent@example.com"

# Optional: Add remote
git remote add origin git@github.com:yourteam/shared-memory.git
```

### Commands

#### Push to Repository

```bash
# Push knowledge to git repo
engram sync push-repo my-project ~/shared-memory

# With custom commit message
engram sync push-repo my-project ~/shared-memory \
  --message "Updated patterns from session abc123"

# Push and sync to remote
engram sync push-repo my-project ~/shared-memory --push-remote
```

#### Pull from Repository

```bash
# Pull knowledge from git repo
engram sync pull-repo my-project ~/shared-memory

# Pull from specific branch
engram sync pull-repo my-project ~/shared-memory --branch develop

# Pull from remote first
engram sync pull-repo my-project ~/shared-memory --fetch-remote
```

#### Sync Workflow

For continuous collaboration:

```bash
# Agent 1: Make changes and push
engram ingest --project my-project
engram sync push-repo my-project ~/shared-memory --push-remote

# Agent 2: Pull changes before working
engram sync pull-repo my-project ~/shared-memory --fetch-remote
engram recall my-project
# ... work on project ...
engram sync push-repo my-project ~/shared-memory --push-remote
```

## Use Cases

### 1. Personal Backup

Use gists for simple backup:

```bash
# Push all projects to gists
for project in $(engram projects | grep -o '^\s*[^ ]*'); do
  engram sync push "$project" --description "$project knowledge backup"
done
```

### 2. Team Knowledge Base

Use git repo for team collaboration:

```bash
# Setup shared repo
git clone git@github.com:yourteam/knowledge.git ~/team-knowledge

# Each team member syncs
engram sync pull-repo project-x ~/team-knowledge --fetch-remote
# ... work ...
engram sync push-repo project-x ~/team-knowledge --push-remote
```

### 3. Multi-Machine Development

Sync between work and home:

```bash
# On work machine
engram sync push my-project
# Note the gist ID: abc123

# On home machine
engram sync pull my-project abc123
```

### 4. Knowledge Versioning

Track knowledge evolution:

```bash
# Push regularly to create version history
engram sync push my-project --gist-id abc123

# Later, view what changed
engram sync history abc123

# Restore old version if needed
engram sync pull my-project <old-version-id>
```

### 5. Multi-Agent System

Multiple AI agents collaborating:

```bash
# Central repo
git clone git@github.com:ai-team/memory.git ~/ai-memory

# Agent A (planning agent)
engram sync pull-repo project-x ~/ai-memory
# ... extract planning decisions ...
engram sync push-repo project-x ~/ai-memory

# Agent B (implementation agent)
engram sync pull-repo project-x ~/ai-memory --fetch-remote
# ... implement based on plans ...
engram sync push-repo project-x ~/ai-memory --push-remote

# Agent C (review agent)
engram sync pull-repo project-x ~/ai-memory --fetch-remote
# ... review and add patterns ...
```

## Comparison

| Feature | GitHub Gists | Git Repository |
|---------|--------------|----------------|
| **Setup** | Simple (just token) | Medium (git config) |
| **Privacy** | Private by default | Self-hosted option |
| **Versioning** | Built-in | Full git history |
| **Collaboration** | View-only sharing | Full collaboration |
| **Size limit** | ~10MB | Unlimited (self-hosted) |
| **Web UI** | GitHub gists UI | GitHub/GitLab/etc |
| **API access** | GitHub API | Git protocol |
| **Best for** | Personal, simple sync | Teams, multi-agent |

## Workflows

### Solo Developer

```bash
# Use gists for simplicity
engram sync push my-project
# Work continues...
engram sync push my-project --gist-id abc123
```

### Small Team (2-5 people)

```bash
# Use shared git repo
git clone git@github.com:team/knowledge.git ~/team-knowledge

# Daily workflow
engram sync pull-repo projects ~/team-knowledge --fetch-remote
# Work...
engram sync push-repo projects ~/team-knowledge --push-remote
```

### Multi-Agent System

```bash
# Central orchestrator sets up repo
git clone git@github.com:org/ai-memory.git ~/ai-memory

# Each agent has a role
engram sync pull-repo project ~/ai-memory --branch agent-a
# Work...
engram sync push-repo project ~/ai-memory --branch agent-a

# Merge knowledge
cd ~/ai-memory
git checkout main
git merge agent-a agent-b agent-c
```

## Security

### Gist Security

- **Private gists** are only visible to you (and those you share the link with)
- **Tokens** should be kept secret (use `chmod 600` on files containing tokens)
- **Revoke tokens** if compromised: https://github.com/settings/tokens

### Git Repository Security

- **Use SSH keys** for authentication
- **Encrypt sensitive knowledge** before committing
- **Use private repos** on GitHub/GitLab
- **Self-host** for maximum control

### Encryption

For sensitive projects, encrypt before syncing:

```bash
# Export and encrypt
engram export my-project json | \
  gpg -e -r team@example.com | \
  base64 > knowledge.enc

# Upload encrypted file
# (knowledge remains encrypted in gist/repo)
```

## Troubleshooting

### "GitHub token not found"

**Solution:** Set GITHUB_TOKEN or GH_TOKEN:
```bash
export GITHUB_TOKEN='your-token-here'
```

### "Permission denied"

**Solution:** Token needs `gist` scope. Create new token with correct permissions.

### "Gist not found"

**Solution:** Check gist ID and ensure it's your gist or publicly accessible.

### Merge Conflicts (Git repo)

**Solution:** Resolve conflicts manually:
```bash
cd ~/shared-memory
git status
# Edit conflicting files
git add .
git commit
```

### Large Gist Size

**Solution:** Gists have ~10MB limit. For larger knowledge bases, use git repositories or split into multiple projects.

## Advanced

### Automated Sync

Add to cron or git hooks:

```bash
# crontab: sync every hour
0 * * * * engram sync push my-project --gist-id abc123

# git post-commit hook
#!/bin/bash
engram ingest
engram sync push my-project --gist-id abc123
```

### CI/CD Integration

```yaml
# GitHub Actions
- name: Sync Knowledge
  env:
    GITHUB_TOKEN: ${{ secrets.GIST_TOKEN }}
  run: |
    engram ingest
    engram sync push my-project
```

### Custom Sync Script

```bash
#!/bin/bash
# sync-all.sh - Sync all projects

for project in $(engram projects | grep -o '^\s*[^ ]*'); do
  echo "Syncing $project..."
  engram sync push "$project" || true
done
```

## Future Features

Planned enhancements:
- [ ] Differential sync (only changed files)
- [ ] Conflict resolution UI
- [ ] Automatic merge strategies
- [ ] Sync to S3/Cloud storage
- [ ] End-to-end encryption
- [ ] Real-time sync via websockets

## Feedback

Questions or suggestions? [Open an issue](https://github.com/Algiras/engram/issues)!
