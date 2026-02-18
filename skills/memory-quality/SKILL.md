---
name: engram:memory-quality
description: >
  Memory quality, observability, and progressive context tools for engram. Covers six features:
  (A) observation-enriched ingest — edited files passed to LLM during extraction;
  (B) access tracking + token analytics — every command records usage events with token counts;
  (C) smart stale-forget — prune old no-TTL knowledge;
  (D) observation-augmented smart inject — today's observed files added to semantic signal;
  (E) progressive MCP context — index/recall(session_ids)/timeline for ~10x token savings;
  (F) <private> tag filtering — wrap any content to exclude it from storage and injection.
  Use this skill when asked about stale knowledge, analytics, smart inject, private content, or MCP token efficiency.
license: MIT
metadata:
  author: engram
  version: "1.1.0"
  repository: https://github.com/Algiras/engram
  triggers:
    - stale knowledge
    - forget old entries
    - engram forget --stale
    - token analytics
    - tokens ingested
    - access tracking
    - command breakdown
    - observation enrichment
    - smart inject observations
    - recently observed files
    - ingest enrichment
    - engram analytics
    - memory quality
    - private tag
    - "<private>"
    - progressive context
    - mcp index
    - mcp timeline
    - token efficient recall
    - session ids recall
---

# Memory Quality & Observability (engram v0.3+)

Four features shipped in this plan that improve how engram captures, measures, and prunes knowledge.

---

## Feature E — Progressive MCP Context Retrieval

**Why:** `recall` previously returned the full synthesized context on every call — often 2,000–8,000 tokens whether needed or not. Three-stage retrieval gives the LLM surgical control.

### The 3-tier workflow

```
1. index("Personal")          → ~100 tokens — compact manifest of all entries
2. timeline("Personal", id)   → ~150 tokens — chronological context around one entry
3. recall("Personal",         → ~300 tokens/entry — full content for specific IDs
     session_ids=["abc","def"])
```

### `index` tool
Returns every active entry as a single line: `session_id (date) — "preview"`, grouped by category.

```
## Personal knowledge index (14 entries)
Use recall(session_ids=[...]) to fetch specific entries.

### decisions (3)
  reflect-2026-01-15 (2026-01-15) — "Decided to use tokio runtime for async..."
  a3b2c1d0 (2026-01-10) — "Chose PostgreSQL over SQLite for..."
  ...

### patterns (5)
  manual-di (2026-01-08) — "Pattern: DI via constructor injection..."
  ...
```

### `recall` with `session_ids`
Pass specific IDs from `index` to get only those blocks:

```json
{"project": "Personal", "session_ids": ["reflect-2026-01-15", "manual-di"]}
```

Without `session_ids`, `recall` still returns the full synthesized context (backward-compatible).

### `timeline` tool
Shows a ±N window of sessions sorted chronologically around any given session:

```
## Timeline: 'reflect-2026-01-15' (±3 sessions)

  a3b2c1d0 [decisions] (2026-01-10) — "Chose PostgreSQL..."
  manual-di [patterns] (2026-01-12) — "Pattern: DI via..."
► reflect-2026-01-15 [decisions] (2026-01-15) — "Decided to use tokio..."
  x9y8z7w6 [solutions] (2026-01-18) — "Problem: connection pool..."

Use recall(session_ids=[...]) to fetch full content.
```

---

## Feature F — `<private>` Tag Filtering

Wrap any content in `<private>...</private>` to exclude it from:
- LLM knowledge extraction during `engram ingest`
- `MEMORY.md` injection (compact, full, and smart modes)
- All MCP tool responses (`recall`, `search`, `lookup`, `index`)

### Usage

```
This is a normal message.
<private>My API key is sk-proj-... and my password is hunter2</private>
This part will be captured normally.
```

**Case-insensitive** — `<PRIVATE>`, `<Private>`, etc. all work.
**Multi-line** — the tag spans as many lines as needed.
**No configuration** — active by default everywhere.

### What is NOT filtered
- CLI `engram recall` and `engram context` commands — you always see your own full knowledge
- The raw `.md` knowledge files on disk — private content is never written there in the first place (stripped before the LLM extraction prompt)

---

## Feature A — Observation-Enriched Ingest

**What it does:** When `engram ingest` extracts knowledge from a conversation, it now looks up today's
and yesterday's `~/memory/observations/<project>/YYYY-MM-DD.jsonl` for records whose `session` field
matches the conversation being processed.  If any edited files are found, the LLM prompt is prepended with:

```
[Files edited in this session: src/inject.rs, src/extractor/knowledge.rs, ...]
```

This gives the extraction LLM a strong hint about which files to pay attention to, producing sharper
decisions, solutions, and patterns.

**No user action needed** — it activates automatically whenever observations exist for a session.
Observations are recorded by the PostToolUse hook (`engram observe`).

**Verify it worked:**
```bash
# Check observations exist for today
ls ~/memory/observations/<project>/

# Run ingest; watch the LLM output reference specific file names
engram ingest --project <project> --force
```

---

## Feature B — Access Tracking + Token Analytics

### New event types

| EventType | Recorded by |
|-----------|-------------|
| `Context` | `engram context <project>` |
| `Inject`  | `engram inject --smart` (smart mode) |
| `Ingest`  | every session processed by `engram ingest` |
| `SemanticSearch` | `engram search-semantic` (was defined, now fires) |

