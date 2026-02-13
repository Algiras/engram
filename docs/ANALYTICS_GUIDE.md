# Analytics and Feedback Learning

The engram system includes a comprehensive analytics and feedback learning system that tracks usage patterns, learns from interactions, and adapts knowledge importance scores.

## Overview

The analytics system automatically tracks every interaction with the knowledge base:
- **Recall**: When you access project context
- **Search**: When you search for information
- **Lookup**: When you query specific topics
- **Add**: When you add new knowledge
- **Promote**: When you promote inbox entries
- **Forget**: When you delete knowledge
- **Export**: When you export knowledge
- **GraphQuery**: When you query the knowledge graph
- **SemanticSearch**: When you perform semantic searches

## Usage Analytics Command

View usage insights and patterns:

```bash
# Show insights for all projects (last 30 days)
engram analytics

# Show insights for specific project
engram analytics <project>

# Show detailed event log
engram analytics --detailed

# Specify time range
engram analytics --days 7

# Clear old analytics data
engram analytics --clear-old --days 30
```

## Analytics Output

### Summary View (Default)

```
📊 Usage Insights
============================================================

Total events: 142
Unique projects: 8
Most active project: engram
Most common action: Recall
Usage trend: increasing (+23%)

🔥 Top Knowledge (by usage):
  1. authentication (12x, 94.2%)
  2. error handling (8x, 87.5%)
  3. async patterns (6x, 81.3%)
  4. testing (5x, 75.0%)
  5. deployment (3x, 62.1%)

🕸️  Stale Knowledge (rarely accessed):
  1. old-api (last: 5.2%)
  2. deprecated-flow (last: 3.8%)
```

### Detailed View

```bash
engram analytics --detailed
```

Shows the last 50 events with:
- Event type icon
- Event type name
- Project name
- Timestamp
- Query (if applicable)
- Results count

## How It Works

### 1. Automatic Tracking

Every command automatically tracks its usage:

```rust
let tracker = analytics::EventTracker::new(&memory_dir);
tracker.track(UsageEvent {
    timestamp: Utc::now(),
    event_type: EventType::Recall,
    project: "my-project".to_string(),
    query: None,
    category: None,
    results_count: None,
    session_id: None,
});
```

### 2. Storage

Events are stored in daily JSONL files:
- Location: `~/memory/analytics/YYYY-MM-DD.jsonl`
- Format: One JSON object per line
- Retention: Configurable via `--clear-old`

### 3. Metrics Computation

The system computes knowledge scores based on:

**Frequency Score** (60% weight)
- How often knowledge is accessed
- Normalized against maximum access count

**Recency Score** (40% weight)
- How recently knowledge was accessed
- Exponential decay over 30 days: `exp(-days_ago / 30)`

**Importance Score** (Combined)
- `importance = 0.6 * frequency + 0.4 * recency`
- Range: 0.0 to 1.0

### 4. Insights Generation

The analytics engine generates:
- Total event count
- Unique project count
- Most active project
- Most common event type
- Usage trend (increasing/decreasing/stable)
- Top knowledge (by importance score)
- Stale knowledge (low recency score)

## Use Cases

### 1. Identify Popular Knowledge

Find which knowledge is most frequently accessed:

```bash
engram analytics my-project
```

Look at the "Top Knowledge" section to see what's most valuable.

### 2. Find Stale Knowledge

Identify knowledge that's rarely used and may be outdated:

```bash
engram analytics my-project
```

Check the "Stale Knowledge" section for cleanup candidates.

### 3. Track Usage Trends

Monitor how usage changes over time:

```bash
# Last week
engram analytics --days 7

# Last month
engram analytics --days 30

# Last quarter
engram analytics --days 90
```

### 4. Audit Project Activity

See detailed logs of all interactions:

```bash
engram analytics my-project --detailed
```

### 5. Clean Up Old Data

Remove analytics older than 30 days:

```bash
engram analytics --clear-old --days 30
```

## Integration with Other Features

### Knowledge Graph

Analytics can inform graph importance:
- Frequently accessed concepts get higher weight
- Stale concepts can be pruned or demoted

### Smart Consolidation

Analytics help identify:
- Duplicate knowledge patterns
- Redundant entries
- Merge candidates

### Doctor Command

