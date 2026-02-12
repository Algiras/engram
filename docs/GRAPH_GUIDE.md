# Knowledge Graph Guide

Build and query **brain-like associative networks** from your knowledge.

## What is a Knowledge Graph?

A knowledge graph represents concepts and their relationships as a **network** (like neurons and synapses in the brain):

```
        MCP
         ↑
         │ uses
         │
Claude Code ──implements──> Conversation Memory
                                    │
                                    │ part-of
                                    ↓
                            Knowledge Injection
```

This enables **associative retrieval** - finding related concepts even when not explicitly linked in text!

## Quick Start

```bash
# 1. Build graph from existing knowledge
claude-memory graph build my-project

# 2. Query related concepts
claude-memory graph query my-project "authentication"

# 3. Visualize
claude-memory graph viz my-project ascii

# 4. Find hubs (central concepts)
claude-memory graph hubs my-project
```

## Commands

### Build Graph

Extract concepts and relationships using LLM:

```bash
# Build with default provider
claude-memory graph build my-project

# Use specific provider for better quality
claude-memory graph build my-project --provider anthropic
```

**What it does:**
1. Reads all knowledge files (context, decisions, solutions, patterns)
2. Uses LLM to extract concepts and relationships
3. Builds graph structure with importance scores
4. Saves to `knowledge/my-project/graph.json`

### Query Related Concepts

Find concepts connected to a given concept:

```bash
# Find concepts within 2 hops
claude-memory graph query my-project "authentication" --depth 2

# Output:
#   [1] OAuth (importance: 0.9)
#   [1] Security (importance: 0.8)
#     [2] JWT (importance: 0.7)
#     [2] Tokens (importance: 0.7)
```

**Use cases:**
- Explore related decisions
- Find connected patterns
- Understand concept clusters

### Find Shortest Path

Discover conceptual connections:

```bash
claude-memory graph path my-project "database" "performance"

# Output:
#   [0] Database
#   [1] Indexing
#   [2] Query Optimization
#   [3] Performance
```

**Use cases:**
- Understand how concepts relate
- Find missing links in knowledge
- Discover unexpected connections

### Find Hubs

Identify most connected concepts (like neuron hubs in the brain):

```bash
claude-memory graph hubs my-project --top 10

# Output:
#   1. Authentication (5 incoming, 3 outgoing, importance: 0.9)
#   2. API Design (4 incoming, 6 outgoing, importance: 0.8)
#   3. Security (7 incoming, 2 outgoing, importance: 0.9)
```

**Use cases:**
- Find central architectural concepts
- Identify knowledge bottlenecks
- Prioritize documentation

### Visualize

Create visual representations:

```bash
# ASCII tree (terminal-friendly)
claude-memory graph viz my-project ascii

# ASCII from specific concept
claude-memory graph viz my-project ascii --root "authentication"

# DOT format (for graphviz)
claude-memory graph viz my-project dot -o graph.dot

# SVG (requires graphviz installed)
claude-memory graph viz my-project svg -o graph.svg
```

## Graph Structure

### Concept Properties

Each node has:
- **ID**: Unique identifier (lowercase-kebab-case)
- **Name**: Display name
- **Category**: technology, pattern, decision, solution, problem, tool, person, other
- **Description**: Brief explanation
- **Importance**: 0.0 (trivial) to 1.0 (critical)
- **Source sessions**: Which conversations mentioned it

### Relationship Types

Eight types of edges (like different neurotransmitters!):

| Type | Meaning | Example | Color |
|------|---------|---------|-------|
| **Implements** | A implements B | "OAuth implements Authentication" | Blue |
| **Uses** | A uses B | "API uses Authentication" | Green |
| **RelatesTo** | A relates to B | "Security relates to Performance" | Gray |
| **Causes** | A causes B | "Caching causes Performance" | Red |
| **PartOf** | A is part of B | "JWT part of Authentication" | Purple |
| **DependsOn** | A depends on B | "API depends on Database" | Orange |
| **Supersedes** | A replaces B | "OAuth2 supersedes OAuth1" | Brown |
| **Contradicts** | A conflicts with B | "Pattern A contradicts Pattern B" | Dark Red |

### Relationship Strength

Each edge has strength (like synaptic weight):
- **0.9-1.0**: Very strong connection
- **0.7-0.9**: Strong connection
- **0.4-0.7**: Medium connection
- **0.0-0.4**: Weak connection

