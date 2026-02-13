# GitHub Gist Sharing for Claude Memory

Share your engram knowledge to private GitHub Gists for backup, collaboration, and cross-machine sync.

## Quick Start

```bash
# Share project knowledge to a private gist
engram sync push my-project

# Pull someone's shared knowledge
engram sync pull their-project <gist-id>

# List your gists
engram sync list my-project
```

## Commands

### `sync push` - Share Knowledge to Gist

Create or update a private GitHub Gist with your project knowledge.

```bash
# Create new private gist
engram sync push <project>

# Update existing gist
engram sync push <project> --gist-id abc123def456

# Custom description
engram sync push <project> --description "My team's best practices"
```

**What gets shared:**
- ✅ `decisions.md` - Architectural decisions
- ✅ `solutions.md` - Problem-solution pairs
- ✅ `patterns.md` - Code patterns and conventions
- ✅ `context.md` - Synthesized summary
- ✅ `metadata.json` - Project info, timestamps, tool version
- ❌ Preferences (excluded by default for privacy)
- ❌ Conversation archives (never shared)
- ❌ Learning state (local only)

**Output:**
```
✓ Syncing Creating new private gist...
✓ Done! Pushed my-project knowledge to gist
  Gist ID:  abc123def456
  URL:      https://gist.github.com/username/abc123def456

To pull on another machine:
  engram sync pull my-project abc123def456
```

---

### `sync pull` - Import Knowledge from Gist

Download and merge knowledge from someone's shared gist.

```bash
# Import from gist URL or ID
engram sync pull their-project abc123def456

# Overwrite local knowledge if conflicts
engram sync pull their-project abc123def456 --force
```

**What happens:**
1. Downloads all knowledge files from the gist
2. Writes to `~/memory/knowledge/their-project/`
3. Preserves local knowledge unless `--force` is used
4. Does NOT import preferences or learning state (local only)

**Conflict handling:**
- **Default**: Warns if project already exists, does not overwrite
- **With `--force`**: Overwrites existing knowledge files

---

### `sync list` - Show Your Gists

List all your GitHub Gists that contain engram knowledge.

```bash
engram sync list my-project
```

**Output:**
```
✓ Searching Listing gists for 'my-project'...

Found 2 engram gists:

1. abc123def456 (updated 2 days ago)
   Description: My best practices
   Files: decisions.md, solutions.md, patterns.md, context.md
   URL: https://gist.github.com/username/abc123def456

2. def456ghi789 (updated 1 week ago)
   Description: Team knowledge base
   Files: decisions.md, solutions.md, context.md
   URL: https://gist.github.com/username/def456ghi789
```

---

## Authentication

Claude-memory uses GitHub authentication for gist operations:

### Option 1: GitHub CLI (Recommended)

```bash
# Install gh CLI: https://cli.github.com
brew install gh  # macOS
# or download from https://github.com/cli/cli/releases

# Authenticate
gh auth login

# engram will automatically use gh token
engram sync push my-project
```

### Option 2: Environment Variable

```bash
# Get a personal access token from GitHub
# https://github.com/settings/tokens
# Scope needed: gist

export GITHUB_TOKEN=ghp_...
engram sync push my-project
```

### Option 3: GH_TOKEN Variable

```bash
export GH_TOKEN=ghp_...
engram sync push my-project
```

---

## Privacy & Security

### What's Shared

**Included by default:**
- Architectural decisions
- Problem solutions
- Code patterns
- Synthesized context

**Excluded by default:**
- User preferences (contain personal workflow habits)
- Conversation archives (private by design)
- Learning state (machine-specific)
- Analytics data (usage patterns)

### Secret Redaction

**Automatic redaction** (planned feature):
- API keys, tokens, passwords detected and removed
- Uses the same `SecretDetector` as the pack security system
- Secrets replaced with `[REDACTED]` placeholder

**Manual review recommended:**
- Always review gist content before sharing publicly
- Check for hardcoded credentials, internal URLs, proprietary info

### Gist Visibility

**Default: Private**
- Only accessible with the gist URL
- Not discoverable via GitHub search
- Shareable with specific collaborators

**Public gists** (future feature):
- Would require `--public` flag
- Discoverable by anyone
- Suitable for open-source knowledge packs

---

## Use Cases

### 1. Backup to Cloud

```bash
# Regular backup of critical project knowledge
engram sync push production-api --gist-id abc123
```

### 2. Share with Team

```bash
# Push once
engram sync push team-standards

# Share gist URL with team
# They pull:
engram sync pull team-standards <gist-id>
```

