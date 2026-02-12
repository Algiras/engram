# Memory Architecture: Brain-Inspired Design

How `claude-memory` mirrors human memory systems and where it can improve.

## Current Architecture

### 1. Multi-Layered Storage (Like Human Memory)

```
claude-memory
├── Episodic Memory (Conversations)
│   ├── Full conversation archives
│   ├── Temporal sequencing
│   └── Rich context preservation
│
├── Semantic Memory (Knowledge)
│   ├── Decisions (declarative facts)
│   ├── Solutions (problem-solving knowledge)
│   ├── Patterns (procedural knowledge)
│   └── Preferences (personal facts)
│
├── Working Memory (Context.md)
│   ├── Synthesized current understanding
│   ├── Limited size (like working memory)
│   └── Injected into active sessions
│
└── Procedural Memory (Patterns)
    ├── Code patterns
    ├── Workflows
    └── Conventions
```

### 2. Brain-Inspired Features ✅

| Human Memory System | claude-memory Equivalent | Status |
|---------------------|-------------------------|--------|
| **Episodic Memory** | Conversation archives | ✅ Implemented |
| **Semantic Memory** | Knowledge extraction (decisions, solutions) | ✅ Implemented |
| **Procedural Memory** | Patterns and workflows | ✅ Implemented |
| **Working Memory** | Context.md (limited, synthesized) | ✅ Implemented |
| **Long-term Consolidation** | LLM synthesis from sessions | ✅ Implemented |
| **Forgetting (decay)** | TTL expiration | ✅ Implemented |
| **Retrieval Cues** | Search, lookup, recall | ✅ Implemented |
| **Chunking** | Session-based organization | ✅ Implemented |

### 3. Missing Brain-Like Features ❌

| Human Memory System | What's Missing | Task |
|---------------------|----------------|------|
| **Associative Networks** | Graph connections between concepts | #5 (pending) |
| **Semantic Similarity** | Embedding-based retrieval | #2 (pending) |
| **Memory Consolidation** | Automatic merging/deduplication | #6 (pending) |
| **Spaced Repetition** | Importance-based retention | Not planned |
| **Emotional Tagging** | Sentiment/importance scores | Not planned |
| **Multi-modal** | Images, diagrams, audio | Not planned |
| **Context-Dependent Recall** | Different contexts → different memories | Partially via projects |

## Detailed Comparison

### Human Brain Memory Types

```
┌─────────────────────────────────────────────┐
│           HUMAN MEMORY SYSTEMS              │
├─────────────────────────────────────────────┤
│                                             │
│  Sensory → Working → Long-term             │
│  Memory    Memory    Memory                │
│  (ms)      (seconds) (lifetime)            │
│                                             │
│  Long-term subdivides into:                │
│  ┌──────────────┬──────────────┐          │
│  │  Explicit    │  Implicit    │          │
│  │  (conscious) │  (automatic) │          │
│  └──────────────┴──────────────┘          │
│       │                │                   │
│  ┌────┴────┐      ┌───┴────┐             │
│  │Episodic │      │Procedural│            │
│  │Semantic │      │Priming   │            │
│  └─────────┘      └──────────┘            │
└─────────────────────────────────────────────┘
```

### claude-memory Current Implementation

```
┌─────────────────────────────────────────────┐
│        CLAUDE-MEMORY ARCHITECTURE           │
├─────────────────────────────────────────────┤
│                                             │
│  JSONL → Archive → Knowledge → Context     │
│  (raw)   (episodic) (semantic) (working)   │
│                                             │
│  Storage Structure:                        │
│  ┌──────────────────────────────┐         │
│  │  conversations/               │         │
│  │  └── project/session/*.md    │ Episodic│
│  ├──────────────────────────────┤         │
│  │  knowledge/                   │         │
│  │  ├── decisions.md            │ Semantic│
│  │  ├── solutions.md            │         │
│  │  ├── patterns.md             │ Procedural
│  │  └── context.md              │ Working │
│  ├──────────────────────────────┤         │
│  │  _global/                    │         │
│  │  ├── preferences.md          │ Personal│
│  │  └── shared.md               │ Cross-project
│  └──────────────────────────────┘         │
└─────────────────────────────────────────────┘
```

