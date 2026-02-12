# Learning System Architecture

## Overview

This document describes the technical architecture of claude-memory's reinforcement learning system. The system implements three complementary learning algorithms (TD learning, Q-learning, and multi-armed bandit) to continuously optimize knowledge management parameters.

## Module Structure

```
src/learning/
├── mod.rs              # Module exports and orchestration
├── signals.rs          # Success signal extraction (~200 lines)
├── algorithms.rs       # Learning algorithms (~400 lines)
├── adaptation.rs       # Parameter adjustment logic (~200 lines)
├── progress.rs         # Learning state tracking (~300 lines)
├── dashboard.rs        # Reporting and visualization (~250 lines)
└── hooks.rs            # Integration hooks (~150 lines)
```

## Core Components

### 1. Signal Layer (`signals.rs`)

**Purpose**: Extract success signals from system usage

**Signal Types**:

```rust
pub enum LearningSignal {
    HealthImprovement { before: u8, after: u8, knowledge_ids: Vec<String> },
    SuccessfulRecall { knowledge_id: String, relevance: f32 },
    ConsolidationAccepted { merged_count: usize, similarity_threshold: f32 },
    HighFrequencyAccess { knowledge_id: String, access_count: usize, recency_score: f32 },
    CoOccurrence { knowledge_ids: Vec<String>, co_access_count: usize },
}
```

**Reward Normalization**:

Each signal type maps to a 0.0-1.0 reward value:

- `HealthImprovement`: `(after - before) / 100.0`
- `SuccessfulRecall`: `relevance` (as provided)
- `ConsolidationAccepted`: `(merge_count / 10) * 0.5 + threshold * 0.5`
- `HighFrequencyAccess`: `(count / 50) * 0.6 + recency * 0.4`
- `CoOccurrence`: `count / 20`

**Signal Extraction Pipeline**:

```rust
analytics::get_events()
  → extract_signals_from_events()
  → signal.to_reward()
  → learning algorithms
```

### 2. Algorithm Layer (`algorithms.rs`)

#### Temporal Difference Learning

**Purpose**: Learn knowledge importance from usage patterns

**Algorithm**:

```rust
pub fn learn_importance(current: f32, reward: f32, learning_rate: f32) -> f32 {
    (current + learning_rate * (reward - current))
        .max(0.1)  // Lower bound
        .min(1.0)  // Upper bound
}
```

**Parameters**:
- `learning_rate`: 0.1-0.3 (default: 0.2)
- `current`: Previous importance score
- `reward`: Normalized signal (0.0-1.0)

**Properties**:
- Converges to expected reward value
- Bounded output prevents extreme adjustments
- Exponentially weighted moving average

#### Q-Learning for TTL

**Purpose**: Learn optimal TTL policies for different knowledge states

**State Space**:

```rust
pub struct TTLState {
    pub importance_tier: ImportanceTier,     // Low/Medium/High
    pub usage_frequency_tier: FrequencyTier, // Rare/Occasional/Frequent
    pub recency_tier: RecencyTier,           // Stale/Normal/Recent
}
```

**Action Space**:

```rust
pub enum TTLAction {
    Extend7d, Extend14d, Extend30d, Extend90d, MakePermanent,
    Reduce1d, Reduce3d, Reduce7d,
}
```

**Update Rule**:

```rust
Q(s,a) ← Q(s,a) + α[r + γ·max Q(s',a') - Q(s,a)]

Where:
  α = learning_rate (0.1)
  γ = discount_factor (0.9)
  r = reward (usage after adjustment - staleness penalty - storage cost)
```

**Exploration Strategy**:

Epsilon-greedy with ε=0.2:
- 20% of time: random action (exploration)
- 80% of time: best known action (exploitation)

#### Multi-Armed Bandit for Consolidation

**Purpose**: Select optimal consolidation strategy

**Arms (Strategies)**:

```rust
pub struct ConsolidationStrategy {
    pub similarity_threshold: f32,      // 0.85, 0.90, 0.95
    pub trigger_frequency_days: u32,    // 7, 14, 30
    pub size_trigger_mb: f32,           // 5.0, 10.0, 15.0
}
```

**Reward Calculation**:

```rust
reward = 0.4 × health_improvement
       + 0.3 × query_perf_improvement
       + 0.3 × user_acceptance_rate
```

