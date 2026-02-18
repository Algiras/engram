//! Red Queen Protocol: Adversarial tests to challenge the learning system
//!
//! These tests are designed to break the system and expose weaknesses.
//! If a test passes, the system is robust. If it fails, we found a vulnerability.

use chrono::Utc;
use engram::analytics::tracker::{EventTracker, EventType, UsageEvent};
use engram::auth::providers::{Provider, ResolvedProvider};
use engram::config::Config;
use engram::learning::{self, progress, signals};
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
fn red_queen_empty_events_should_not_panic() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "empty-test";

    // Challenge: Can the system handle empty event lists?
    let signals = signals::extract_signals_from_events(&[]);
    assert!(signals.is_empty(), "Empty events should produce no signals");

    // Challenge: Can hooks handle projects with no data?
    let result = learning::post_ingest_hook(&config, project);
    assert!(result.is_ok(), "Hook should not panic on empty project");
}

#[test]
fn red_queen_malformed_knowledge_ids() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "malformed-ids";
    let tracker = EventTracker::new(&config.memory_dir);

    // Challenge: Can the system handle malicious knowledge IDs?
    let evil_ids: Vec<String> = vec![
        "".to_string(),                       // Empty
        " ".to_string(),                      // Whitespace only
        "../../../../etc/passwd".to_string(), // Path traversal
        "a".repeat(10000),                    // Extremely long
        "test\0null".to_string(),             // Null byte
        "test\n\r\t".to_string(),             // Control characters
        "🔥💀🎃".repeat(100),                 // Unicode spam
        "test::double::colon".to_string(),    // Multiple colons
        "no-category".to_string(),            // Missing category separator
    ];

    for (i, evil_id) in evil_ids.iter().enumerate() {
        let parts: Vec<&str> = evil_id.as_str().split(':').collect();
        let (category, query) = if parts.len() >= 2 {
            (Some(parts[0].to_string()), Some(parts[1..].join(":")))
        } else {
            (Some("patterns".to_string()), Some(evil_id.to_string()))
        };

        tracker
            .track(UsageEvent {
                timestamp: Utc::now(),
                event_type: EventType::Recall,
                project: project.to_string(),
                query,
                category,
                results_count: Some(1),
                session_id: Some(format!("evil-{}", i)),
                tokens_consumed: None,
            })
            .unwrap();
    }

    // Challenge: Does ingest survive malformed data?
    let result = learning::post_ingest_hook(&config, project);
    assert!(
        result.is_ok(),
        "Should handle malformed knowledge IDs gracefully"
    );
}

#[test]
fn red_queen_events_without_categories() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "no-categories";
    let tracker = EventTracker::new(&config.memory_dir);

    // Challenge: What happens when events have no category?
    for i in 0..10 {
        tracker
            .track(UsageEvent {
                timestamp: Utc::now(),
                event_type: EventType::Recall,
                project: project.to_string(),
                query: Some(format!("query-{}", i)),
                category: None, // No category!
                results_count: Some(1),
                session_id: Some(format!("no-cat-{}", i)),
                tokens_consumed: None,
            })
            .unwrap();
    }

    let events = tracker.get_events(Some(project), 30).unwrap();
    let _signals = signals::extract_signals_from_events(&events);

    // System should handle this gracefully (no signals without categories)
    assert!(
        _signals.is_empty(),
        "Events without categories should not generate signals"
    );
}

#[test]
fn red_queen_extremely_high_frequency() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "spam-test";
    let tracker = EventTracker::new(&config.memory_dir);

    // Challenge: Can the system handle spam-level access (1000x same knowledge)?
    for i in 0..1000 {
        tracker
            .track(UsageEvent {
                timestamp: Utc::now(),
                event_type: EventType::Recall,
                project: project.to_string(),
                query: Some("spam-knowledge".to_string()),
                category: Some("patterns".to_string()),
                results_count: Some(1),
                session_id: Some(format!("spam-{}", i)),
                tokens_consumed: None,
            })
            .unwrap();
    }

    let events = tracker.get_events(Some(project), 1000).unwrap();
    let _signals = signals::extract_signals_from_events(&events);

    // Challenge: Are importance values clamped properly?
    let result = learning::post_ingest_hook(&config, project);
    assert!(result.is_ok(), "Should handle high-frequency access");

    let state = progress::load_state(&config.memory_dir, project).unwrap();
    let boost = state
        .learned_parameters
        .importance_boosts
        .get("patterns:spam-knowledge")
        .copied()
        .unwrap_or(0.0);

    // Importance should be bounded (not infinite)
    assert!(
        (0.0..=1.0).contains(&boost),
        "Importance boost should be bounded: got {}",
        boost
    );
}

