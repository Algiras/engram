# Multi-Agent Collaboration Demo

This demo shows how multiple AI agents can collaborate using a shared git repository for knowledge.

## Scenario

Three specialized agents working on the same project:
- **Agent A (Research)**: Gathers patterns and decisions
- **Agent B (Implementation)**: Adds solutions and code patterns
- **Agent C (Review)**: Consolidates and validates knowledge

## Setup

### 1. Initialize Shared Repository

```bash
# Create shared memory repo (can be local or remote)
claude-memory sync init-repo ~/team-memory

# Optional: Push to GitHub for distributed teams
cd ~/team-memory
git remote add origin git@github.com:yourteam/ai-memory.git
git push -u origin master
```

### 2. Each Agent Configuration

All agents use the same configuration:

```bash
# ~/.bashrc or ~/.zshrc
export SHARED_MEMORY_REPO="$HOME/team-memory"
export CLAUDE_MEMORY_AUTO_SYNC=true
```

## Workflow

### Agent A (Research Phase)

```bash
# Agent A: Extract decisions from recent work
claude-memory ingest --project myapp

# Push to shared repo
claude-memory sync push-repo myapp ~/team-memory \
  --message "Agent A: Added architectural decisions" \
  --push-remote
```

### Agent B (Implementation Phase)

```bash
# Agent B: Pull latest knowledge from Agent A
claude-memory sync pull-repo myapp ~/team-memory --fetch-remote

# Check what Agent A discovered
claude-memory recall myapp

# Work on implementation...
# Extract solutions
claude-memory ingest --project myapp

# Push solutions
claude-memory sync push-repo myapp ~/team-memory \
  --message "Agent B: Added implementation solutions" \
  --push-remote
```

### Agent C (Review Phase)

```bash
# Agent C: Get all knowledge from A and B
claude-memory sync pull-repo myapp ~/team-memory --fetch-remote

# Review consolidated knowledge
claude-memory recall myapp
claude-memory lookup myapp "authentication"

# Add patterns and consolidate
claude-memory add myapp patterns "Use JWT tokens with 1h expiry" --label "security-review"
claude-memory regen myapp

# Push consolidated knowledge
claude-memory sync push-repo myapp ~/team-memory \
  --message "Agent C: Consolidated security patterns" \
  --push-remote
```

## Complete Example

```bash
#!/bin/bash
# multi-agent-demo.sh

REPO="/tmp/ai-memory"
PROJECT="demo-app"

echo "🏗️  Setting up shared repository..."
claude-memory sync init-repo "$REPO"

echo
echo "🤖 Agent A (Research): Extracting architecture decisions..."
claude-memory add "$PROJECT" decisions "Use microservices architecture" --label "agent-a"
claude-memory sync push-repo "$PROJECT" "$REPO" --message "Agent A: Architecture decisions"

echo
echo "🤖 Agent B (Implementation): Pulling A's knowledge..."
# Simulate fresh environment
rm -rf ~/memory/knowledge/"$PROJECT"
claude-memory sync pull-repo "$PROJECT" "$REPO"
echo "   Agent B sees:"
claude-memory recall "$PROJECT" | head -10

echo "   Adding implementation solutions..."
claude-memory add "$PROJECT" solutions "Implemented service mesh with Istio" --label "agent-b"
claude-memory sync push-repo "$PROJECT" "$REPO" --message "Agent B: Implementation solutions"

echo
echo "🤖 Agent C (Review): Pulling knowledge from A and B..."
rm -rf ~/memory/knowledge/"$PROJECT"
claude-memory sync pull-repo "$PROJECT" "$REPO"
echo "   Agent C sees:"
claude-memory recall "$PROJECT" | head -20

echo "   Adding review patterns..."
claude-memory add "$PROJECT" patterns "Always version APIs with /v1/ prefix" --label "agent-c"
claude-memory sync push-repo "$PROJECT" "$REPO" --message "Agent C: API patterns"

echo
echo "📊 Final git history:"
cd "$REPO" && git log --oneline

echo
echo "📁 Final knowledge structure:"
cd "$REPO" && find "$PROJECT" -type f

echo
echo "✅ Multi-agent collaboration complete!"
echo
echo "All three agents contributed to shared knowledge:"
echo "  - Agent A: Architecture decisions"
echo "  - Agent B: Implementation solutions"
echo "  - Agent C: Review patterns"
```

## Real-World Scenarios

### Scenario 1: Continuous Development

**Morning:**
```bash
# Agent pulls latest knowledge
claude-memory sync pull-repo myapp ~/team-memory --fetch-remote
```

**During work:**
```bash
# Agent works and learns
claude-memory ingest --project myapp
```

**End of day:**
```bash
# Agent pushes new knowledge
claude-memory sync push-repo myapp ~/team-memory \
  --message "Daily update: $(date +%Y-%m-%d)" \
  --push-remote
```

### Scenario 2: Specialized Agents

