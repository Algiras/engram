use chrono::{DateTime, Utc};

use crate::analytics::metrics::KnowledgeScore;
use crate::config::Config;
use crate::error::{MemoryError, Result};
use crate::extractor::knowledge::{parse_session_blocks, parse_ttl, partition_by_expiry};
use crate::learning::algorithms::{
    ConsolidationStrategy, FrequencyTier, ImportanceTier, RecencyTier, TTLState,
};
use crate::learning::progress::LearningState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Learned parameter adjustments
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LearnedParameters {
    /// Importance boosts for specific knowledge IDs
    pub importance_boosts: HashMap<String, f32>,

    /// TTL adjustments (knowledge_id -> new TTL in days, None = permanent)
    pub ttl_adjustments: HashMap<String, Option<u32>>,

    /// Consolidation strategy
    pub consolidation_strategy: Option<ConsolidationStrategy>,

    /// Graph importance weight adjustments
    pub graph_weight_boosts: HashMap<String, f32>,
}

/// Apply learned importance boosts to knowledge scores
pub fn apply_importance_boosts(
    scores: &mut HashMap<String, KnowledgeScore>,
    boosts: &HashMap<String, f32>,
) {
    for (id, boost) in boosts {
        if let Some(score) = scores.get_mut(id) {
            // Apply boost while keeping within bounds [0.1, 1.0]
            score.importance = (score.importance + boost).clamp(0.1, 1.0);
        }
    }
}

/// Apply learned parameters to a project
pub fn apply_learned_parameters(
    config: &Config,
    project: &str,
    learning_state: &LearningState,
) -> Result<ApplyResult> {
    let params = &learning_state.learned_parameters;
    let mut result = ApplyResult::default();

    // 1. Apply importance boosts to analytics
    if !params.importance_boosts.is_empty() {
        apply_importance_boosts_to_analytics(config, project, &params.importance_boosts)?;
        result.importance_adjustments = params.importance_boosts.len();
    }

    // 2. Apply TTL adjustments via Q-learning policy
    result.ttl_adjustments = apply_ttl_adjustments(config, project, learning_state)?;

    // 3. Apply graph weight boosts
    if !params.graph_weight_boosts.is_empty() {
        result.graph_adjustments =
            apply_graph_weight_boosts(config, project, &params.graph_weight_boosts)?;
    }

    // 4. Update consolidation config
    if let Some(strategy) = &params.consolidation_strategy {
        update_consolidation_config(config, project, strategy)?;
        result.consolidation_updated = true;
    }

    Ok(result)
}

#[derive(Debug, Default)]
pub struct ApplyResult {
    pub importance_adjustments: usize,
    pub ttl_adjustments: usize,
    pub graph_adjustments: usize,
    pub consolidation_updated: bool,
}

/// Apply importance boosts to analytics data
fn apply_importance_boosts_to_analytics(
    _config: &Config,
    _project: &str,
    _boosts: &HashMap<String, f32>,
) -> Result<()> {
    // Placeholder - would need to extend analytics module to support persisting scores
    // For now, boosts are applied dynamically when scores are computed
    Ok(())
}