Visualized as:
- Bold lines (strong)
- Solid lines (medium)
- Dashed lines (weak)

## How It Works

### 1. LLM Extraction

```
Knowledge Files → LLM Prompt → Structured JSON
                      ↓
              {
                concepts: [...],
                relationships: [...]
              }
```

The LLM analyzes your knowledge and extracts:
- Key concepts mentioned
- How they relate to each other
- Importance scores
- Relationship strengths

### 2. Graph Construction

```
JSON → Nodes (concepts) + Edges (relationships) → petgraph
```

Uses the `petgraph` library for efficient graph operations.

### 3. Querying

```
BFS/DFS Traversal → Find connected concepts
Dijkstra → Shortest path
Degree Counting → Find hubs
```

## Use Cases

### 1. Understand Architecture

```bash
# Find all architectural decisions
claude-memory graph hubs my-project --top 20

# See how they connect
claude-memory graph viz my-project ascii
```

### 2. Discover Hidden Connections

```bash
# You mentioned "caching" and "performance" - are they linked?
claude-memory graph path my-project "caching" "performance"
```

### 3. Find Related Work

```bash
# Working on authentication? See related concepts
claude-memory graph query my-project "authentication" --depth 2
```

### 4. Knowledge Gaps

```bash
# Find isolated concepts (not well connected)
claude-memory graph viz my-project dot -o graph.dot
# Look for orphan nodes
```

### 5. Multi-Project Analysis

```bash
# Build graphs for all projects
for proj in $(claude-memory projects | grep -o '^\s*[^ ]*'); do
  claude-memory graph build "$proj"
done

# Find cross-project patterns
# (future: merge graphs)
```

## Brain-Like Behavior

### Spreading Activation

When you query a concept, the system traverses connections (like neural activation spreading):

```
Query: "authentication"
  ↓
Activate: authentication node
  ↓
Spread to: OAuth, JWT, Security (depth 1)
  ↓
Spread to: Tokens, Sessions, HTTPS (depth 2)
```

### Hub Concepts (Like Brain Regions)

Some concepts are highly connected (like prefrontal cortex for executive functions):

```
Authentication ←→ 8 concepts (central hub!)
Performance ←→ 6 concepts (important!)
Logging ←→ 2 concepts (peripheral)
```

### Synaptic Strength

Frequently co-occurring concepts have stronger connections:

```
Strong: API ─[0.95]─> Authentication  (almost always together)
Medium: API ─[0.6]─> Caching          (sometimes together)
Weak: API ─[0.3]─> Logging            (loosely related)
```

## Advanced Features

### Graph Merging (Multi-Agent)

When multiple agents build graphs:

```bash
# Agent A builds graph
claude-memory graph build my-project --provider anthropic

# Agent B adds to graph
# (currently creates new graph - future: merge)
```

### Temporal Graph Evolution

Track how concepts evolve:

```bash
# Version 1 (old graph)
git checkout v1.0
claude-memory graph build my-project
cp graph.json graph-v1.json

# Version 2 (new graph)
git checkout v2.0
claude-memory graph build my-project
cp graph.json graph-v2.json

# Compare (future: automated diff)
```

### Subgraph Extraction

Focus on specific areas:

```bash
# Build graph
claude-memory graph build my-project

# Extract authentication subgraph
claude-memory graph query my-project "authentication" --depth 3 > auth-subgraph.txt
```

## Visualization

### ASCII (Terminal)

```
📊 Knowledge Graph: claude-memory

🔵 Claude Memory (importance: 0.9)
├── ▶ Model Context Protocol (implements)
├── ▶ Conversation Memory System (implements)
└── → Rust (uses)
```

### DOT (Graphviz)

```bash
claude-memory graph viz my-project dot -o graph.dot
dot -Tsvg graph.dot -o graph.svg
open graph.svg
```

Creates professional graph visualizations with:
- Color-coded nodes by category
- Line thickness shows relationship strength
- Node size reflects importance
- Legend included

### Interactive (Future)

```bash
# Future: web-based interactive explorer
claude-memory graph serve my-project --port 8080
# Opens browser with D3.js force-directed graph
```

## Integration

### MCP Server

The graph is accessible via MCP:

```javascript
// Claude Desktop can now:
await mcp.call("graph_query", {
  project: "my-project",
  concept: "authentication",
  depth: 2
});
```

### TUI Integration (Future)

```bash
claude-memory tui
# Press 'g' to view graph mode
# Navigate relationships interactively
```

