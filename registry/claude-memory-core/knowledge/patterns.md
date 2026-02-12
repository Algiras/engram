# Patterns

## Session: core-knowledge-extraction (2026-02-12T00:00:00Z) [ttl:never]

### Knowledge Extraction Workflow

**Pattern:** Conversation → Archive → Extract → Synthesize → Inject

Claude-memory follows a systematic workflow for knowledge management:

1. **Ingest:** Parse JSONL conversation archives from `~/.claude/projects/`
2. **Extract:** Use LLM to identify decisions, solutions, patterns, and preferences
3. **Organize:** Store in categorized markdown files under `~/memory/knowledge/{project}/`
4. **Synthesize:** Generate `context.md` summarizing key knowledge
5. **Inject:** Optionally inject into Claude Code's project memory for automatic recall

**Key Commands:**
```bash
claude-memory ingest [--project <name>]  # Extract knowledge from conversations
claude-memory recall <project>            # View synthesized knowledge
claude-memory inject [project]            # Inject into Claude Code
```

## Session: knowledge-categorization (2026-02-12T00:00:00Z) [ttl:never]

### Knowledge Categories

**Pattern:** Four-category knowledge taxonomy

Knowledge is organized into four primary categories:

1. **Decisions:** Architectural choices, technology selections, design trade-offs
   - Example: "Use async/await for all I/O operations"
   - File: `decisions.md`

2. **Solutions:** Problem-solution pairs, debugging insights, fixes
   - Example: "When X fails, use workaround Y"
   - File: `solutions.md`

3. **Patterns:** Reusable code patterns, best practices, conventions
   - Example: "Always validate user input at API boundaries"
   - File: `patterns.md`

4. **Preferences:** Tool preferences, workflow habits, coding style
   - Example: "Prefer explicit over implicit"
   - File: `preferences.md`

**Additional Files:**
- `context.md`: LLM-synthesized summary of all knowledge
- `MEMORY.md`: Auto-injected into Claude Code sessions

## Session: session-blocks (2026-02-12T00:00:00Z) [ttl:never]

### Session Block Format

**Pattern:** Markdown sections with metadata headers

Knowledge files use a structured session block format:

```markdown
## Session: {session-id} ({timestamp}) [ttl:{duration}]

Content of the knowledge entry...

Multiple paragraphs supported.
```

**Metadata:**
- `session-id`: Unique identifier (conversation ID or label)
- `timestamp`: ISO 8601 timestamp
- `ttl`: Time-to-live (e.g., "7d", "30d", "never")

**TTL Management:**
- Expired entries are marked `[EXPIRED]` in recall
- Use `claude-memory forget --expired` to clean up
- Default TTL: 30 days (configurable with `--ttl`)

## Session: hive-mind-architecture (2026-02-12T00:00:00Z) [ttl:never]

### Hive Mind: Distributed Knowledge Sharing

**Pattern:** Git-based knowledge pack distribution (like npm/cargo for knowledge)

The Hive Mind system enables sharing knowledge across users:

**Architecture:**
```
Registries (Git repos)
    ↓
Knowledge Packs (.pack/manifest.json)
    ↓
Local Installation (~/memory/packs/installed/)
    ↓
Recall Integration (union of local + pack knowledge)
```

**Key Components:**
1. **Registry:** Git repository containing multiple packs
2. **Pack:** Structured directory with `.pack/manifest.json`, `knowledge/`, `graph/`
3. **Installer:** Manages pack installation, updates, and discovery

**Privacy Model:**
- Raw conversations NEVER leave your machine
- Only extracted, redacted knowledge can be shared
- Privacy policy in pack manifest controls what's shareable

## Session: knowledge-graph (2026-02-12T00:00:00Z) [ttl:never]

### Knowledge Graph: Semantic Relationships

**Pattern:** Concept-relationship graph for connected knowledge

Claude-memory builds a knowledge graph to capture semantic relationships:

**Graph Structure:**
```
Concepts (nodes)
  - name: "async/await"
  - importance: 0.8
  - source_sessions: [...]

Relationships (edges)
  - from: "async/await"
  - to: "error handling"
  - type: "requires"
  - strength: 0.9
```

**Operations:**
```bash
claude-memory graph build <project>     # Build graph
claude-memory graph query <concept>     # Explore concept
claude-memory graph viz <project>       # Visualize
claude-memory graph path <from> <to>    # Find connections
```

**Use Cases:**
- Discover related knowledge
- Find knowledge gaps
- Understand concept dependencies

## Session: learning-system (2026-02-12T00:00:00Z) [ttl:never]

### Reinforcement Learning: Self-Improving Memory

**Pattern:** Usage signals → Learning → Optimization

Claude-memory includes a reinforcement learning system:

**Learning Loop:**
1. **Observe:** Track recall patterns, search queries, feedback
2. **Learn:** Adjust knowledge importance, retention policies
3. **Optimize:** Prioritize high-value knowledge, prune low-value

**Commands:**
```bash
claude-memory learn dashboard          # View learning progress
claude-memory learn optimize <project> # Apply optimizations
claude-memory learn feedback --helpful # Explicit feedback
```

**Signals:**
- Recall frequency (implicit)
- Search success/failure (implicit)
- User feedback (explicit via `--helpful`/`--unhelpful`)