**Selection Strategy**:

Epsilon-greedy with ε=0.2:
- Select arm with highest average reward
- Occasionally try random arms for exploration

### 3. Adaptation Layer (`adaptation.rs`)

**Purpose**: Apply learned parameters to the system

**Operations**:

1. **Importance Boosts**:
   ```rust
   score.importance = (score.importance + boost).max(0.1).min(1.0)
   ```

2. **TTL Adjustments**:
   ```rust
   // Update knowledge file metadata with new TTL
   update_knowledge_ttl(knowledge_id, new_ttl_days)
   ```

3. **Graph Weight Updates**:
   ```rust
   concept.importance = (concept.importance + boost).max(0.1).min(1.0)
   ```

4. **Consolidation Config**:
   ```rust
   // Write strategy to config file
   ~/memory/config/<project>_consolidation.json
   ```

**Safety Bounds**:

All parameters have hard limits:
- Importance: [0.1, 1.0]
- TTL: [1 day, ∞]
- Learning rate: [0.05, 0.5]

### 4. Progress Layer (`progress.rs`)

**Purpose**: Track learning state and history

**State Structure**:

```rust
pub struct LearningState {
    pub project: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    pub learned_parameters: LearnedParameters,
    pub ttl_q_learning: TTLQLearning,
    pub consolidation_bandit: ConsolidationBandit,
    pub hyperparameters: Hyperparameters,

    pub metrics_history: Vec<MetricsSnapshot>,
    pub adaptation_history: Vec<AdaptationRecord>,
}
```

**Persistence**:

```
~/memory/learning/<project>.json
```

**Convergence Detection**:

```rust
pub fn has_converged(&self) -> bool {
    // Check if last 10 health scores have low variance
    variance < 10.0
}
```

**Success Rate Calculation**:

```rust
successful_adaptations / total_adaptations
```

### 5. Dashboard Layer (`dashboard.rs`)

**Purpose**: Format and display learning progress

**Displays**:
- Current metrics vs targets
- Improvement trends over time
- Adaptation success rate
- Top importance boosts
- Suggested interventions

**Color Coding**:
- Green: ✓ Target met
- Yellow: ⚠ Warning
- Red: ✗ Critical

### 6. Hooks Layer (`hooks.rs`)

**Purpose**: Integrate learning with existing commands

**Integration Points**:

```rust
// After ingest
post_ingest_hook(config, project) → extract signals → update state

// After recall
post_recall_hook(config, project, accessed_knowledge) → boost importance

// After consolidate
post_consolidate_hook(config, project, merge_count, confirmed) → update bandit

// After doctor --fix
post_doctor_fix_hook(config, project, health_before, health_after) → track improvement
```

## Data Flow

```
User Command
    ↓
Analytics Tracking (existing)
    ↓
Event Log (JSONL)
    ↓
Signal Extraction (post-command hooks)
    ↓
Learning Algorithms
    ↓
Learning State Update
    ↓
Persistence (JSON)
    ↓
(Later) learn optimize
    ↓
Adaptation Layer
    ↓
System Parameters Updated
    ↓
Improved Performance
```

## Storage Schema

### Learning State File

```json
{
  "project": "claude-memory",
  "created_at": "2026-02-12T10:00:00Z",
  "updated_at": "2026-02-12T14:30:00Z",

  "learned_parameters": {
    "importance_boosts": {
      "patterns:authentication": 0.15,
      "decisions:architecture": 0.22
    },
    "ttl_adjustments": {
      "patterns:authentication": 30,  // days
      "decisions:architecture": null  // permanent
    },
    "consolidation_strategy": {
      "similarity_threshold": 0.90,
      "trigger_frequency_days": 14,
      "size_trigger_mb": 10.0
    }
  },

  "ttl_q_learning": {
    "q_table": {
      "[High,Frequent,Recent]": {
        "Extend30d": 0.85,
        "MakePermanent": 0.92
      }
    },
    "learning_rate": 0.1,
    "discount_factor": 0.9,
    "epsilon": 0.2
  },

  "consolidation_bandit": {
    "arms": [...],
    "rewards": [[0.8, 0.7], [0.9, 0.85], ...],
    "epsilon": 0.2
  },

  "hyperparameters": {
    "importance_learning_rate": 0.2,
    "ttl_learning_rate": 0.1,
    "ttl_discount_factor": 0.9,
    "exploration_rate": 0.2
  },

  "metrics_history": [
    {
      "timestamp": "2026-02-12T10:00:00Z",
      "health_score": 85,
      "avg_query_time_ms": 120,
      "stale_knowledge_pct": 15.0,
      "storage_size_mb": 10.0
    }
  ],

  "adaptation_history": [
    {
      "timestamp": "2026-02-12T14:00:00Z",
      "importance_adjustments": 5,
      "ttl_adjustments": 3,
      "graph_adjustments": 2,
      "health_before": 85,
      "health_after": 90,
      "health_improvement": 5
    }
  ]
}
```

