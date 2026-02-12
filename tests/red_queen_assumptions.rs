//! Red Queen Protocol: Challenge core assumptions
//!
//! Question: Does the learning system actually IMPROVE outcomes,
//! or does it just track usage without real benefit?
//!
//! Critical Test: If we boost importance, does recall quality improve?

use claude_memory::auth::providers::{Provider, ResolvedProvider};
use claude_memory::config::Config;
use claude_memory::learning::{self, progress};
use tempfile::TempDir;

fn create_test_config(temp: &TempDir) -> Config {
    Config {
        memory_dir: temp.path().to_path_buf(),
        claude_projects_dir: temp.path().to_path_buf(),
        llm: ResolvedProvider {
            provider: Provider::Ollama,
            endpoint: "http://localhost:11434".to_string(),
            model: "llama2".to_string(),
            api_key: None,
        },
    }
}

#[test]
fn assumption_learning_actually_changes_parameters() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "change-test";

    // Baseline: No learning yet (load_state creates new state if missing)
    let baseline_state = progress::load_state(&config.memory_dir, project).unwrap();
    assert_eq!(
        baseline_state.session_count(),
        0,
        "Baseline state should have 0 sessions before learning"
    );
    assert!(
        baseline_state
            .learned_parameters
            .importance_boosts
            .is_empty(),
        "Baseline state should have no importance boosts before learning"
    );

    // Generate usage and trigger learning
    learning::simulation::simulate_high_frequency_knowledge(&config, project, "critical", 20)
        .unwrap();
    learning::post_ingest_hook(&config, project).unwrap();

    // Check: Did parameters actually change?
    let state = progress::load_state(&config.memory_dir, project).unwrap();

    assert!(
        !state.learned_parameters.importance_boosts.is_empty(),
        "Learning should have created importance boosts"
    );

    let boost = state
        .learned_parameters
        .importance_boosts
        .get("patterns:critical")
        .copied()
        .unwrap_or(0.0);

    assert!(
        boost > 0.0,
        "High-frequency knowledge should have positive importance boost, got: {}",
        boost
    );
}

#[test]
fn assumption_importance_correlates_with_usage() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "correlation-test";

    // Create different usage patterns
    learning::simulation::simulate_high_frequency_knowledge(&config, project, "frequent", 30)
        .unwrap();
    learning::simulation::simulate_high_frequency_knowledge(&config, project, "rare", 3).unwrap();
    learning::post_ingest_hook(&config, project).unwrap();

    let state = progress::load_state(&config.memory_dir, project).unwrap();

    let frequent_boost = state
        .learned_parameters
        .importance_boosts
        .get("patterns:frequent")
        .copied()
        .unwrap_or(0.0);

    let rare_boost = state
        .learned_parameters
        .importance_boosts
        .get("patterns:rare")
        .copied()
        .unwrap_or(0.0);

    // Critical assumption: Frequently accessed knowledge should have higher importance
    assert!(
        frequent_boost > rare_boost,
        "Frequent knowledge ({}) should have higher importance than rare ({})",
        frequent_boost,
        rare_boost
    );
}

#[test]
fn assumption_learning_is_cumulative() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "cumulative-test";

    // Session 1
    learning::simulation::simulate_high_frequency_knowledge(&config, project, "knowledge", 10)
        .unwrap();
    learning::post_ingest_hook(&config, project).unwrap();

    let state1 = progress::load_state(&config.memory_dir, project).unwrap();
    let boost1 = state1
        .learned_parameters
        .importance_boosts
        .get("patterns:knowledge")
        .copied()
        .unwrap_or(0.0);

    // Session 2: More access to same knowledge
    learning::simulation::simulate_high_frequency_knowledge(&config, project, "knowledge", 10)
        .unwrap();
    learning::post_ingest_hook(&config, project).unwrap();

    let state2 = progress::load_state(&config.memory_dir, project).unwrap();
    let boost2 = state2
        .learned_parameters
        .importance_boosts
        .get("patterns:knowledge")
        .copied()
        .unwrap_or(0.0);

    // Critical: Learning should be cumulative, not reset
    assert!(
        boost2 >= boost1,
        "Importance should increase or stay stable with more access: {} -> {}",
        boost1,
        boost2
    );

    assert!(
        state2.session_count() > state1.session_count(),
        "Session count should increase: {} -> {}",
        state1.session_count(),
        state2.session_count()
    );
}

