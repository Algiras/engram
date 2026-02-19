use crate::analytics::tracker::EventTracker;
use crate::config::Config;
use crate::error::Result;
use crate::learning::{outcome_signals, progress, signals};

/// Hook called after ingest to extract learning signals
pub fn post_ingest_hook(config: &Config, project: &str) -> Result<()> {
    let tracker = EventTracker::new(&config.memory_dir);
    let events = tracker.get_events(Some(project), 30)?;

    if events.is_empty() {
        return Ok(());
    }

    // Extract signals from recent events
    let learning_signals = signals::extract_signals_from_events(&events);

    // Update learning state
    let mut state = progress::load_state(&config.memory_dir, project)?;

    // Always train the TTL Q-table from current knowledge state (even if no
    // usage signals yet) so the policy is warm-started from the first ingest.
    train_ttl_q_table_from_knowledge(config, project, &mut state);

    if learning_signals.is_empty() {
        progress::save_state(&config.memory_dir, &state)?;
        return Ok(());
    }

    // Track metrics snapshot using real data
    let health_score = crate::health::check_project_health(&config.memory_dir, project)
        .map(|r| r.score)
        .unwrap_or(75);
    let avg_query_time = 100; // Would need actual timing instrumentation
    let stale_knowledge = compute_stale_percentage(&config.memory_dir, project);
    let storage_size = compute_dir_size_mb(
        &config
            .memory_dir
            .join("knowledge")
            .join(project),
    );

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
        let weighted_reward = reward * 1.5; // 50% more weight for explicit outcomes

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
                state.hyperparameters.importance_learning_rate * 1.2, // Learn faster from outcomes
            );

            state
                .learned_parameters
                .importance_boosts
                .insert(knowledge_id, new_importance);
        }
    }

    // Knowledge decay: reduce importance of all boosts slightly each cycle (Feature 4)
    // Entries that are recalled or searched will have their boosts refreshed via learning signals.
    // Entries not accessed will gradually fade toward neutral importance.
    for boost in state.learned_parameters.importance_boosts.values_mut() {
        *boost = (*boost * crate::config::IMPORTANCE_DECAY_FACTOR).max(0.0);
    }

    // Save updated state
    progress::save_state(&config.memory_dir, &state)?;

    Ok(())
}