## Performance Considerations

### Time Complexity

- **Signal Extraction**: O(n) where n = number of events
- **TD Learning Update**: O(1) per knowledge item
- **Q-Learning Update**: O(1) per state-action pair
- **Bandit Update**: O(k) where k = number of arms (4)
- **Adaptation Application**: O(m) where m = number of adjustments

### Space Complexity

- **Learning State**: ~100 KB per project
- **Q-Table**: Grows with unique states (~1-10 KB)
- **Metrics History**: ~1 KB per snapshot
- **Adaptation History**: ~500 bytes per record

### Optimization Strategies

1. **Lazy Loading**: Load learning state only when needed
2. **Incremental Updates**: Update state file only on changes
3. **Batch Processing**: Process multiple signals before persistence
4. **Sampling**: Sample from large Q-tables for faster lookups

## Testing Strategy

### Unit Tests

Each module has embedded tests:

- `signals.rs`: Signal extraction and reward calculation
- `algorithms.rs`: TD learning, Q-learning, bandit logic
- `adaptation.rs`: Parameter application and clamping
- `progress.rs`: State management and convergence
- `hooks.rs`: Hook integration

### Integration Tests

End-to-end scenarios:

```rust
#[test]
fn test_full_learning_cycle() {
    // 1. Create project and ingest sessions
    // 2. Generate usage events (recall 50x)
    // 3. Extract signals
    // 4. Apply learning algorithms
    // 5. Verify improvements
}
```

### Manual Testing

```bash
# Create test project
claude-memory ingest --project test-learning

# Generate usage
for i in {1..50}; do
  claude-memory recall test-learning > /dev/null
done

# Check learning
claude-memory learn dashboard test-learning

# Apply optimizations
claude-memory learn optimize test-learning --dry-run
claude-memory learn optimize test-learning --auto

# Verify improvements
claude-memory doctor test-learning
```

## Future Enhancements

### Planned Features

1. **Neural Network Importance Prediction**
   - Replace linear TD learning with MLP
   - Input: knowledge features (category, age, access patterns)
   - Output: Predicted importance score

2. **Transfer Learning Across Projects**
   - Share learned policies between related projects
   - Meta-learning for hyperparameter optimization
   - Cross-project knowledge graph embeddings

3. **Active Learning**
   - System suggests knowledge to review
   - User feedback strengthens learning
   - Reduces manual curation effort

4. **Automated Actions**
   - Auto-consolidate at learned optimal thresholds
   - Auto-archive stale knowledge
   - Auto-promote high-value patterns

5. **Explainable AI**
   - Show why specific adjustments were made
   - Visualize learning progress over time
   - Generate natural language explanations

### Research Directions

1. **Multi-Objective Optimization**
   - Balance health score, query speed, storage
   - Pareto-optimal parameter selection

2. **Contextual Bandits**
   - Context-aware consolidation strategies
   - Per-category learning policies

3. **Model-Based RL**
   - Learn dynamics model of knowledge evolution
   - Predictive planning for future optimizations

## Contributing

When extending the learning system:

1. **Follow existing patterns** - Match code style and structure
2. **Add tests** - Cover new signal types and algorithms
3. **Update docs** - Keep both guides in sync
4. **Benchmark performance** - Ensure O(1) or O(n) operations
5. **Preserve safety bounds** - All parameters must have limits

## See Also

- [LEARNING_GUIDE.md](LEARNING_GUIDE.md) - User-facing documentation
- [ANALYTICS_GUIDE.md](ANALYTICS_GUIDE.md) - Analytics system details
- [DOGFOODING_INSIGHTS.md](DOGFOODING_INSIGHTS.md) - Real-world results