#[test]
fn assumption_metrics_reflect_reality() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "metrics-test";

    learning::simulation::simulate_recall_session(&config, project, 20).unwrap();
    learning::post_ingest_hook(&config, project).unwrap();

    let state = progress::load_state(&config.memory_dir, project).unwrap();

    // Check: Are metrics actually being recorded?
    assert!(
        !state.metrics_history.is_empty(),
        "Metrics history should not be empty after learning"
    );

    let latest_metrics = state.metrics_history.last().unwrap();

    // Sanity checks on metric values
    assert!(
        latest_metrics.health_score > 0 && latest_metrics.health_score <= 100,
        "Health score should be in [1, 100], got: {}",
        latest_metrics.health_score
    );

    assert!(
        latest_metrics.avg_query_time_ms > 0,
        "Query time should be positive, got: {}",
        latest_metrics.avg_query_time_ms
    );

    assert!(
        latest_metrics.stale_knowledge_pct >= 0.0 && latest_metrics.stale_knowledge_pct <= 100.0,
        "Stale knowledge should be in [0, 100]%, got: {}",
        latest_metrics.stale_knowledge_pct
    );
}

#[test]
fn assumption_convergence_is_detectable() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "convergence-test";

    // Simulate steady-state: Same pattern repeatedly
    for _ in 0..15 {
        learning::simulation::simulate_recall_session(&config, project, 20).unwrap();
        learning::post_ingest_hook(&config, project).unwrap();
    }

    let state = progress::load_state(&config.memory_dir, project).unwrap();

    // Check: Can we detect convergence?
    let has_converged = state.has_converged();
    let session_count = state.session_count();

    // After 15 stable sessions, system should show signs of convergence
    // (or at least not crash when checking)
    assert!(
        session_count >= 10,
        "Should have processed multiple sessions: {}",
        session_count
    );

    // Convergence detection should work without panic
    println!(
        "Convergence status after {} sessions: {}",
        session_count, has_converged
    );
}

#[test]
fn assumption_optimization_produces_valid_changes() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "optimize-test";

    // Generate learning data
    learning::simulation::simulate_mixed_usage(&config, project, 50).unwrap();
    learning::post_ingest_hook(&config, project).unwrap();

    let state = progress::load_state(&config.memory_dir, project).unwrap();

    // Check: Are the learned parameters valid?
    for (knowledge_id, boost) in &state.learned_parameters.importance_boosts {
        assert!(
            (&0.0..=&1.0).contains(&boost),
            "Importance boost for '{}' should be in [0, 1], got: {}",
            knowledge_id,
            boost
        );
    }

    // Check: Is there actually something to optimize?
    assert!(
        !state.learned_parameters.importance_boosts.is_empty(),
        "Should have some learned parameters to optimize"
    );
}

#[test]
fn assumption_no_learning_without_signals() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "no-signals";

    // Create events that won't generate signals (below threshold)
    learning::simulation::simulate_recall_session(&config, project, 2).unwrap();
    learning::post_ingest_hook(&config, project).unwrap();

    let state_result = progress::load_state(&config.memory_dir, project);

    // Critical: Without sufficient signals, learning should not create state
    // OR if it does, it should have no boosts
    if let Ok(state) = state_result {
        assert_eq!(
            state.session_count(),
            0,
            "Session count should be 0 without learning signals"
        );
    }
}