## What Makes It Brain-Like

### 1. **Consolidation Process** (Like Sleep!)

```
Night/Session End → Full knowledge extraction
Day/PostToolUse   → Quick archival (debounced)
```

The brain consolidates memories during sleep. We consolidate during:
- Session end (deep extraction)
- Background hooks (quick saves)

### 2. **Hierarchical Organization**

```
Global Memory
  ├── Cross-project patterns
  └── Personal preferences
      │
Project Memory (per domain)
  ├── Specific decisions
  ├── Local solutions
  └── Project patterns
      │
Session Memory (episodic)
  └── Full conversation context
```

Similar to brain's hierarchical organization:
- Prefrontal cortex (global/executive)
- Domain-specific regions (projects)
- Hippocampus (episodic sessions)

### 3. **Temporal Dynamics**

- **Recency**: `--since` filters recent memories
- **Decay**: TTL expiration (forgetting curve)
- **Reinforcement**: Multiple sessions strengthen knowledge
- **Timestamps**: Temporal context preserved

### 4. **Retrieval Methods**

Like the brain has multiple retrieval pathways:

| Pathway | Human Brain | claude-memory |
|---------|-------------|---------------|
| **Direct recall** | "What's the capital of France?" | `recall project` |
| **Associative** | "That reminds me of..." | Search with context |
| **Semantic** | "Things related to X" | `lookup project topic` |
| **Episodic** | "Remember when we..." | Conversation archives |
| **Procedural** | "How do I...?" | Pattern lookup |

## What's Missing vs. Brain

### 1. **Associative/Graph Structure** ❌

**Human brain:**
```
Concept A ──┬── Concept B
            ├── Concept C
            └── Concept D ── Concept E
```

**What we need:** Knowledge graph showing:
- Which decisions relate to which patterns
- Cross-project concept links
- Causal relationships
- Similarity clusters

**Planned:** Task #5 - Knowledge Graph Visualization

### 2. **Semantic Similarity** ❌

**Human brain:** Automatically connects similar concepts even if different words used

**What we need:** Embedding-based search:
```bash
# Search: "database optimization"
# Should find: "SQL query performance", "index tuning", "caching strategies"
# Even if exact words don't match
```

**Planned:** Task #2 - Semantic Search with Embeddings

### 3. **Importance Weighting** ⚠️  Partial

**Human brain:** Important memories are strengthened and retained longer

**What we have:** TTL for decay
**What we need:**
- Automatic importance scoring
- Frequently accessed knowledge stays longer
- Critical decisions flagged automatically

### 4. **Context-Dependent Retrieval** ⚠️  Partial

**Human brain:** Same cue triggers different memories in different contexts

**What we have:** Project-based separation
**What we need:**
- Context switching (dev vs. docs vs. testing)
- Role-based memory (researcher vs. implementer)
- Temporal context (Q1 goals vs. Q2 goals)

### 5. **Reconsolidation** ❌

**Human brain:** Memories are updated/modified when recalled

**What we have:** Static knowledge files
**What we need:**
- Update knowledge when used in new contexts
- Merge new insights with old knowledge
- Detect contradictions and resolve

**Partially planned:** Task #6 - Smart Consolidation

## How to Make It More Brain-Like

### Phase 1: Add Graph Structure (Task #5)

```rust
// Knowledge Graph
struct KnowledgeGraph {
    nodes: Vec<Concept>,
    edges: Vec<(ConceptId, ConceptId, RelationType)>,
}

enum RelationType {
    Causes,        // "X causes Y"
    Implements,    // "X implements pattern Y"
    RelatesTo,     // "X relates to Y"
    Contradicts,   // "X contradicts Y"
    Supersedes,    // "X replaces Y"
}
```

### Phase 2: Add Embeddings (Task #2)

```rust
// Semantic Memory
struct SemanticMemory {
    embeddings: Vec<(ConceptId, Vec<f32>)>,  // Vector embeddings
    index: HNSWIndex,                         // Fast similarity search
}

// Query
memory.find_similar("authentication", top_k=10)
// Returns: ["OAuth", "JWT", "session management", ...]
```

### Phase 3: Importance Scoring