/// Hook called after recall to boost importance of accessed knowledge.
/// Also increments access_count and boosts strength (FadeMem) for each recalled block.
pub fn post_recall_hook(
    config: &Config,
    project: &str,
    knowledge_accessed: &[String],
) -> Result<()> {
    if knowledge_accessed.is_empty() {
        return Ok(());
    }

    let mut state = progress::load_state(&config.memory_dir, project)?;

    // Recall is a positive signal: knowledge that gets read is valuable
    let recall_reward = 0.6;

    for knowledge_id in knowledge_accessed {
        // Update importance boosts (learning state)
        let current = state
            .learned_parameters
            .importance_boosts
            .get(knowledge_id)
            .copied()
            .unwrap_or(0.0);

        let new_importance = crate::learning::algorithms::learn_importance(
            current,
            recall_reward,
            state.hyperparameters.importance_learning_rate,
        );

        state
            .learned_parameters
            .importance_boosts
            .insert(knowledge_id.clone(), new_importance);
    }

    progress::save_state(&config.memory_dir, &state)?;

    // Write back access_count + strength to knowledge files (FadeMem + access tracking)
    let knowledge_dir = config.memory_dir.join("knowledge").join(project);
    for knowledge_id in knowledge_accessed {
        // knowledge_id may be "category:session_id" or just "session_id"
        let (cat_hint, bare_id) = if let Some(pos) = knowledge_id.find(':') {
            let (cat, id) = knowledge_id.split_at(pos);
            (Some(cat.to_string()), id[1..].to_string())
        } else {
            (None, knowledge_id.clone())
        };

        for cat_file in crate::config::CATEGORY_FILES {
            // Optimization: if we have a category hint, only scan matching file
            if let Some(ref hint) = cat_hint {
                if !cat_file.starts_with(hint.as_str()) {
                    continue;
                }
            }

            let path = knowledge_dir.join(cat_file);
            if !path.exists() {
                continue;
            }

            let content = match std::fs::read_to_string(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Combined read-modify-write: increment access_count AND boost strength
            use crate::extractor::knowledge::{parse_session_blocks, build_header, reconstruct_blocks};
            let (preamble, mut blocks) = parse_session_blocks(&content);
            let mut modified = false;

            for block in &mut blocks {
                if block.session_id == bare_id {
                    block.access_count = Some(block.access_count.unwrap_or(0) + 1);
                    let new_strength = (block
                        .strength
                        .unwrap_or(crate::config::INITIAL_STRENGTH)
                        + crate::config::STRENGTH_RECALL_BOOST)
                        .min(crate::config::STRENGTH_MAX);
                    block.strength = Some(new_strength);
                    // Rebuild header in-place
                    block.header = build_header(
                        &block.session_id,
                        &block.timestamp,
                        block.ttl.as_deref(),
                        block.confidence.as_deref(),
                        block.strength,
                        block.access_count,
                    );
                    modified = true;
                    break; // Each session_id appears once per file
                }
            }

            if modified {
                let rebuilt = reconstruct_blocks(&preamble, &blocks);
                let _ = std::fs::write(&path, rebuilt);
                break; // Found and updated — skip remaining files
            }
        }
    }

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

/// Train the TTL Q-table from observed knowledge state.
///
/// For each active block we compute its TTLState and derive a reward signal
/// from the existing importance boost + recency tier:
///
/// | Situation                        | Preferred action | Reward |
/// |----------------------------------|------------------|--------|
/// | High boost + Recent              | Extend30d        |  0.8   |
/// | Medium boost + Normal            | Extend7d         |  0.5   |
/// | Low boost + Stale                | Reduce7d         |  0.7   |
/// | Low boost + Recent (new entry)   | Extend7d         |  0.3   |
/// | Any + Stale (no boost)           | Reduce3d         |  0.6   |
fn train_ttl_q_table_from_knowledge(
    config: &Config,
    project: &str,
    state: &mut crate::learning::progress::LearningState,
) {
    use crate::extractor::knowledge::{parse_session_blocks, partition_by_expiry};
    use crate::learning::adaptation::compute_ttl_state;
    use crate::learning::algorithms::{ImportanceTier, RecencyTier, TTLAction};
    use chrono::Utc;

    let knowledge_dir = config.memory_dir.join("knowledge").join(project);
    let now = Utc::now();

    for cat in crate::config::CATEGORIES {
        let path = knowledge_dir.join(format!("{}.md", cat));
        if let Ok(content) = std::fs::read_to_string(&path) {
            let (_, blocks) = parse_session_blocks(&content);
            let (active, _) = partition_by_expiry(blocks);

            for block in &active {
                let boost = state
                    .learned_parameters
                    .importance_boosts
                    .get(&block.session_id)
                    .copied()
                    .unwrap_or(0.0);

                let ttl_state = compute_ttl_state(boost, &block.timestamp, now);

                // Choose a reward-action pair based on observed state
                let (action, reward): (TTLAction, f32) =
                    match (&ttl_state.importance_tier, &ttl_state.recency_tier) {
                        (ImportanceTier::High, RecencyTier::Recent) => {
                            (TTLAction::Extend30d, 0.8)
                        }
                        (ImportanceTier::High, _) => (TTLAction::Extend14d, 0.7),
                        (ImportanceTier::Medium, RecencyTier::Stale) => (TTLAction::Extend7d, 0.4),
                        (ImportanceTier::Medium, _) => (TTLAction::Extend7d, 0.5),
                        (ImportanceTier::Low, RecencyTier::Stale) => (TTLAction::Reduce7d, 0.7),
                        (ImportanceTier::Low, RecencyTier::Normal) => (TTLAction::Reduce3d, 0.5),
                        (ImportanceTier::Low, RecencyTier::Recent) => (TTLAction::Extend7d, 0.3),
                    };

                // Update Q-table: treat current state as both current and next state
                // (steady-state approximation — reward is the signal we care about)
                state
                    .ttl_q_learning
                    .update(ttl_state.clone(), action, reward, ttl_state);
            }
        }
    }
}

/// Compute the percentage of knowledge entries older than 30 days (stale).
fn compute_stale_percentage(memory_dir: &std::path::Path, project: &str) -> f32 {
    use crate::extractor::knowledge::{parse_session_blocks, partition_by_expiry};

    let knowledge_dir = memory_dir.join("knowledge").join(project);
    let now = chrono::Utc::now();
    let threshold = chrono::Duration::days(30);

    let mut total = 0usize;
    let mut stale = 0usize;

    for cat in crate::config::CATEGORIES {
        let path = knowledge_dir.join(format!("{}.md", cat));
        if let Ok(content) = std::fs::read_to_string(&path) {
            let (_, blocks) = parse_session_blocks(&content);
            let (active, _) = partition_by_expiry(blocks);
            for block in &active {
                total += 1;
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&block.timestamp) {
                    if now - ts.with_timezone(&chrono::Utc) > threshold {
                        stale += 1;
                    }
                }
            }
        }
    }

    if total == 0 {
        0.0
    } else {
        stale as f32 / total as f32 * 100.0
    }
}

/// Compute total size of a directory in megabytes (shallow, files only).
fn compute_dir_size_mb(dir: &std::path::Path) -> f32 {
    if !dir.exists() {
        return 0.0;
    }
    let mut total_bytes = 0u64;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    total_bytes += meta.len();
                }
            }
        }
    }
    total_bytes as f32 / 1_048_576.0
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
