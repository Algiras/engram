use crate::analytics::tracker::EventTracker;
use crate::config::Config;
use crate::error::Result;
use crate::learning::{progress, signals, outcome_signals};

/// Hook called after ingest to extract learning signals
pub fn post_ingest_hook(config: &Config, project: &str) -> Result<()> {
    let tracker = EventTracker::new(&config.memory_dir);
    let events = tracker.get_events(Some(project), 30)?;

    if events.is_empty() {
        return Ok(());
    }

    // Extract signals from recent events
    let learning_signals = signals::extract_signals_from_events(&events);

    if learning_signals.is_empty() {
        return Ok(());
    }

    // Update learning state with new signals
    let mut state = progress::load_state(&config.memory_dir, project)?;

    // Track metrics snapshot
    let health_score = 75; // Would call health::check_project_health
    let avg_query_time = 100;
    let stale_knowledge = 10.0;
    let storage_size = 5.0;

    progress::record_metrics(
        &mut state,
        health_score,
        avg_query_time,
        stale_knowledge,
        storage_size,
    );

    // Apply learning algorithms to usage-based signals
    for signal in &learning_signals {
        let reward = signal.to_reward();

        // Update importance for affected knowledge
        for knowledge_id in signal.knowledge_ids() {
            let current_importance = state
                .learned_parameters
                .importance_boosts
                .get(&knowledge_id)
                .copied()
                .unwrap_or(0.0);

            let new_importance = crate::learning::algorithms::learn_importance(
                current_importance,
                reward,
                state.hyperparameters.importance_learning_rate,
            );

            state
                .learned_parameters
                .importance_boosts
                .insert(knowledge_id, new_importance);
        }
    }

    // Process outcome-based signals (feedback, errors, success rates)
    let outcome_signals = outcome_signals::load_outcome_signals(&config.memory_dir, project)?;

    for outcome in &outcome_signals {
        let reward = outcome.to_reward();

        // Outcome signals have higher weight than usage signals
        let weighted_reward = reward * 1.5;  // 50% more weight for explicit outcomes

        for knowledge_id in outcome.knowledge_ids() {
            let current_importance = state
                .learned_parameters
                .importance_boosts
                .get(&knowledge_id)
                .copied()
                .unwrap_or(0.0);

            let new_importance = crate::learning::algorithms::learn_importance(
                current_importance,
                weighted_reward,
                state.hyperparameters.importance_learning_rate * 1.2,  // Learn faster from outcomes
            );

            state
                .learned_parameters
                .importance_boosts
                .insert(knowledge_id, new_importance);
        }
    }

    // Save updated state
    progress::save_state(&config.memory_dir, &state)?;

    Ok(())
}

/// Hook called after recall to track successful knowledge access
pub fn post_recall_hook(config: &Config, project: &str, _knowledge_accessed: &[String]) -> Result<()> {
    // Track the recall event (already done by analytics)
    // Future: Could boost importance of accessed knowledge here
    let _state = progress::load_state(&config.memory_dir, project)?;
    // For now, just ensure learning state exists
    Ok(())
}

/// Hook called after consolidate to learn from merge patterns
pub fn post_consolidate_hook(
    config: &Config,
    project: &str,
    merge_count: usize,
    user_confirmed: bool,
) -> Result<()> {
    if merge_count == 0 {
        return Ok(());
    }

    let mut state = progress::load_state(&config.memory_dir, project)?;

    // Reward based on user acceptance
    let reward = if user_confirmed { 0.8 } else { 0.3 };

    // Update consolidation bandit
    let arm_index = 0; // Would track which strategy was used
    state.consolidation_bandit.update_reward(arm_index, reward);

    // Save updated state
    progress::save_state(&config.memory_dir, &state)?;

    Ok(())
}

/// Hook called by doctor --fix to record health improvements
pub fn post_doctor_fix_hook(
    config: &Config,
    project: &str,
    health_before: u8,
    health_after: u8,
) -> Result<()> {
    let signal = signals::extract_health_improvement_signal(
        &crate::health::HealthReport {
            project: project.to_string(),
            score: health_before,
            issues: Vec::new(),
            recommendations: Vec::new(),
        },
        &crate::health::HealthReport {
            project: project.to_string(),
            score: health_after,
            issues: Vec::new(),
            recommendations: Vec::new(),
        },
        Vec::new(), // Would track which knowledge helped
    );

    if let Some(_signal) = signal {
        // Would update learning state here
        let _state = progress::load_state(&config.memory_dir, project)?;
        // For now, just acknowledge the improvement
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_post_ingest_hook() {
        use crate::auth::providers::{Provider, ResolvedProvider};

        let temp = TempDir::new().unwrap();
        let config = Config {
            memory_dir: temp.path().to_path_buf(),
            claude_projects_dir: temp.path().to_path_buf(),
            llm: ResolvedProvider {
                provider: Provider::Ollama,
                endpoint: "http://localhost:11434".to_string(),
                model: "llama2".to_string(),
                api_key: None,
            },
        };

        // Should not fail even with no data
        let result = post_ingest_hook(&config, "test-project");
        assert!(result.is_ok());
    }

    #[test]
    fn test_post_recall_hook() {
        use crate::auth::providers::{Provider, ResolvedProvider};

        let temp = TempDir::new().unwrap();
        let config = Config {
            memory_dir: temp.path().to_path_buf(),
            claude_projects_dir: temp.path().to_path_buf(),
            llm: ResolvedProvider {
                provider: Provider::Ollama,
                endpoint: "http://localhost:11434".to_string(),
                model: "llama2".to_string(),
                api_key: None,
            },
        };

        let result = post_recall_hook(&config, "test-project", &[]);
        assert!(result.is_ok());
    }
}