## Tips

### Better Graphs

1. **Use better models**: Anthropic > Gemini > Ollama
   ```bash
   claude-memory graph build myapp --provider anthropic
   ```

2. **More knowledge = Better graph**:
   ```bash
   claude-memory ingest --project myapp
   claude-memory graph build myapp
   ```

3. **Rebuild after major changes**:
   ```bash
   claude-memory ingest --project myapp
   claude-memory graph build myapp  # Regenerate
   ```

### Performance

- **Build time**: ~5-30s depending on LLM provider
- **Query time**: <10ms (in-memory graph)
- **Graph size**: Typically 10-50 concepts, 20-100 relationships

### Accuracy

Graph quality depends on:
- **LLM model**: Claude > GPT > Gemini > Ollama
- **Knowledge quality**: Well-structured knowledge → better graph
- **Knowledge quantity**: More sessions → more concepts discovered

## Troubleshooting

### "Failed to parse graph JSON"

**Cause**: LLM returned invalid JSON (common with local models)

**Solution**: Use a better model:
```bash
claude-memory graph build myapp --provider anthropic
```

### Empty or Small Graph

**Cause**: Not enough knowledge extracted

**Solution**:
```bash
# Ensure knowledge exists
claude-memory recall myapp

# If empty, ingest first
claude-memory ingest --project myapp
claude-memory graph build myapp
```

### Graphviz Not Found

**Cause**: SVG export requires graphviz

**Solution**:
```bash
# macOS
brew install graphviz

# Linux
sudo apt install graphviz

# Or use DOT/ASCII instead
claude-memory graph viz myapp ascii
```

## Future Enhancements

Planned features:
- [ ] **Automatic graph merging** (multi-agent consolidation)
- [ ] **Temporal graph evolution** (track changes over time)
- [ ] **Subgraph extraction** (focus on specific domains)
- [ ] **Graph diff** (compare versions)
- [ ] **Interactive web UI** (D3.js force-directed)
- [ ] **Graph metrics** (centrality, clustering coefficient)
- [ ] **Community detection** (find concept clusters)
- [ ] **TUI graph mode** (navigate graph interactively)

## Comparison: Text vs. Graph Retrieval

### Text Search

```bash
claude-memory search "authentication"
# Finds: documents containing word "authentication"
```

**Pros:** Fast, simple
**Cons:** Misses related concepts with different names

### Graph Query

```bash
claude-memory graph query my-project "authentication" --depth 2
# Finds: OAuth, JWT, security, sessions, tokens...
# Even if they don't mention "authentication"!
```

**Pros:** Discovers connections, finds related concepts
**Cons:** Requires graph build step

### Best Practice

Use both:
1. **Graph query** to discover related concepts
2. **Text search** to find specific details

```bash
# 1. Find what's related
claude-memory graph query myapp "auth" --depth 2
# → Shows: OAuth, JWT, sessions

# 2. Search for details
claude-memory search "OAuth implementation" --project myapp
```

## Example Workflows

### Architecture Documentation

```bash
# Build graph
claude-memory graph build myapp

# Find central concepts
claude-memory graph hubs myapp --top 20

# Generate architecture diagram
claude-memory graph viz myapp svg -o architecture.svg

# Add to docs
cp architecture.svg docs/architecture/knowledge-graph.svg
```

### Code Review

```bash
# Reviewer: What security concepts exist?
claude-memory graph query myapp "security" --depth 2

# Shows all security-related decisions
# Helps ensure nothing is missed
```

### Onboarding

```bash
# New team member: Understand the system
claude-memory graph viz myapp ascii

# Shows key concepts and how they relate
# Better than reading linear docs!
```

## The Power of Graphs

### Before (Text-based)

```
Q: "What authentication patterns do we use?"
A: Grep through files, read linearly, manually connect concepts
```

### After (Graph-based)

```
Q: "What authentication patterns do we use?"
A: Graph query → OAuth → JWT → sessions → tokens
   All connected concepts revealed instantly!
```

This is **how your brain actually works** - associative networks, not linear search! 🧠

## Success!

You now have:
- ✅ **Associative memory** (like brain networks)
- ✅ **Spreading activation** (graph traversal)
- ✅ **Hub detection** (important concepts)
- ✅ **Path finding** (conceptual connections)
- ✅ **Visual representation** (see your knowledge)

Your memory is now **85% brain-like**! 🎉
