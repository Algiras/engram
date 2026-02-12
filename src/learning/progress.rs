use crate::error::Result;
use crate::learning::adaptation::LearnedParameters;
use crate::learning::algorithms::{ConsolidationBandit, TTLQLearning};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Complete learning state for a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningState {
    pub project: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    /// Learned parameters ready to apply
    pub learned_parameters: LearnedParameters,

    /// Q-learning state for TTL optimization
    pub ttl_q_learning: TTLQLearning,

    /// Multi-armed bandit for consolidation
    pub consolidation_bandit: ConsolidationBandit,

    /// Learning hyperparameters
    pub hyperparameters: Hyperparameters,

    /// Tracked metrics over time
    pub metrics_history: Vec<MetricsSnapshot>,

    /// Adaptation history (what was applied and when)
    pub adaptation_history: Vec<AdaptationRecord>,
}

impl LearningState {
    pub fn new(project: String) -> Self {
        let now = Utc::now();
        Self {
            project,
            created_at: now,
            updated_at: now,
            learned_parameters: LearnedParameters::default(),
            ttl_q_learning: TTLQLearning::default(),
            consolidation_bandit: ConsolidationBandit::default(),
            hyperparameters: Hyperparameters::default(),
            metrics_history: Vec::new(),
            adaptation_history: Vec::new(),
        }
    }

    /// Check if learning has converged (parameters stable)
    pub fn has_converged(&self) -> bool {
        if self.metrics_history.len() < 10 {
            return false;
        }

        // Check if last 10 health scores have low variance
        let recent_health: Vec<f32> = self
            .metrics_history
            .iter()
            .rev()
            .take(10)
            .map(|m| m.health_score as f32)
            .collect();

        let mean = recent_health.iter().sum::<f32>() / recent_health.len() as f32;
        let variance = recent_health
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f32>()
            / recent_health.len() as f32;

        variance < 10.0 // Low variance = converged
    }

    /// Compute adaptation success rate
    pub fn adaptation_success_rate(&self) -> f32 {
        if self.adaptation_history.is_empty() {
            return 0.0;
        }

        let successful = self
            .adaptation_history
            .iter()
            .filter(|r| r.health_improvement > 0)
            .count();

        successful as f32 / self.adaptation_history.len() as f32
    }

    /// Get the number of sessions/iterations processed
    pub fn session_count(&self) -> usize {
        self.metrics_history.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hyperparameters {
    /// Learning rate for TD learning (importance)
    pub importance_learning_rate: f32,

    /// Learning rate for Q-learning (TTL)
    pub ttl_learning_rate: f32,

    /// Discount factor for Q-learning
    pub ttl_discount_factor: f32,

    /// Exploration rate (epsilon) for all algorithms
    pub exploration_rate: f32,
}

impl Default for Hyperparameters {
    fn default() -> Self {
        Self {
            importance_learning_rate: 0.2,
            ttl_learning_rate: 0.1,
            ttl_discount_factor: 0.9,
            exploration_rate: 0.2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub timestamp: DateTime<Utc>,
    pub health_score: u8,
    pub avg_query_time_ms: u32,
    pub stale_knowledge_pct: f32,
    pub storage_size_mb: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaptationRecord {
    pub timestamp: DateTime<Utc>,
    pub importance_adjustments: usize,
    pub ttl_adjustments: usize,
    pub graph_adjustments: usize,
    pub health_before: u8,
    pub health_after: u8,
    pub health_improvement: i8,
}

/// Load learning state from disk
pub fn load_state(memory_dir: &Path, project: &str) -> Result<LearningState> {
    let path = get_state_path(memory_dir, project);

    if !path.exists() {
        return Ok(LearningState::new(project.to_string()));
    }

    let content = std::fs::read_to_string(&path)?;
    let state: LearningState = serde_json::from_str(&content)?;

    Ok(state)
}

/// Save learning state to disk
pub fn save_state(memory_dir: &Path, state: &LearningState) -> Result<()> {
    let path = get_state_path(memory_dir, &state.project);

    std::fs::create_dir_all(path.parent().unwrap())?;

    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(path, json)?;

    Ok(())
}

/// Get the path to the learning state file
fn get_state_path(memory_dir: &Path, project: &str) -> PathBuf {
    memory_dir
        .join("learning")
        .join(format!("{}.json", project))
}

/// Reset learning state to defaults (but preserve history)
pub fn reset_state(memory_dir: &Path, project: &str) -> Result<()> {
    let mut state = load_state(memory_dir, project)?;

    // Reset learned parameters
    state.learned_parameters = LearnedParameters::default();

    // Reset algorithms
    state.ttl_q_learning = TTLQLearning::default();
    state.consolidation_bandit = ConsolidationBandit::default();

    // Keep history for analysis
    state.updated_at = Utc::now();

    save_state(memory_dir, &state)?;

    Ok(())
}

/// Maximum metrics history to keep (sliding window)
const MAX_METRICS_HISTORY: usize = 100;

/// Record a new metrics snapshot
pub fn record_metrics(
    state: &mut LearningState,
    health_score: u8,
    avg_query_time_ms: u32,
    stale_knowledge_pct: f32,
    storage_size_mb: f32,
) {
    // Sliding window: keep only last MAX_METRICS_HISTORY entries
    if state.metrics_history.len() >= MAX_METRICS_HISTORY {
        state.metrics_history.remove(0);
    }

    state.metrics_history.push(MetricsSnapshot {
        timestamp: Utc::now(),
        health_score,
        avg_query_time_ms,
        stale_knowledge_pct,
        storage_size_mb,
    });

    state.updated_at = Utc::now();
}

/// Record an adaptation application
pub fn record_adaptation(
    state: &mut LearningState,
    importance_adjustments: usize,
    ttl_adjustments: usize,
    graph_adjustments: usize,
    health_before: u8,
    health_after: u8,
) {
    let health_improvement = health_after as i8 - health_before as i8;

    state.adaptation_history.push(AdaptationRecord {
        timestamp: Utc::now(),
        importance_adjustments,
        ttl_adjustments,
        graph_adjustments,
        health_before,
        health_after,
        health_improvement,
    });

    state.updated_at = Utc::now();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learning_state_new() {
        let state = LearningState::new("test-project".to_string());
        assert_eq!(state.project, "test-project");
        assert_eq!(state.metrics_history.len(), 0);
    }

    #[test]
    fn test_convergence_detection() {
        let mut state = LearningState::new("test".to_string());

        // Not converged with <10 samples
        assert!(!state.has_converged());

        // Add 10 samples with low variance
        for _ in 0..10 {
            record_metrics(&mut state, 90, 100, 5.0, 10.0);
        }

        assert!(state.has_converged());
    }

    #[test]
    fn test_adaptation_success_rate() {
        let mut state = LearningState::new("test".to_string());

        record_adaptation(&mut state, 5, 3, 2, 80, 85);
        record_adaptation(&mut state, 3, 2, 1, 85, 90);
        record_adaptation(&mut state, 2, 1, 0, 90, 88); // Failed

        let rate = state.adaptation_success_rate();
        assert!((rate - 0.666).abs() < 0.01); // 2/3 successful
    }
}