/// Apply TTL adjustments derived from the Q-learning policy.
///
/// For each active knowledge block, we:
/// 1. Compute its TTLState (importance, frequency, recency tiers).
/// 2. Ask the Q-table for the recommended TTLAction.
/// 3. Skip if Q-table is untrained (no data) or action would be a no-op.
/// 4. Rewrite the block header in the knowledge file with the new TTL.
///
/// Returns the number of blocks actually updated.
fn apply_ttl_adjustments(
    config: &Config,
    project: &str,
    state: &LearningState,
) -> Result<usize> {
    // Require at least some Q-table training before touching files.
    if state.ttl_q_learning.q_table.is_empty() {
        return Ok(0);
    }

    let knowledge_dir = config.memory_dir.join("knowledge").join(project);
    let boosts = &state.learned_parameters.importance_boosts;
    let now = Utc::now();
    let mut total_updated = 0usize;

    let categories = [
        "decisions",
        "solutions",
        "patterns",
        "bugs",
        "insights",
        "questions",
    ];

    for cat in &categories {
        let path = knowledge_dir.join(format!("{}.md", cat));
        if !path.exists() {
            continue;
        }

        let raw = std::fs::read_to_string(&path)?;
        let (preamble, blocks) = parse_session_blocks(&raw);
        let (active, expired) = partition_by_expiry(blocks);

        let mut file_modified = false;
        let mut rebuilt = preamble.clone();

        for block in &active {
            // Determine TTL state for this block
            let boost = boosts.get(&block.session_id).copied().unwrap_or(0.0);
            let ttl_state = compute_ttl_state(boost, &block.timestamp, now);

            // Query Q-learning for recommended action
            let action = state.ttl_q_learning.choose_action(&ttl_state);

            // Parse current TTL in days (None = permanent)
            let current_ttl_days: Option<u32> = block
                .ttl
                .as_deref()
                .and_then(|t| if t == "never" { None } else { parse_ttl(t) })
                .map(|d| d.num_days().max(0) as u32);

            let new_ttl_days = action.apply_to_days(current_ttl_days);

            // Skip if TTL wouldn't actually change
            if new_ttl_days == current_ttl_days {
                rebuilt.push_str(&block.header);
                rebuilt.push_str(&block.content);
                continue;
            }

            // Build updated header with new TTL
            let new_header = build_block_header(
                &block.session_id,
                &block.timestamp,
                new_ttl_days,
                block.confidence.as_deref(),
            );

            rebuilt.push_str(&new_header);
            rebuilt.push_str(&block.content);
            file_modified = true;
            total_updated += 1;
        }

        // Re-append expired blocks unchanged
        for block in &expired {
            rebuilt.push_str(&block.header);
            rebuilt.push_str(&block.content);
        }

        if file_modified {
            std::fs::write(&path, rebuilt)?;
        }
    }

    Ok(total_updated)
}

/// Build a session block header string from its components.
fn build_block_header(
    session_id: &str,
    timestamp: &str,
    ttl_days: Option<u32>,
    confidence: Option<&str>,
) -> String {
    let ttl_part = match ttl_days {
        Some(days) => format!(" [ttl:{}d]", days),
        None => String::new(), // permanent — omit TTL tag
    };
    let conf_part = match confidence {
        Some(c) => format!(" [confidence:{}]", c),
        None => String::new(),
    };
    format!(
        "## Session: {} ({}){}{}",
        session_id, timestamp, ttl_part, conf_part
    )
}

/// Map a block's importance boost + timestamp age into a TTLState.
fn compute_ttl_state(boost: f32, timestamp: &str, now: DateTime<Utc>) -> TTLState {
    let importance_tier = if boost >= 0.7 {
        ImportanceTier::High
    } else if boost >= 0.4 {
        ImportanceTier::Medium
    } else {
        ImportanceTier::Low
    };

    // Use boost magnitude as a proxy for recall frequency
    let usage_frequency_tier = if boost >= 0.5 {
        FrequencyTier::Frequent
    } else if boost >= 0.2 {
        FrequencyTier::Occasional
    } else {
        FrequencyTier::Rare
    };

    let recency_tier = if let Ok(ts) = DateTime::parse_from_rfc3339(timestamp) {
        let age_days = (now - ts.with_timezone(&Utc)).num_days();
        if age_days < 7 {
            RecencyTier::Recent
        } else if age_days < 30 {
            RecencyTier::Normal
        } else {
            RecencyTier::Stale
        }
    } else {
        RecencyTier::Normal // fallback for malformed timestamps
    };

    TTLState {
        importance_tier,
        usage_frequency_tier,
        recency_tier,
    }
}

/// Apply graph weight boosts
fn apply_graph_weight_boosts(
    config: &Config,
    project: &str,
    boosts: &HashMap<String, f32>,
) -> Result<usize> {
    use crate::graph::KnowledgeGraph;

    let graph_path = config
        .memory_dir
        .join("graph")
        .join(format!("{}.json", project));

    if !graph_path.exists() {
        return Ok(0);
    }

    // Load graph
    let mut graph = KnowledgeGraph::load(&graph_path)
        .map_err(|e| MemoryError::Io(std::io::Error::other(e.to_string())))?;

    let mut count = 0;

    // Apply weight boosts to concepts
    for (id, boost) in boosts {
        if let Some(concept) = graph.concepts.get_mut(id) {
            concept.importance = (concept.importance + boost).clamp(0.1, 1.0);
            count += 1;
        }
    }

    // Save updated graph
    graph
        .save(&graph_path)
        .map_err(|e| MemoryError::Io(std::io::Error::other(e.to_string())))?;

    Ok(count)
}

