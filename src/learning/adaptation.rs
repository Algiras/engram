use crate::analytics::metrics::KnowledgeScore;
use crate::config::Config;
use crate::error::{MemoryError, Result};
use crate::learning::algorithms::ConsolidationStrategy;
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

    // 2. Apply TTL adjustments to knowledge files
    if !params.ttl_adjustments.is_empty() {
        result.ttl_adjustments = apply_ttl_adjustments(config, project, &params.ttl_adjustments)?;
    }

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

/// Apply TTL adjustments to knowledge metadata
fn apply_ttl_adjustments(
    _config: &Config,
    _project: &str,
    adjustments: &HashMap<String, Option<u32>>,
) -> Result<usize> {
    // This would update TTL metadata in knowledge files
    // Placeholder for now - actual implementation would:
    // 1. Read knowledge files
    // 2. Update TTL frontmatter/metadata
    // 3. Write back

    Ok(adjustments.len())
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

/// Preview what would be changed without applying
pub fn preview_changes(
    _config: &Config,
    _project: &str,
    learning_state: &LearningState,
) -> Result<ChangePreview> {
    let params = &learning_state.learned_parameters;

    let mut preview = ChangePreview {
        importance_changes: Vec::new(),
        ttl_changes: Vec::new(),
        graph_changes: Vec::new(),
        consolidation_change: None,
    };

    // Preview importance boosts
    for (id, boost) in &params.importance_boosts {
        preview.importance_changes.push(ImportanceChange {
            knowledge_id: id.clone(),
            current: 0.5, // Would load actual current value
            proposed: (0.5 + boost).clamp(0.1, 1.0),
            boost: *boost,
        });
    }

    // Preview TTL adjustments
    for (id, new_ttl) in &params.ttl_adjustments {
        preview.ttl_changes.push(TTLChange {
            knowledge_id: id.clone(),
            current: Some(7), // Would load actual current value
            proposed: *new_ttl,
        });
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
