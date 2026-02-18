# Memory Quality & Observability Guide (v0.3.1+)

Four features that improve how engram captures, measures, and prunes knowledge.

---

## A — Observation-Enriched Ingest

During `engram ingest`, the LLM extraction prompt is now prepended with the list of files
edited in the session (sourced from today's and yesterday's observations JSONL):

```
[Files edited in this session: src/inject.rs, src/extractor/knowledge.rs]
```

**No setup needed** — activates automatically whenever the PostToolUse hook has recorded
observations for a session.

**Verify:**
```bash
ls ~/memory/observations/<project>/          # should list YYYY-MM-DD.jsonl files
engram ingest --project <project> --force    # re-run; LLM output references file names
```

---

## B — Access Tracking + Token Analytics

### New tracked events

| Command | Event |
|---------|-------|
| `engram context <project>` | `Context` |
| `engram inject --smart` | `Inject` (with `tokens_consumed`) |
| `engram ingest` | `Ingest` (with `tokens_consumed = input + output tokens`) |
| `engram search-semantic` | `SemanticSearch` |

### Enhanced analytics

```bash
engram analytics <project>
```

Now shows **Tokens ingested** and a **Command Usage Breakdown**:

```
Tokens ingested: 184230

📋 Command Usage Breakdown:
  Recall: 18
  Ingest: 12
  Search: 7
  SemanticSearch: 3
  Inject: 2
```

### Backward compatibility

Existing analytics JSONL files remain valid — `tokens_consumed` is absent in old records and
defaults to `None` via `#[serde(default)]`.

---

## C — Smart `engram forget --stale`

Prune old entries that were never given a TTL.

```bash
# Interactive: list entries older than 30 days, confirm before deleting
engram forget <project> --stale 30d

# Non-interactive: delete immediately (for scripts / CI)
engram forget <project> --stale 60d --auto

# Supported duration units: m (minutes), h (hours), d (days), w (weeks)
engram forget <project> --stale 6w --auto
```

**What gets pruned:** blocks in `decisions.md`, `solutions.md`, `patterns.md` whose
RFC 3339 timestamp is older than the threshold. `context.md` is deleted afterwards.

**Regenerate context immediately:**
```bash
engram regen <project>
```

**Combine with `--expired` for a full cleanup:**
```bash
engram forget <project> --expired         # remove TTL-expired entries first
engram forget <project> --stale 90d --auto  # then prune old no-TTL entries
engram regen <project>                    # rebuild context
```

---

## D — Observation-Augmented Smart Inject

`engram inject --smart` now reads today's observations file and adds the edited files
to the semantic search signal, even when there is no git history:

```
Context signal: project: Personal. recently observed: src/inject.rs, src/cli.rs
```

**No setup needed** — activates automatically when `~/memory/observations/<project>/YYYY-MM-DD.jsonl` exists.

---

## Combined Daily Workflow

```bash
# Work normally — hook records every Edit/Write/Bash to observations
# ...

# 1. Ingest (enriched with observed files — Feature A)
engram ingest --project Personal

# 2. Inject for the next session (uses observed files as signal — Feature D)
engram inject Personal --smart

# 3. Review analytics (Feature B)
engram analytics Personal

# 4. Monthly cleanup (Feature C)
engram forget Personal --stale 60d --auto && engram regen Personal
```
