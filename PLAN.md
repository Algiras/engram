# Progressive Disclosure for MEMORY.md

## Problem
MEMORY.md is 839 lines but Claude Code only loads the first 200. The most valuable content (project context, pack solutions) is never seen. Repetitive per-session preferences dominate.

## Design: 3-Tier Progressive Disclosure

### Tier 1: MEMORY.md (≤180 lines, always loaded)

```
# Project Memory (auto-injected by engram)

## User Preferences (consolidated)
- **Languages:** Rust, TypeScript, Python
- **Tools:** Cargo, Clippy, pnpm, Jest, Playwright
- **Style:** Modular design, clap for CLI, thiserror for errors
- **Workflow:** Task-based, iterative dev, read-first approach
- **Testing:** cargo test + clippy, Jest for unit, Playwright for E2E
(~20 lines — deduplicated from all session blocks)

## Project: engram
(full context.md content — ~40 lines)

## Shared Knowledge
(most recent/important blocks — ~30 lines, trimmed to budget)

## Installed Packs
- **engram-core**: patterns (6 entries), solutions (8 entries)
  → `engram recall engram` for full pack content
(~15 lines — index only)

## Retrieving More Context
For detailed knowledge beyond this summary:
- `engram lookup <project> <query>` — search specific entries
- `engram search <query>` — full-text search across all knowledge
- `engram recall <project>` — full project context + packs
(~10 lines)
```

### Tier 2: On-Demand (via CLI commands)
- Full per-session preferences (17+ blocks)
- Full pack knowledge (patterns, solutions, decisions)
- Detailed decisions, solutions, patterns files

### Tier 3: Deep Search
- `engram search-semantic <query>` for vector search
- `engram graph query <concept>` for relationships

## Implementation Plan

### 1. Add `compact_preferences()` function (main.rs)
- Parse all session blocks from preferences.md
- Extract bullet points matching `**Key:** Value` pattern
- Deduplicate by key (keep most recent value, merge unique values)
- Output consolidated ~20-line section
- No LLM needed — pure structural deduplication

### 2. Add `compact_pack_summary()` function (main.rs)
- For each installed pack, count entries per category
- Output one-line summary: pack name + entry counts
- Include retrieval command hint

### 3. Add `compact_shared()` function (main.rs)
- Take shared blocks, sort by recency
- Trim to line budget (keep most recent that fit)

### 4. Modify `cmd_inject()` with line budgets
- Define `MAX_LINES = 180`
- Section budgets: prefs=25, project=60, shared=40, packs=20, guide=15
- Assemble with budget enforcement (truncate sections that overflow)
- Add retrieval guide footer

### 5. Add `--full` flag to `inject` command
- `engram inject` → compact (default, new behavior)
- `engram inject --full` → legacy full dump

### Files to modify:
- `src/main.rs`: cmd_inject(), new helper functions
- `src/cli.rs`: add --full flag to Inject command