```bash
# Setup
REPO="$HOME/team-memory"

# Testing Agent: Adds test patterns
claude-memory add myapp patterns "Use Jest with 80% coverage threshold" --label "testing-agent"
claude-memory sync push-repo myapp "$REPO"

# Security Agent: Adds security patterns
claude-memory sync pull-repo myapp "$REPO" --fetch-remote
claude-memory add myapp patterns "Validate all inputs with Joi schemas" --label "security-agent"
claude-memory sync push-repo myapp "$REPO"

# Performance Agent: Adds optimization patterns
claude-memory sync pull-repo myapp "$REPO" --fetch-remote
claude-memory add myapp patterns "Use Redis for session caching" --label "perf-agent"
claude-memory sync push-repo myapp "$REPO"
```

### Scenario 3: Human + AI Collaboration

```bash
# Human: Sets up project
git clone git@github.com:team/memory.git ~/team-memory
claude-memory sync pull-repo myapp ~/team-memory

# Human: Manually edits knowledge
cd ~/team-memory/myapp
vim decisions.md  # Add human insights

# Human: Commits changes
cd ~/team-memory
git add myapp/decisions.md
git commit -m "Human: Added business requirements"
git push

# AI Agent: Syncs and sees human's input
claude-memory sync pull-repo myapp ~/team-memory --fetch-remote
claude-memory recall myapp
# AI responds to human requirements...
```

## Advanced Features

### Branch-Based Workflows

```bash
# Experimental features in branch
cd ~/team-memory
git checkout -b experiment

claude-memory sync push-repo myapp ~/team-memory --message "Experimental: New approach"

# If successful, merge to main
git checkout main
git merge experiment
```

### Conflict Resolution

When multiple agents modify the same knowledge:

```bash
# Agent gets pull conflicts
claude-memory sync pull-repo myapp ~/team-memory --fetch-remote
# Error: merge conflict in decisions.md

# Manual resolution
cd ~/team-memory
git status
# Edit conflicting files
vim myapp/decisions.md
git add myapp/decisions.md
git commit -m "Resolved: Merged Agent A and B decisions"
git push

# Agent pulls resolved version
claude-memory sync pull-repo myapp ~/team-memory --fetch-remote
```

### Audit Trail

```bash
# See who changed what
cd ~/team-memory
git log --all --oneline myapp/

# View specific change
git show ae04d88

# Blame (find who added each line)
git blame myapp/patterns.md
```

## Integration with Claude Code

### Auto-Sync Hook

Add to `~/.claude/hooks/session-end-hook.sh`:

```bash
#!/bin/bash
PROJECT_NAME="$(basename "${CLAUDE_PROJECT_DIR:-$(pwd)}")"
SHARED_REPO="$HOME/team-memory"

# Pull latest
claude-memory sync pull-repo "$PROJECT_NAME" "$SHARED_REPO" --fetch-remote 2>/dev/null

# Ingest session
claude-memory ingest --project "$PROJECT_NAME" --since 1h

# Push updates
claude-memory sync push-repo "$PROJECT_NAME" "$SHARED_REPO" --push-remote 2>/dev/null &

exit 0
```

This automatically syncs after each Claude Code session!

## Performance Tips

### Large Repositories

For repos with many projects:

```bash
# Use git sparse checkout
git sparse-checkout init --cone
git sparse-checkout set myapp
```

### Reduce Sync Frequency

```bash
# Only sync on significant changes
claude-memory forget myapp --expired  # Clean up first
claude-memory sync push-repo myapp ~/team-memory
```

### Shallow Clones

For faster initial setup:

```bash
git clone --depth 1 git@github.com:team/memory.git ~/team-memory
```

## Benefits

✅ **Version control**: Full git history of knowledge evolution
✅ **Collaboration**: Multiple agents/humans work together
✅ **Offline**: Works without network
✅ **Self-hosted**: Keep sensitive knowledge on your infrastructure
✅ **Branching**: Experiment with different knowledge versions
✅ **Audit trail**: See who added what and when
✅ **Merge tools**: Resolve conflicts with git tools
✅ **CI/CD ready**: Integrate with existing workflows

## Comparison: Gist vs Git Repo

**Use Gists when:**
- Solo developer
- Simple backup/sync
- Quick sharing
- No conflicts expected

**Use Git Repo when:**
- Multiple agents/collaborators
- Need branching/merging
- Self-hosted requirement
- Large knowledge bases (>10MB)
- Audit trail needed

## Example: Real Multi-Agent System

```bash
# Orchestrator sets up
git clone git@gitlab.company.com:ai/memory.git ~/ai-memory

# Planning Agent
claude-memory sync pull-repo project ~/ai-memory --fetch-remote
# Adds decisions...
claude-memory sync push-repo project ~/ai-memory --push-remote

# Implementation Agents (parallel)
claude-memory sync pull-repo project ~/ai-memory --branch agent-impl-1
# Works on feature A...
claude-memory sync push-repo project ~/ai-memory --push-remote

# Review Agent (pulls all)
claude-memory sync pull-repo project ~/ai-memory --fetch-remote
# Reviews and consolidates...
claude-memory regen project
claude-memory sync push-repo project ~/ai-memory --push-remote

# Human reviews final state
cd ~/ai-memory
git log --graph --all --oneline
```

## Success!

You now have a complete multi-agent collaboration system with:
- ✅ Local git repository support
- ✅ GitHub gist backup
- ✅ Version history tracking
- ✅ Merge conflict resolution
- ✅ Distributed team workflows
- ✅ Integration with `gh` CLI

The knowledge is now **truly collaborative**! 🚀