### 3. Cross-Machine Sync

```bash
# On work machine
engram sync push my-project
# → Get gist ID

# On home machine
engram sync pull my-project <gist-id>
```

### 4. Publish Best Practices

```bash
# Share engineering patterns publicly (future)
engram sync push rust-patterns --public
```

---

## Advanced Usage

### Selective Sharing (Future Feature)

```bash
# Share only specific categories
engram sync push my-project --categories decisions,patterns

# Exclude sensitive categories
engram sync push my-project --exclude preferences
```

### Version History

Gists automatically track version history:
- Every push creates a new revision
- View history on gist.github.com
- Download specific versions via GitHub API

### Gist Metadata

Each gist includes `metadata.json`:

```json
{
  "project": "my-project",
  "synced_at": "2026-02-13T10:00:00Z",
  "tool": "engram",
  "version": "0.3.0"
}
```

---

## Integration with Claude Code

### Auto-Inject After Pull

```bash
# Pull gist and inject into Claude Code in one flow
engram sync pull team-knowledge <gist-id>
engram inject team-knowledge
```

### Periodic Sync Hook (Future Feature)

Add to your session hooks to auto-push after sessions:

```bash
# ~/.claude/hooks/session-end.sh
engram sync push $PROJECT_NAME --gist-id $GIST_ID
```

---

## Troubleshooting

### "GitHub token not found"

**Solution:** Authenticate with GitHub CLI or set environment variable.

```bash
gh auth login
# or
export GITHUB_TOKEN=ghp_your_token_here
```

### "Project already exists"

**Solution:** Use `--force` to overwrite or choose a different project name.

```bash
engram sync pull their-project <gist-id> --force
```

### "Failed to push to gist"

**Possible causes:**
- Network connectivity issue
- Invalid gist ID
- Token expired or lacks `gist` scope
- Gist belongs to another user (read-only)

**Solution:** Check token permissions at https://github.com/settings/tokens

---

## Comparison: Gists vs. Hive Packs

| Feature | Gists | Hive Packs |
|---------|-------|------------|
| **Purpose** | Backup, personal sync, team sharing | Public distribution, versioned releases |
| **Discovery** | URL-based (private by default) | Git registries, searchable |
| **Updates** | Direct push/pull | Install, update, uninstall |
| **Privacy** | Private or public | Defined by pack manifest |
| **Versioning** | Automatic gist revisions | Semantic versioning |
| **Best for** | Personal backups, team collaboration | Community knowledge packs |

**When to use Gists:**
- Backing up your personal knowledge
- Sharing with specific collaborators (via URL)
- Quick cross-machine sync

**When to use Hive Packs:**
- Publishing to the community
- Stable, versioned knowledge releases
- Distributable best practices

---

## Future Enhancements

- [ ] `--public` flag for public gists
- [ ] `--categories` flag for selective sharing
- [ ] Automatic secret redaction before upload
- [ ] Gist version browsing and rollback
- [ ] Team gist organizations
- [ ] Conflict resolution for concurrent edits
- [ ] Integration with Claude Code session hooks

---

## Examples

### Example 1: Backup Critical Project

```bash
$ engram sync push production-api

✓ Syncing Creating new private gist...
✓ Done! Pushed production-api knowledge to gist
  Gist ID:  abc123def456789
  URL:      https://gist.github.com/youruser/abc123def456789

To pull on another machine:
  engram sync pull production-api abc123def456789
```

### Example 2: Team Knowledge Base

```bash
# Team lead creates gist
$ engram sync push team-onboarding
✓ Done! Pushed team-onboarding knowledge to gist
  Gist ID:  teamabc123
  URL:      https://gist.github.com/teamlead/teamabc123

# Team member imports
$ engram sync pull team-onboarding teamabc123
✓ Done! Pulled team-onboarding knowledge from gist
  4 files synced

$ engram recall team-onboarding
# ... shows all team knowledge ...
```

### Example 3: Update Existing Gist

```bash
# Make changes to knowledge locally
engram add my-project decisions "New architecture decision"

# Push update to existing gist
engram sync push my-project --gist-id abc123def456
✓ Syncing Updating gist abc123def456...
✓ Done! Pushed my-project knowledge to gist
```

---

## See Also

- [Hive Mind Guide](./HIVE.md) - Git-based knowledge pack distribution
- [TUI Guide](./TUI_GUIDE.md) - Interactive terminal browser
- [Publishing Guide](./PUBLISHING.md) - Creating public knowledge packs