### New field: `tokens_consumed`

`UsageEvent` has a new optional `tokens_consumed: Option<u64>` field (backward-compatible via
`#[serde(default)]`). Ingest events record `total_input_tokens + total_output_tokens` from the
parsed JSONL conversation.

### Enhanced analytics output

```bash
engram analytics <project>
```

Now shows:
- **Tokens ingested** — sum of `tokens_consumed` across all `Ingest` events in the window
- **Command Usage Breakdown** — all event types ranked by frequency

Example output:
```
📊 Usage Insights
============================================================

Total events: 42
Unique projects: 3
Most active project: Personal
Most common action: Recall
Usage trend: increasing (+20%)
Tokens ingested: 184230

📋 Command Usage Breakdown:
  Recall: 18
  Ingest: 12
  Search: 7
  SemanticSearch: 3
  Inject: 2
```

---

## Feature C — Smart `engram forget --stale`

Prune knowledge entries that are old and were never given a TTL.

### Usage

```bash
# List stale entries older than 30 days (interactive)
engram forget <project> --stale 30d

# Auto-remove without confirmation prompt
engram forget <project> --stale 30d --auto

# Other supported durations
engram forget <project> --stale 6w    # 6 weeks
engram forget <project> --stale 2h    # 2 hours (useful for testing)
engram forget <project> --stale 90d   # 3 months
```

### Interactive example

```
$ engram forget Personal --stale 30d
Stale entries (older than 30d) in 'Personal':
  [decisions]  reflect-2025-10-15  (2025-10-15)  "Decided to use tokio runtime..."
  [solutions]  a3b2c1d0            (2025-10-12)  "Problem: race condition in..."
  [patterns]   manual              (2025-10-10)  "Pattern: DI via constructor..."

3 entries found. Remove? [y/N]
```

### Non-interactive (scripts / CI)

```bash
engram forget Personal --stale 90d --auto
# Done! Removed 3 stale entries from Personal.
```

### What happens after removal

- Matching blocks are removed from `decisions.md`, `solutions.md`, `patterns.md`
- `context.md` is deleted so the next `engram inject` or `engram recall` triggers a fresh synthesis
- Run `engram regen <project>` to immediately regenerate context

### Combining with `--expired`

```bash
# Remove TTL-expired entries first, then stale no-TTL entries
engram forget <project> --expired
engram forget <project> --stale 60d --auto
```

---

## Feature D — Observation-Augmented Smart Inject

**What it does:** `detect_work_context()` (the function that builds the semantic search signal for
`engram inject --smart`) now reads `~/memory/observations/<project>/YYYY-MM-DD.jsonl` in addition
to git state.  The unique file paths from today's observations are appended to the signal string as:

```
recently observed: src/inject.rs, src/commands/knowledge.rs, src/cli.rs
```

This means smart inject works correctly **even when there is no git history** (new repos, files not
yet committed, non-git directories).

**No configuration needed** — activates automatically when today's observation file exists.

### See it in action

```bash
# Trigger some tool use (Edit/Write/Bash) — hook records observations
# Then:
engram inject <project> --smart

# Output will include:
# Smart: Context signal: project: Personal. recently observed: src/foo.rs, src/bar.rs
```

---

## Combined Workflow (all features together)

```bash
# 1. Work normally — the PostToolUse hook records observations automatically
# (edit files, run commands — each is logged to ~/memory/observations/)

# 2. Ingest with enrichment (Feature A activates automatically)
engram ingest --project Personal

# 3. Smart inject now uses observed files as signal (Feature D activates automatically)
engram inject Personal --smart

# 4. Check what was tracked (Feature B)
engram analytics Personal

# 5. Periodically prune stale entries (Feature C)
engram forget Personal --stale 60d --auto && engram regen Personal
```

---

## Storage Layout

```
~/memory/
├── observations/
│   └── <project>/
│       ├── 2026-02-18.jsonl    ← today's tool-use observations
│       └── 2026-02-17.jsonl    ← yesterday's
├── analytics/
│   └── 2026-02-18.jsonl        ← usage events (Recall, Ingest, Inject, ...)
└── knowledge/
    └── <project>/
        ├── decisions.md
        ├── solutions.md
        ├── patterns.md
        └── context.md          ← deleted by forget --stale, regenerated on next inject
```

---

## Troubleshooting

**`--stale` says "No stale entries found" but entries exist:**
- The timestamp format in the block header must be RFC 3339 (`2025-10-15T...`).  Manually-added
  entries with date-only timestamps (`2025-10-15`) are skipped. Use `engram forget <project>` (no
  flags) to list all sessions and inspect their timestamps.

**Smart inject doesn't mention "recently observed":**
- Check the observation file exists: `ls ~/memory/observations/<project>/`
- Verify the hook is installed: `engram hooks status`
- Re-install if needed: `engram hooks install`

**Analytics shows 0 tokens ingested:**
- Only sessions with at least 1 token in the JSONL are tracked. Conversations that failed to parse
  token counts (very old format) will show `None`.

**`context.md` not regenerated after stale forget:**
```bash
engram regen <project>
# or force a full re-ingest:
engram ingest --project <project> --force
```