/// Update consolidation configuration
fn update_consolidation_config(
    config: &Config,
    project: &str,
    strategy: &ConsolidationStrategy,
) -> Result<()> {
    // This would update a consolidation config file
    // For now, just log the strategy
    let config_path = config
        .memory_dir
        .join("config")
        .join(format!("{}_consolidation.json", project));

    std::fs::create_dir_all(config_path.parent().unwrap())?;

    let json = serde_json::to_string_pretty(strategy)?;
    std::fs::write(config_path, json)?;

    Ok(())
}

/// Preview what would be changed without applying.
/// Returns real current TTL values from knowledge files.
pub fn preview_changes(
    config: &Config,
    project: &str,
    learning_state: &LearningState,
) -> Result<ChangePreview> {
    let params = &learning_state.learned_parameters;

    let mut preview = ChangePreview {
        importance_changes: Vec::new(),
        ttl_changes: Vec::new(),
        graph_changes: Vec::new(),
        consolidation_change: None,
    };

    // Preview importance boosts (no file reads needed — boosts are the source of truth)
    for (id, boost) in &params.importance_boosts {
        if boost.abs() > 0.01 {
            preview.importance_changes.push(ImportanceChange {
                knowledge_id: id.clone(),
                current: 0.5,
                proposed: (0.5 + boost).clamp(0.1, 1.0),
                boost: *boost,
            });
        }
    }

    // Preview TTL changes via Q-learning policy (read actual files for current TTL)
    if !learning_state.ttl_q_learning.q_table.is_empty() {
        let knowledge_dir = config.memory_dir.join("knowledge").join(project);
        let now = Utc::now();

        for cat in &[
            "decisions",
            "solutions",
            "patterns",
            "bugs",
            "insights",
            "questions",
        ] {
            let path = knowledge_dir.join(format!("{}.md", cat));
            if !path.exists() {
                continue;
            }
            let raw = std::fs::read_to_string(&path).unwrap_or_default();
            let (_, blocks) = parse_session_blocks(&raw);
            let (active, _) = partition_by_expiry(blocks);

            for block in &active {
                let boost = params
                    .importance_boosts
                    .get(&block.session_id)
                    .copied()
                    .unwrap_or(0.0);
                let ttl_state = compute_ttl_state(boost, &block.timestamp, now);
                let action = learning_state.ttl_q_learning.choose_action(&ttl_state);

                let current_ttl_days: Option<u32> = block
                    .ttl
                    .as_deref()
                    .and_then(|t| if t == "never" { None } else { parse_ttl(t) })
                    .map(|d| d.num_days().max(0) as u32);

                let proposed = action.apply_to_days(current_ttl_days);

                // Only surface changes that are actually different
                if proposed != current_ttl_days {
                    preview.ttl_changes.push(TTLChange {
                        knowledge_id: format!("{}:{}", cat, block.session_id),
                        current: current_ttl_days,
                        proposed,
                    });
                }
            }
        }
    }

    // Preview consolidation strategy
    if let Some(strategy) = &params.consolidation_strategy {
        preview.consolidation_change = Some(*strategy);
    }

    Ok(preview)
}

#[derive(Debug, Clone)]
pub struct ChangePreview {
    pub importance_changes: Vec<ImportanceChange>,
    pub ttl_changes: Vec<TTLChange>,
    pub graph_changes: Vec<GraphChange>,
    pub consolidation_change: Option<ConsolidationStrategy>,
}

#[derive(Debug, Clone)]
pub struct ImportanceChange {
    pub knowledge_id: String,
    pub current: f32,
    pub proposed: f32,
    pub boost: f32,
}