```rust
struct MemoryEntry {
    content: String,
    importance: f32,        // 0.0 - 1.0
    access_count: u32,      // How often accessed
    last_accessed: DateTime,
    reinforcement: f32,     // Increases with use
}

// Spaced repetition-style decay
fn should_retain(entry: &MemoryEntry) -> bool {
    let recency_score = calculate_recency(entry.last_accessed);
    let frequency_score = entry.access_count as f32 * 0.1;
    let importance = entry.importance;

    (recency_score + frequency_score + importance) > THRESHOLD
}
```

### Phase 4: Multi-Modal Memory

```rust
enum MemoryContent {
    Text(String),
    Image { path: PathBuf, description: String },
    Diagram { svg: String, concept_map: Graph },
    Code { language: String, content: String },
    Audio { transcript: String, recording: PathBuf },
}
```

## Proposed Enhanced Architecture

```
┌─────────────────────────────────────────────────────┐
│          BRAIN-INSPIRED MEMORY SYSTEM               │
├─────────────────────────────────────────────────────┤
│                                                     │
│  Input Layer (Sensory)                             │
│  └── JSONL conversations, user inputs              │
│                    ↓                                │
│  Working Memory (Claude's context)                 │
│  └── Current session + injected context           │
│                    ↓                                │
│  Consolidation (LLM extraction)                    │
│  └── Extract structure, importance, relationships  │
│                    ↓                                │
│  ┌──────────────────────────────────────┐         │
│  │   MULTI-FACETED LONG-TERM MEMORY     │         │
│  ├──────────────────────────────────────┤         │
│  │                                       │         │
│  │ [1] Episodic Store                   │ ✅      │
│  │     └── Conversation archives         │         │
│  │                                       │         │
│  │ [2] Semantic Network                  │ ⚠️      │
│  │     ├── Knowledge files (text)        │ ✅      │
│  │     ├── Embeddings (vectors)          │ ❌      │
│  │     └── Concept graph (relations)     │ ❌      │
│  │                                       │         │
│  │ [3] Procedural Memory                │ ✅      │
│  │     └── Patterns and workflows        │         │
│  │                                       │         │
│  │ [4] Spatial/Visual                    │ ❌      │
│  │     └── Diagrams, graphs, trees       │         │
│  │                                       │         │
│  │ [5] Temporal Index                    │ ✅      │
│  │     └── Time-based organization       │         │
│  │                                       │         │
│  │ [6] Importance Weighting             │ ⚠️      │
│  │     ├── TTL (basic decay)             │ ✅      │
│  │     ├── Access frequency              │ ❌      │
│  │     └── Automatic scoring              │ ❌      │
│  └───────────────────────────────────────┘         │
│                    ↓                                │
│  Retrieval Layer (Multiple pathways)              │
│  ├── Full-text search (regex)          ✅         │
│  ├── Fuzzy search (partial matching)   ✅         │
│  ├── Semantic search (embeddings)      ❌         │
│  ├── Graph traversal (associations)    ❌         │
│  └── Temporal queries (time-based)     ✅         │
│                    ↓                                │
│  Output Layer                                      │
│  ├── CLI (text)                        ✅         │
│  ├── TUI (interactive text)            ✅         │
│  ├── MCP (structured API)              ✅         │
│  └── Export (various formats)          ✅         │
└─────────────────────────────────────────────────────┘
```

## Human Brain Memory Systems

### What We Match Well ✅

#### 1. **Episodic Memory** (Hippocampus-like)
**Brain:** Remembers specific events - "Yesterday I learned about OAuth"
**claude-memory:** Conversation archives with full context

```bash
~/memory/conversations/myapp/session-123/
├── conversation.md  # Full episodic record
└── meta.json        # Temporal metadata
```

#### 2. **Semantic Memory** (Temporal lobe-like)
**Brain:** Facts without context - "OAuth uses tokens"
**claude-memory:** Extracted decisions/solutions/patterns

```bash
~/memory/knowledge/myapp/
├── decisions.md     # Declarative facts
├── solutions.md     # Problem-solving knowledge
└── patterns.md      # Procedural knowledge
```

#### 3. **Memory Consolidation** (Sleep-like)
**Brain:** Transfers memories from short-term to long-term during sleep
**claude-memory:** LLM extracts knowledge from sessions