Health checks consider analytics:
- Warns if no recent usage
- Identifies unused knowledge
- Suggests cleanup actions

## Privacy and Data

### What's Tracked

- Command type (recall, search, etc.)
- Project name
- Query text (for searches)
- Result count
- Timestamp

### What's NOT Tracked

- Actual knowledge content
- File paths
- User identity
- Network data
- Sensitive information

### Data Location

All analytics data is stored locally:
- Path: `~/memory/analytics/`
- No cloud sync
- No external transmission
- User-controlled retention

## Future Enhancements

The feedback learning system can be extended with:

1. **Adaptive Knowledge Ranking**
   - Auto-promote frequently accessed knowledge
   - Auto-demote rarely accessed knowledge
   - Smart TTL adjustment based on usage

2. **Query Understanding**
   - Learn from search queries
   - Improve search results based on click-through
   - Suggest related queries

3. **Personalization**
   - Per-user preferences
   - Project-specific patterns
   - Context-aware suggestions

4. **Reinforcement Learning**
   - Track which results are useful
   - Adapt importance scores
   - Optimize knowledge organization

## Examples

### Example 1: Identify Hot Topics

```bash
$ engram analytics my-app --days 7

📊 Usage Insights
============================================================

🔥 Top Knowledge (by usage):
  1. authentication (42x, 98.5%)
  2. database migrations (28x, 94.2%)
  3. deployment scripts (15x, 87.3%)
```

**Insight**: Team is heavily focused on auth and database work this week.

### Example 2: Find Cleanup Candidates

```bash
$ engram analytics my-app

🕸️  Stale Knowledge (rarely accessed):
  1. old-payment-flow (last: 2.1%)
  2. deprecated-api-v1 (last: 1.8%)
  3. legacy-cron-jobs (last: 0.5%)
```

**Action**: Review these entries for archival or deletion.

### Example 3: Monitor Growth

```bash
$ engram analytics my-app --days 1 --detailed

📋 Detailed Event Log
============================================================

  1. 🔎 Search my-app - 2026-02-12 17:15:23
      Query: rate limiting
      Results: 5
  2. 🔍 Recall my-app - 2026-02-12 17:12:45
  3. ➕ Add my-app - 2026-02-12 16:58:12
      Query: new caching strategy
```

**Insight**: Active knowledge creation and retrieval happening today.

## Best Practices

1. **Regular Review**: Check analytics weekly to understand usage patterns
2. **Clean Up Stale Data**: Use `--clear-old` monthly to manage storage
3. **Track Trends**: Compare different time ranges to see evolution
4. **Combine with Doctor**: Use both for comprehensive health monitoring
5. **Export Before Cleanup**: Use `engram export` before removing stale knowledge

## Technical Details

### Event Schema

```typescript
{
  timestamp: DateTime<Utc>,
  event_type: "Recall" | "Search" | "Lookup" | ...,
  project: string,
  query?: string,
  category?: string,
  results_count?: number,
  session_id?: string
}
```

### Storage Format

- **File**: `~/memory/analytics/YYYY-MM-DD.jsonl`
- **Format**: Newline-delimited JSON
- **Encoding**: UTF-8
- **Atomicity**: Append-only writes

### Performance

- **Tracking**: < 1ms overhead per command
- **Querying**: < 100ms for 30 days of data
- **Storage**: ~100 bytes per event
- **Retention**: Configurable, default unlimited

## Troubleshooting

### No Analytics Data

```bash
$ engram analytics

📊 No usage data found
```

**Solution**: Start using commands like `recall`, `search`, `add` to generate data.

### Old Data Not Clearing

```bash
$ engram analytics --clear-old --days 30
```

Check that you have write permissions to `~/memory/analytics/`.

### Insights Look Wrong

Verify the data:

```bash
$ cat ~/memory/analytics/2026-02-12.jsonl
```

Ensure events are valid JSON and have correct timestamps.

## Summary

The analytics and feedback learning system provides:
- ✅ Automatic usage tracking
- ✅ Knowledge importance scoring
- ✅ Usage trend analysis
- ✅ Stale knowledge detection
- ✅ Detailed event logging
- ✅ Privacy-preserving local storage
- ✅ Integration with other features

Use it to understand your memory usage, optimize knowledge organization, and improve retrieval effectiveness.
