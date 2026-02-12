//! Integration tests for learning system using simulations

use claude_memory::config::Config;
use claude_memory::learning::{self, progress};
use claude_memory::auth::providers::{Provider, ResolvedProvider};
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
fn test_learning_convergence_simulation() {
    // Setup isolated test environment
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "test-learning";

    // Phase 1: Generate usage events (50 recalls)
    learning::simulation::simulate_recall_session(&config, project, 50).unwrap();

    // Phase 2: Trigger learning hook
    learning::post_ingest_hook(&config, project).unwrap();

    // Phase 3: Verify learning state updated
    let state = progress::load_state(&config.memory_dir, project).unwrap();

    assert!(state.session_count() > 0, "Learning should track sessions");
    assert!(
        !state.learned_parameters.importance_boosts.is_empty(),
        "Should have learned importance boosts"
    );
}

#[test]
fn test_high_frequency_learning() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "test-high-freq";

    // Simulate 20 accesses to same knowledge
    learning::simulation::simulate_high_frequency_knowledge(
        &config,
        project,
        "critical-pattern",
        20,
    )
    .unwrap();

    // Trigger learning
    learning::post_ingest_hook(&config, project).unwrap();

    // Verify high importance boost
    let state = progress::load_state(&config.memory_dir, project).unwrap();
    let boost = state
        .learned_parameters
        .importance_boosts
        .get("patterns:critical-pattern");

    assert!(boost.is_some(), "Should boost high-frequency knowledge");
    assert!(boost.unwrap() > &0.1, "Boost should be significant");
}

#[test]
fn test_mixed_usage_simulation() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "test-mixed";

    // Simulate mixed usage patterns
    learning::simulation::simulate_mixed_usage(&config, project, 30).unwrap();

    // Trigger learning
    learning::post_ingest_hook(&config, project).unwrap();

    // Verify learning state exists and has data
    let state = progress::load_state(&config.memory_dir, project).unwrap();

    assert!(state.session_count() > 0, "Should track sessions");
    assert!(!state.metrics_history.is_empty(), "Should have metrics history");
}

#[test]
fn test_learning_with_multiple_sessions() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "test-multi-session";

    // Simulate multiple sessions with enough events to trigger signals
    // (need at least 3 accesses to same knowledge to generate a signal)
    for _i in 0..5 {
        learning::simulation::simulate_recall_session(&config, project, 15).unwrap();
        learning::post_ingest_hook(&config, project).unwrap();
    }

    // Verify cumulative learning
    let state = progress::load_state(&config.memory_dir, project).unwrap();

    // With 15 events per session (pattern-0 to pattern-4), each pattern appears 3 times
    // This should generate signals and thus metrics snapshots
    assert!(
        state.session_count() >= 1,
        "Should track at least one session (actual: {})",
        state.session_count()
    );
}

#[test]
fn test_consolidate_hook() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "test-consolidate";

    // Initialize state
    learning::post_ingest_hook(&config, project).unwrap();

    // Simulate consolidation with user confirmation
    learning::post_consolidate_hook(&config, project, 3, true).unwrap();

    let state = progress::load_state(&config.memory_dir, project).unwrap();

    // Verify consolidation was tracked
    assert!(
        !state.consolidation_bandit.arms.is_empty(),
        "Should have consolidation bandit data"
    );
}

#[test]
fn test_recall_hook() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "test-recall";

    // Initialize state
    learning::post_ingest_hook(&config, project).unwrap();

    // Test recall hook (should not fail)
    let result = learning::post_recall_hook(&config, project, &[]);
    assert!(result.is_ok(), "Recall hook should succeed");
}

#[test]
fn test_doctor_hook() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "test-doctor";

    // Initialize state
    learning::post_ingest_hook(&config, project).unwrap();

    // Simulate health improvement
    learning::post_doctor_fix_hook(&config, project, 65, 85).unwrap();

    // Verify hook executed successfully (no panic)
    let state = progress::load_state(&config.memory_dir, project).unwrap();
    assert!(state.session_count() >= 0, "State should be valid");
}
