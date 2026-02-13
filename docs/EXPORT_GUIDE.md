# Export Guide

Export your project knowledge to various formats for sharing, documentation, or backup.

## Quick Start

```bash
# Export to markdown (stdout)
engram export my-project markdown

# Export to JSON file
engram export my-project json -o knowledge.json

# Export to standalone HTML
engram export my-project html -o knowledge.html
```

## Supported Formats

### Markdown

**Use when:** Creating documentation, README files, or wiki pages.

**Features:**
- Clean, readable format
- Compatible with GitHub, GitLab, etc.
- Easy to version control
- Can be converted to other formats (PDF, HTML) via pandoc

**Example:**
```bash
engram export my-project markdown -o PROJECT_KNOWLEDGE.md
```

**Output structure:**
```markdown
# Project - Knowledge Export

**Exported:** 2026-02-12 14:37:05 UTC

---

## Project Context
[Synthesized overview]

## Decisions
[Key decisions with session timestamps]

## Solutions
[Problem-solution pairs]

## Patterns
[Code patterns and conventions]
```

### JSON

**Use when:** Programmatic access, data processing, or integration with other tools.

**Features:**
- Structured data format
- Includes metadata
- Easy to parse programmatically
- Can be imported into databases or analytics tools

**Example:**
```bash
engram export my-project json -o knowledge.json
```

**Output structure:**
```json
{
  "project": "my-project",
  "exported_at": "2026-02-12T14:37:05Z",
  "tool": "engram",
  "tool_url": "https://github.com/Algiras/engram",
  "knowledge": {
    "context": "...",
    "decisions": "...",
    "solutions": "...",
    "patterns": "..."
  },
  "conversations": []
}
```

### HTML

**Use when:** Creating shareable, standalone documentation.

**Features:**
- Self-contained webpage with styling
- Built-in search functionality
- No dependencies required
- Works offline
- Professional appearance

**Example:**
```bash
engram export my-project html -o knowledge.html
```

**Features:**
- Responsive design
- Client-side search
- Syntax highlighting (for code blocks)
- Clean, readable layout
- Works in any browser

## Options

### Include Conversations

By default, exports only include synthesized knowledge (decisions, solutions, patterns). To include full conversation archives:

```bash
engram export my-project markdown --include-conversations > full_export.md
```

**Warning:** This can produce very large files (100MB+) for projects with many sessions.

### Output to File

Use `-o` or `--output` to write to a file instead of stdout:

```bash
engram export my-project json -o knowledge.json
```

Without `-o`, output goes to stdout for piping:

```bash
engram export my-project markdown | pandoc -o knowledge.pdf
```

## Common Use Cases

### 1. Project Documentation

Export to markdown and include in your repository:

```bash
engram export my-project markdown -o docs/KNOWLEDGE.md
git add docs/KNOWLEDGE.md
git commit -m "docs: Add project knowledge base"
```

### 2. Team Sharing

Export to HTML and share the file:

```bash
engram export my-project html -o knowledge.html
# Share knowledge.html via email, Slack, or file server
```

### 3. Backup

Export to JSON for archival:

```bash
engram export my-project json -o backup/$(date +%Y%m%d)-knowledge.json
```

### 4. Data Analysis

Export to JSON and analyze:

```python
import json

with open('knowledge.json') as f:
    data = json.load(f)

print(f"Project: {data['project']}")
print(f"Decisions: {len(data['knowledge']['decisions'])}")
```

### 5. PDF Generation

Export to markdown and convert:

```bash
engram export my-project markdown | \
  pandoc -o knowledge.pdf \
  --pdf-engine=xelatex \
  --toc \
  --number-sections
```

### 6. Static Site

Export to HTML and deploy:

```bash
engram export my-project html -o public/knowledge.html
# Deploy public/ to GitHub Pages, Netlify, etc.
```

## Advanced Usage

### Export Multiple Projects

```bash
for project in $(engram projects | grep -o '^\s*[^ ]*'); do
  engram export "$project" html -o "exports/${project}.html"
done
```

### Automated Exports

Add to cron or CI/CD:

```bash
# crontab
0 2 * * * engram export my-project json -o /backup/knowledge.json

# GitHub Actions
- run: engram export my-project markdown -o docs/KNOWLEDGE.md
```

### Custom Processing

Pipe through jq for JSON:

```bash
engram export my-project json | \
  jq '.knowledge.decisions' | \
  grep -o '"decision": "[^"]*"'
```

### Diff Between Exports

Track knowledge evolution:

```bash
engram export my-project markdown -o knowledge-old.md
# ... make changes ...
engram ingest --project my-project
engram export my-project markdown -o knowledge-new.md
diff -u knowledge-old.md knowledge-new.md
```

## Format Comparison

| Format | Size | Readable | Searchable | Portable | Programmable |
|--------|------|----------|------------|----------|--------------|
| **Markdown** | Small | ✅ High | ✅ Via tools | ✅ High | ⚠️  Medium |
| **JSON** | Medium | ⚠️  Low | ✅ Via jq | ✅ High | ✅ High |
| **HTML** | Large | ✅ High | ✅ Built-in | ✅ High | ⚠️  Medium |

## Tips

### Reduce File Size

For large projects:
```bash
# Export only knowledge (no conversations)
engram export my-project markdown -o knowledge.md

# Clean up old sessions first
engram forget my-project --expired
```

### Better Markdown

Use pandoc for enhanced markdown:

```bash
engram export my-project markdown | \
  pandoc -f markdown -t gfm \
  -o README.md
```

### Searchable PDF

Convert HTML to PDF (preserves search):

```bash
engram export my-project html -o knowledge.html
wkhtmltopdf knowledge.html knowledge.pdf
```

### Share Securely

For sensitive projects, encrypt exports:

```bash
engram export my-project json -o - | \
  gpg -e -r recipient@example.com \
  > knowledge.json.gpg
```

## Troubleshooting

### Empty Export

**Problem:** Export file is empty or shows "No knowledge found"

**Solution:** Run ingest first:
```bash
engram ingest --project my-project
engram export my-project markdown
```

### Large File Size

**Problem:** Export is too large to open

**Solution:** Don't include conversations:
```bash
engram export my-project html  # without --include-conversations
```

### Missing Sessions

**Problem:** Some sessions are missing from export

**Solution:** TTL may have expired them. Check:
```bash
engram lookup my-project "" --all
```

### Formatting Issues

**Problem:** HTML doesn't look right

**Solution:** Open in a modern browser (Chrome, Firefox, Safari). Avoid IE.

## Integration

### Obsidian

Export to markdown vault:

```bash
engram export my-project markdown -o ~/Obsidian/Projects/my-project.md
```

### Notion

1. Export to markdown
2. Import markdown file into Notion
3. Use Notion's import feature

### Confluence

1. Export to HTML
2. Use Confluence's HTML import
3. Or convert HTML → Markdown → Confluence

### Documentation Sites

Integrate with MkDocs, Docusaurus, etc.:

```yaml
# mkdocs.yml
nav:
  - Knowledge: knowledge.md

# In CI
- run: engram export my-project markdown -o docs/knowledge.md
```

## Future Enhancements

Planned export features:
- PDF export via built-in renderer
- Anki deck generation (spaced repetition)
- EPUB for ebook readers
- MediaWiki format
- Custom templates
- Filtering by date/category

## Feedback

Have a use case we didn't cover? [Open an issue](https://github.com/Algiras/engram/issues)!