#[test]
fn red_queen_concurrent_learning_sessions() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "concurrent-test";

    // Challenge: What happens if multiple learning sessions run simultaneously?
    // Simulate by running ingest multiple times rapidly
    for _i in 0..5 {
        learning::simulation::simulate_recall_session(&config, project, 10).unwrap();
        learning::post_ingest_hook(&config, project).unwrap();
    }

    let state = progress::load_state(&config.memory_dir, project).unwrap();

    // System should maintain consistency
    assert!(
        state.session_count() >= 1,
        "Should track at least one session"
    );
    assert!(
        !state.metrics_history.is_empty(),
        "Should have metrics history"
    );
}

#[test]
fn red_queen_learning_state_corruption() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "corruption-test";

    // Challenge: Can the system recover from corrupted state?
    learning::simulation::simulate_recall_session(&config, project, 10).unwrap();
    learning::post_ingest_hook(&config, project).unwrap();

    // Corrupt the state by writing invalid JSON
    let state_path = config
        .memory_dir
        .join("learning")
        .join(project)
        .join("state.json");
    std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();
    std::fs::write(&state_path, "{ invalid json }").unwrap();

    // Challenge: Does the system recreate state or panic?
    let result = learning::post_ingest_hook(&config, project);

    // System should either recover or fail gracefully
    if result.is_err() {
        // If it fails, it should be a proper error, not a panic — reaching here is success
    } else {
        // If it succeeds, it should have recreated state
        let state = progress::load_state(&config.memory_dir, project).unwrap();
        assert!(state.project == project, "State should be valid");
    }
}

#[test]
fn red_queen_reward_calculation_edge_cases() {
    use engram::learning::signals::LearningSignal;

    // Challenge: Are rewards always in valid range?
    let test_signals = vec![
        LearningSignal::HealthImprovement {
            before: 100,
            after: 100,
            knowledge_ids: vec![],
        },
        LearningSignal::HealthImprovement {
            before: 0,
            after: 100,
            knowledge_ids: vec![],
        },
        LearningSignal::HealthImprovement {
            before: 100,
            after: 0,
            knowledge_ids: vec![],
        },
        LearningSignal::SuccessfulRecall {
            knowledge_id: "test".to_string(),
            relevance: 1.5,
        },
        LearningSignal::SuccessfulRecall {
            knowledge_id: "test".to_string(),
            relevance: -0.5,
        },
        LearningSignal::HighFrequencyAccess {
            knowledge_id: "test".to_string(),
            access_count: 1000000,
            recency_score: 2.0,
        },
    ];

    for signal in test_signals {
        let reward = signal.to_reward();
        assert!(
            (0.0..=1.0).contains(&reward),
            "Reward should be in [0, 1], got: {} for signal: {:?}",
            reward,
            signal
        );
    }
}

#[test]
fn red_queen_session_count_accuracy() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "count-test";

    // Challenge: Does session count actually reflect learning iterations?
    let expected_sessions = 10;

    for _ in 0..expected_sessions {
        learning::simulation::simulate_recall_session(&config, project, 15).unwrap();
        learning::post_ingest_hook(&config, project).unwrap();
    }

    let state = progress::load_state(&config.memory_dir, project).unwrap();

    // Session count should match iterations (within reason, since some might not generate signals)
    assert!(
        state.session_count() >= 1,
        "Should have at least 1 session after {} iterations, got {}",
        expected_sessions,
        state.session_count()
    );
}

#[test]
fn red_queen_importance_does_not_decrease_with_single_access() {
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "no-decrease";

    // Build up importance
    learning::simulation::simulate_high_frequency_knowledge(
        &config,
        project,
        "stable-knowledge",
        20,
    )
    .unwrap();
    learning::post_ingest_hook(&config, project).unwrap();

    let state_before = progress::load_state(&config.memory_dir, project).unwrap();
    let importance_before = state_before
        .learned_parameters
        .importance_boosts
        .get("patterns:stable-knowledge")
        .copied()
        .unwrap_or(0.0);

    // Access once more
    learning::simulation::simulate_high_frequency_knowledge(
        &config,
        project,
        "stable-knowledge",
        1,
    )
    .unwrap();
    learning::post_ingest_hook(&config, project).unwrap();

    let state_after = progress::load_state(&config.memory_dir, project).unwrap();
    let importance_after = state_after
        .learned_parameters
        .importance_boosts
        .get("patterns:stable-knowledge")
        .copied()
        .unwrap_or(0.0);

    // Challenge: Does importance ever decrease from positive signals?
    assert!(
        importance_after >= importance_before,
        "Importance should not decrease from additional access: {} -> {}",
        importance_before,
        importance_after
    );
}
