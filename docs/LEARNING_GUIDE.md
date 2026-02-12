# Reinforcement Learning Guide

## Overview

Claude-memory includes a built-in reinforcement learning system that automatically optimizes itself based on usage patterns. The system learns which knowledge is most valuable, adjusts time-to-live (TTL) settings, and fine-tunes consolidation strategies - all without manual intervention.

## How It Works

The learning system operates through three main mechanisms:

### 1. **Temporal Difference (TD) Learning** for Knowledge Importance

As you use `claude-memory recall`, `search`, and `lookup`, the system tracks which knowledge gets accessed most frequently. It uses TD learning to continuously update importance scores:

```
importance(new) = importance(current) + α × [reward - importance(current)]
```

- **Frequently accessed** knowledge gets higher importance
- **Recently used** knowledge is prioritized
- **Co-occurring** knowledge (accessed together) strengthens connections

### 2. **Q-Learning** for TTL Optimization

The system learns optimal time-to-live (TTL) settings for different types of knowledge:

- **High-importance, frequently accessed** → Extended TTLs or permanent
- **Low-importance, rarely accessed** → Shorter TTLs for cleanup
- **Context-dependent** → TTL adjusted based on usage patterns

### 3. **Multi-Armed Bandit** for Consolidation Strategy

Different consolidation strategies (similarity thresholds, trigger frequencies) are tested and the best-performing approach is automatically selected:

- **Similarity threshold**: 0.85, 0.90, 0.95
- **Trigger frequency**: Daily, weekly, monthly
- **Size-based triggers**: >5MB, >10MB

## Commands

### View Learning Dashboard

See the current learning state, metrics, and top improvements:

```bash
# For a specific project
claude-memory learn dashboard claude-memory

# For all projects
claude-memory learn dashboard
```

**Dashboard shows:**
- Current health score and metrics
- Improvements since learning started
- Adaptation success rate
- Top importance boosts
- Convergence status

### Apply Learned Optimizations

Preview or apply the learned parameter adjustments:

```bash
# Dry run - see what would change
claude-memory learn optimize myproject --dry-run

# Apply with confirmation
claude-memory learn optimize myproject

# Apply automatically without confirmation
claude-memory learn optimize myproject --auto
```

**Optimizations applied:**
- Importance boosts for frequently accessed knowledge
- TTL adjustments based on usage patterns
- Updated consolidation strategy
- Graph importance weights

### Reset Learning State

Reset learned parameters while preserving history:

```bash
claude-memory learn reset myproject
```

This is useful if:
- Learning has diverged or converged to a suboptimal state
- You want to restart learning from defaults
- Usage patterns have drastically changed

## Learning Lifecycle

### 1. **Data Collection Phase** (Sessions 1-20)

- System tracks all usage events (recall, search, add, etc.)
- Builds initial analytics on knowledge access patterns
- No optimizations applied yet - still learning baseline

### 2. **Active Learning Phase** (Sessions 20-100)

- Learning algorithms actively adjust parameters
- Exploration (trying new strategies) balanced with exploitation (using best known)
- Metrics tracked: health score, query performance, storage efficiency
- Adaptation success rate monitored

### 3. **Convergence Phase** (Sessions 100+)

- Parameters stabilize around optimal values
- Exploration rate reduced (more exploitation)
- System maintains health score >90 automatically
- Continuous fine-tuning for changing patterns

## Interpreting the Dashboard

### Health Score Trend

```
Health Score: 85 → 93 (+8) ✓
```

- **Target**: >90
- **Trend**: Positive (+8) shows learning is working
- **Action**: None needed if trending up

### Query Performance

```
Avg Query Time: 120ms → 95ms (-21%) ✓
```

- **Target**: <100ms
- **Trend**: Negative (faster) is good
- **Action**: None needed if improving

### Stale Knowledge

```
Stale Knowledge: 15% → 12% (-3%) ✓
```

- **Target**: <10%
- **Trend**: Decreasing is good
- **Action**: If >15%, run `consolidate` or `forget --expired`

### Adaptation Success Rate

```
Success Rate: 42/60 (70%) ✓
```

- **Target**: >70%
- **Trend**: >70% shows good learning
- **Action**: If <50%, consider `learn reset`

## Best Practices

### 1. **Let It Learn Naturally**

- Don't force optimizations early (<20 sessions)
- Let the system collect enough data
- Natural usage patterns produce best results

### 2. **Regular Check-ins**

- Run `learn dashboard` weekly
- Monitor convergence status
- Watch for adaptation success rate

### 3. **Dry Run First**

- Always use `--dry-run` before applying optimizations
- Review proposed changes
- Understand why each change is suggested

### 4. **Incremental Adjustments**

- Apply optimizations gradually
- Monitor health score after each application
- Rollback (`learn reset`) if score decreases >10 points

### 5. **Project-Specific Learning**

- Each project learns independently
- Different projects may need different strategies
- Don't expect uniform learning across all projects

## Troubleshooting

### Learning Not Converging (>100 sessions)

**Symptoms:**
- Dashboard shows "Learning..." status after 100+ sessions
- High parameter variance

**Solutions:**
1. Check if usage patterns are consistent
2. Reduce learning rate (requires code change)
3. Reset and restart: `learn reset myproject`

### Low Adaptation Success Rate (<50%)

**Symptoms:**
- Many applied optimizations don't improve metrics
- Success rate in dashboard <50%

**Solutions:**
1. Review proposed changes in `--dry-run`
2. Check if manual changes conflict with learning
3. Consider project restructuring if knowledge is too chaotic

### Health Score Decreasing After Optimization

**Symptoms:**
- Health score drops after `learn optimize`
- Metrics worse than before

**Solutions:**
1. Immediately run `learn reset myproject`
2. Check recent manual changes (may conflict)
3. Run `doctor --fix` to restore health
4. Report issue if problem persists

## Advanced Usage

### Manual Tuning

You can manually adjust hyperparameters by editing:

```bash
~/memory/learning/<project>.json
```

**Key parameters:**
- `importance_learning_rate`: 0.1-0.3 (default: 0.2)
- `ttl_learning_rate`: 0.05-0.2 (default: 0.1)
- `exploration_rate`: 0.1-0.3 (default: 0.2)

**After manual edits:**
```bash
# Restart learning with new parameters
claude-memory learn reset myproject
```

### Integrating with Workflows

Learning automatically triggers during:

- **After ingest**: Signals extracted from new sessions
- **After recall**: Importance boosted for accessed knowledge
- **After consolidate**: Strategy rewards updated
- **After doctor --fix**: Health improvements tracked

No manual intervention needed - it just works!

## FAQ

**Q: How much data does learning need?**
A: Minimum 20 sessions for basic patterns, 100+ for convergence.

**Q: Does learning affect performance?**
A: Minimal impact - learning happens asynchronously after commands complete.

**Q: Can I disable learning?**
A: Yes, simply don't run `learn optimize`. Learning tracks passively but doesn't apply changes unless you explicitly optimize.

**Q: What happens to my manual knowledge entries?**
A: They're tracked separately - learning won't override your explicit additions.

**Q: Can learning delete knowledge?**
A: No - it only adjusts importance scores and TTLs. Actual deletion requires explicit `forget` commands.

**Q: How do I transfer learning across projects?**
A: Currently, each project learns independently. Future versions may support transfer learning.

## See Also

- [LEARNING_ARCHITECTURE.md](LEARNING_ARCHITECTURE.md) - Technical implementation details
- [ANALYTICS_GUIDE.md](ANALYTICS_GUIDE.md) - Understanding usage analytics