```
Session End → LLM extraction → Knowledge files → Context synthesis
```

#### 4. **Forgetting Curve** (Natural decay)
**Brain:** Unused memories fade over time
**claude-memory:** TTL expiration

```bash
--ttl 7d    # Decays after 7 days (like human memory)
--ttl 30d   # Important knowledge lasts longer
```

#### 5. **Retrieval Practice Effect**
**Brain:** Recalling strengthens memories
**claude-memory:** Could track access frequency (not yet implemented)

### What We're Missing ❌

#### 1. **Associative Network** (Connections)

**Human brain:**
```
Authentication ←→ OAuth
      ↓
   Security ←→ JWT ←→ Tokens
      ↓
   Rate Limiting
```

**What we need:**
```rust
struct KnowledgeGraph {
    nodes: HashMap<ConceptId, Concept>,
    edges: Vec<Edge>,
}

struct Edge {
    from: ConceptId,
    to: ConceptId,
    relation: RelationType,
    strength: f32,  // How strong is the connection
}

// Query
graph.find_related("authentication", max_depth=2)
// → ["OAuth", "JWT", "tokens", "sessions", "security"]
```

#### 2. **Semantic Embeddings** (Meaning-based)

**Human brain:** Connects concepts by meaning, not just words

**What we need:**
```rust
struct EmbeddingStore {
    embeddings: Vec<(String, Vec<f32>)>,  // text → 384-dim vector
    index: HNSWIndex,                      // Fast nearest-neighbor
}

// Semantic search
store.find_similar("user authentication")
// → ["login system", "OAuth flow", "session management"]
// Even though different words!
```

#### 3. **Importance Scoring** (Salience)

**Human brain:** Important events remembered better

**What we need:**
```rust
struct ImportanceSignals {
    recency: f32,           // How recent
    frequency: f32,         // How often recalled
    surprise: f32,          // How unexpected
    emotional: f32,         // User explicitly flagged
    outcome_quality: f32,   // Did it work?
}

fn calculate_importance(entry: &Entry) -> f32 {
    0.3 * entry.recency
    + 0.3 * entry.frequency
    + 0.2 * entry.surprise
    + 0.2 * entry.emotional
}
```

#### 4. **Context-Dependent Memory**

**Human brain:** Same cue → different memory in different contexts

**What we need:**
```rust
enum Context {
    Development,
    Documentation,
    Debugging,
    Planning,
    Review,
}

// Same query, different results based on context
memory.recall("authentication", context=Context::Development)
// → Implementation details

memory.recall("authentication", context=Context::Documentation)
// → API documentation
```

## Proposed Improvements

### Short-term: Make Current System More Brain-Like

#### A. Add Access Tracking (Memory Reinforcement)

```rust
struct KnowledgeEntry {
    content: String,
    created_at: DateTime,
    accessed_at: DateTime,
    access_count: u32,     // New!
    importance: f32,       // New! Auto-calculated
}

// Update on access
fn recall(project: &str) {
    let knowledge = load_knowledge(project);
    knowledge.accessed_at = now();
    knowledge.access_count += 1;
    knowledge.importance = calculate_importance(&knowledge);
    save_knowledge(knowledge);
}
```

#### B. Add Relationship Tags

```markdown
## Session: abc123 (2024-01-01)

Decision: Use OAuth 2.0 for authentication

[relates-to: #security, #api-design]
[implements: #authentication-pattern]
[supersedes: #session-cookies]
```

#### C. Add Importance Flags

```bash
# High importance (never expires)
claude-memory add myapp decisions "Critical: Database sharding strategy" \
  --importance high

# Low importance (expires quickly)
claude-memory add myapp patterns "Minor: Prefer const over let" \
  --importance low --ttl 7d
```

### Long-term: Full Brain-Like Architecture

#### Phase 1: Add Knowledge Graph (Task #5)

```bash
# Build graph from existing knowledge
claude-memory graph build myapp

# Query graph
claude-memory graph query myapp "authentication" --depth 2
# Shows: authentication → OAuth → JWT → tokens → expiry

# Visualize
claude-memory graph viz myapp -o graph.svg
```

#### Phase 2: Add Semantic Embeddings (Task #2)