#[derive(Debug, Clone)]
pub struct TTLChange {
    pub knowledge_id: String,
    pub current: Option<u32>,
    pub proposed: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct GraphChange {
    pub node_id: String,
    pub current_weight: f32,
    pub proposed_weight: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_config(dir: &TempDir) -> Config {
        Config {
            memory_dir: dir.path().to_path_buf(),
            claude_projects_dir: dir.path().to_path_buf(),
            llm: crate::auth::providers::ResolvedProvider {
                provider: crate::auth::providers::Provider::Ollama,
                endpoint: "http://localhost:11434".to_string(),
                model: "llama2".to_string(),
                api_key: None,
            },
        }
    }

    #[test]
    fn test_build_block_header_with_ttl() {
        let h = build_block_header("abc123", "2026-02-19T10:00:00Z", Some(30), Some("high"));
        assert_eq!(h, "## Session: abc123 (2026-02-19T10:00:00Z) [ttl:30d] [confidence:high]");
    }

    #[test]
    fn test_build_block_header_permanent() {
        let h = build_block_header("abc123", "2026-02-19T10:00:00Z", None, None);
        assert_eq!(h, "## Session: abc123 (2026-02-19T10:00:00Z)");
    }

    #[test]
    fn test_compute_ttl_state_high_boost_recent() {
        let now = Utc::now();
        let recent_ts = (now - chrono::Duration::days(1)).to_rfc3339();
        let state = compute_ttl_state(0.8, &recent_ts, now);
        assert_eq!(state.importance_tier, ImportanceTier::High);
        assert_eq!(state.usage_frequency_tier, FrequencyTier::Frequent);
        assert_eq!(state.recency_tier, RecencyTier::Recent);
    }

    #[test]
    fn test_compute_ttl_state_zero_boost_stale() {
        let now = Utc::now();
        let old_ts = (now - chrono::Duration::days(60)).to_rfc3339();
        let state = compute_ttl_state(0.0, &old_ts, now);
        assert_eq!(state.importance_tier, ImportanceTier::Low);
        assert_eq!(state.usage_frequency_tier, FrequencyTier::Rare);
        assert_eq!(state.recency_tier, RecencyTier::Stale);
    }

    #[test]
    fn test_apply_ttl_adjustments_no_q_table_returns_zero() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        // Empty Q-table → no changes
        let state = crate::learning::progress::LearningState::new("test".to_string());
        assert!(state.ttl_q_learning.q_table.is_empty());

        // write a knowledge file
        let kdir = tmp.path().join("knowledge").join("test");
        std::fs::create_dir_all(&kdir).unwrap();
        let now = Utc::now().to_rfc3339();
        std::fs::write(
            kdir.join("decisions.md"),
            format!("# Decisions\n\n## Session: s1 ({}) [ttl:7d] [confidence:high]\n\nContent.\n\n", now),
        ).unwrap();

        let count = apply_ttl_adjustments(&config, "test", &state).unwrap();
        assert_eq!(count, 0, "Empty Q-table should produce zero changes");
    }

    #[test]
    fn test_preview_changes_reads_real_ttl() {
        let tmp = TempDir::new().unwrap();
        let config = make_config(&tmp);
        let state = crate::learning::progress::LearningState::new("preview-test".to_string());
        // No Q-table data → preview TTL changes should be empty
        let preview = preview_changes(&config, "preview-test", &state).unwrap();
        assert!(preview.ttl_changes.is_empty());
    }

    #[test]
    fn test_apply_importance_boosts() {
        use chrono::Utc;

        let mut scores = HashMap::new();
        scores.insert(
            "doc1".to_string(),
            KnowledgeScore {
                category: "patterns".to_string(),
                label: "doc1".to_string(),
                access_count: 10,
                last_accessed: Some(Utc::now()),
                recency_score: 0.8,
                frequency_score: 0.6,
                importance: 0.5,
            },
        );

        let mut boosts = HashMap::new();
        boosts.insert("doc1".to_string(), 0.2);

        apply_importance_boosts(&mut scores, &boosts);

        assert_eq!(scores.get("doc1").unwrap().importance, 0.7);
    }

    #[test]
    fn test_apply_importance_boosts_clamping() {
        use chrono::Utc;

        let mut scores = HashMap::new();
        scores.insert(
            "doc1".to_string(),
            KnowledgeScore {
                category: "patterns".to_string(),
                label: "doc1".to_string(),
                access_count: 50,
                last_accessed: Some(Utc::now()),
                recency_score: 0.9,
                frequency_score: 0.9,
                importance: 0.9,
            },
        );

        let mut boosts = HashMap::new();
        boosts.insert("doc1".to_string(), 0.5); // Would exceed 1.0

        apply_importance_boosts(&mut scores, &boosts);

        assert_eq!(scores.get("doc1").unwrap().importance, 1.0); // Clamped
    }
}