```bash
# Generate embeddings
claude-memory embed myapp

# Semantic search
claude-memory search myapp "secure user data" --semantic
# Finds: "encryption", "HTTPS", "password hashing"
# Even though different words!
```

#### Phase 3: Add Smart Consolidation (Task #6)

```bash
# Detect duplicates and conflicts
claude-memory consolidate myapp

# Output:
# Found 3 similar entries:
#  1. "Use Redis for caching" (session-a)
#  2. "Implement Redis cache" (session-b)
#  3. "Cache with Redis" (session-c)
#
# Suggested merge:
# "Use Redis for caching (confirmed across sessions a, b, c)"
```

## Memory Retrieval Comparison

### Current Implementation (3 modes)

```
1. Direct Query    → recall project → Returns context.md
2. Text Search     → search "query" → Regex matching
3. Topic Lookup    → lookup project topic → Substring match
```

### Brain-Like Implementation (6 modes)

```
1. Direct Recall   → recall project
2. Text Search     → search "query" --fuzzy
3. Semantic Search → search "query" --semantic
4. Graph Traversal → graph query concept --depth N
5. Temporal Query  → search --since "last week"
6. Associative     → "Similar to this session..."
```

## Information Flow: Brain vs. claude-memory

### Human Brain

```
Sensory Input → Working Memory → Consolidation → Long-term
     ↓              ↓                 ↓              ↓
  Attention    Rehearsal         Sleep          Retrieval
   Filter      (7±2 items)      (LLM-like)      (Cues)
```

### claude-memory

```
JSONL Input → Session Parse → LLM Extract → Knowledge Store
     ↓              ↓              ↓              ↓
  Filter       Archive      Categorize       Synthesize
 (events)    (markdown)   (D/S/P/Prefs)    (context.md)
     ↓              ↓              ↓              ↓
  Hooks         Git Sync      Embedding       Retrieval
(auto-run)   (multi-agent)   (semantic)    (MCP/CLI/TUI)
```

## Cognitive Functions Mapped

| Cognitive Function | Human Implementation | claude-memory Implementation |
|-------------------|---------------------|----------------------------|
| **Encoding** | Attention, semantic processing | LLM extraction, categorization |
| **Storage** | Neural networks, synapses | Markdown files, git commits |
| **Retrieval** | Activation spreading | Search, recall, graph traversal |
| **Consolidation** | Sleep, replay | Session-end hooks, LLM synthesis |
| **Forgetting** | Synaptic pruning | TTL expiration, cleanup |
| **Recognition** | Pattern matching | Fuzzy/semantic search |
| **Recall** | Cue-based retrieval | CLI commands, MCP tools |
| **Working Memory** | Prefrontal cortex | context.md, MEMORY.md |

## Conclusion

### Current State: **~60% Brain-Like** 🧠

**Strong areas:**
- ✅ Episodic memory (conversations)
- ✅ Semantic memory (knowledge extraction)
- ✅ Procedural memory (patterns)
- ✅ Temporal organization
- ✅ Forgetting curve (TTL)
- ✅ Multi-agent collaboration (git sync)

**Missing areas:**
- ❌ Associative graph structure
- ❌ Semantic similarity (embeddings)
- ❌ Importance weighting
- ❌ Reconsolidation
- ❌ Multi-modal (text only)

### To Reach **90% Brain-Like**:

1. **Add embeddings** → Semantic understanding
2. **Add knowledge graph** → Associative connections
3. **Add importance scoring** → Retention priority
4. **Add reconsolidation** → Update on recall
5. **Add context awareness** → Role/mode-based retrieval

### The Vision: True Artificial Memory

```
claude-memory (future)
├── Multi-modal storage (text, code, diagrams, audio)
├── Graph-structured (concepts connected like neurons)
├── Embedding-indexed (semantic similarity)
├── Importance-weighted (retention based on salience)
├── Context-aware (retrieves based on current mode)
├── Self-consolidating (automatically merges and updates)
├── Distributed (multi-agent collaboration)
└── Adaptive (learns what to remember)
```

This would be the **closest thing to artificial long-term memory** for AI agents! 🚀

---

**Current philosophy:** Store everything, organize well, search efficiently
**Future philosophy:** Store intelligently, connect deeply, retrieve semantically

We're already closer to the brain than most systems! 🧠
