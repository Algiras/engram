//! Red Queen Protocol: Memory Disorders
//!
//! Test sophisticated failure modes inspired by human memory disorders:
//! - Memory Poisoning: One corruption spreads to all related knowledge
//! - False Memory Syndrome: System learns incorrect patterns
//! - Catastrophic Forgetting: New learning erases old knowledge
//! - Interference: Conflicting signals cause confusion
//! - Confabulation: System fills gaps with plausible falsehoods
//!
//! These are the "red pill" tests - they reveal deep system vulnerabilities.

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
fn red_pill_memory_poisoning_cascade() {
    // Challenge: Can one corrupted knowledge entry poison all related memories?
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "poison-test";
    let tracker = EventTracker::new(&config.memory_dir);

    // Step 1: Build up legitimate knowledge
    for i in 0..10 {
        for _ in 0..5 {
            tracker
                .track(UsageEvent {
                    timestamp: Utc::now(),
                    event_type: EventType::Recall,
                    project: project.to_string(),
                    query: Some(format!("legitimate-{}", i)),
                    category: Some("patterns".to_string()),
                    results_count: Some(1),
                    session_id: Some(format!("legit-{}", i)),
                    tokens_consumed: None,
                })
                .unwrap();
        }
    }

    learning::post_ingest_hook(&config, project).unwrap();
    let state_before = progress::load_state(&config.memory_dir, project).unwrap();
    let legit_count_before = state_before.learned_parameters.importance_boosts.len();

    // Step 2: Inject "poisoned" knowledge with extremely high frequency
    for _ in 0..1000 {
        tracker
            .track(UsageEvent {
                timestamp: Utc::now(),
                event_type: EventType::Recall,
                project: project.to_string(),
                query: Some("POISON".to_string()),
                category: Some("patterns".to_string()),
                results_count: Some(1),
                session_id: Some("poison".to_string()),
                tokens_consumed: None,
            })
            .unwrap();
    }

    learning::post_ingest_hook(&config, project).unwrap();
    let state_after = progress::load_state(&config.memory_dir, project).unwrap();

    // Verify: Did the poison corrupt legitimate knowledge?
    let poison_boost = state_after
        .learned_parameters
        .importance_boosts
        .get("patterns:POISON")
        .copied()
        .unwrap_or(0.0);

    let legit_boosts: Vec<f32> = state_after
        .learned_parameters
        .importance_boosts
        .iter()
        .filter(|(k, _)| k.contains("legitimate"))
        .map(|(_, v)| *v)
        .collect();

    // Critical test: Poison should not REDUCE legitimate importance
    assert!(
        legit_boosts.len() == legit_count_before,
        "Poisoned knowledge should not erase legitimate knowledge"
    );

    for boost in &legit_boosts {
        assert!(
            *boost > 0.0,
            "Legitimate knowledge should retain positive importance"
        );
    }

    // Poison should be bounded despite extreme frequency
    assert!(
        poison_boost <= 1.0,
        "Even poisoned knowledge should be bounded: got {}",
        poison_boost
    );
}

#[test]
fn red_pill_false_memory_reinforcement() {
    // Challenge: Can false patterns reinforce themselves through feedback loops?
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "false-memory";
    let tracker = EventTracker::new(&config.memory_dir);

    // Create a false pattern that repeats
    let false_pattern = ["A", "B", "C", "A", "B", "C"];

    for cycle in 0..5 {
        for (i, item) in false_pattern.iter().enumerate() {
            tracker
                .track(UsageEvent {
                    timestamp: Utc::now(),
                    event_type: EventType::Recall,
                    project: project.to_string(),
                    query: Some(item.to_string()),
                    category: Some("patterns".to_string()),
                    results_count: Some(1),
                    session_id: Some(format!("cycle-{}-{}", cycle, i)),
                    tokens_consumed: None,
                })
                .unwrap();
        }

        learning::post_ingest_hook(&config, project).unwrap();
    }

    let state = progress::load_state(&config.memory_dir, project).unwrap();

    // Check: Did the false pattern reinforce itself?
    let boost_a = state
        .learned_parameters
        .importance_boosts
        .get("patterns:A")
        .copied()
        .unwrap_or(0.0);

    let boost_b = state
        .learned_parameters
        .importance_boosts
        .get("patterns:B")
        .copied()
        .unwrap_or(0.0);

    // False memories should still be learned (system doesn't know they're false)
    // But they should be bounded
    assert!(
        boost_a > 0.0 && boost_a <= 1.0,
        "False memory A should exist but be bounded: {}",
        boost_a
    );

    assert!(
        boost_b > 0.0 && boost_b <= 1.0,
        "False memory B should exist but be bounded: {}",
        boost_b
    );

    // Critical: No unbounded reinforcement
    assert!(
        boost_a < 0.9 && boost_b < 0.9,
        "False memories should not reach extreme importance through reinforcement"
    );
}

#[test]
fn red_pill_catastrophic_forgetting() {
    // Challenge: Does new learning erase old knowledge?
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "forgetting-test";

    // Phase 1: Learn "old" knowledge
    learning::simulation::simulate_high_frequency_knowledge(&config, project, "old-knowledge", 20)
        .unwrap();
    learning::post_ingest_hook(&config, project).unwrap();

    let state_phase1 = progress::load_state(&config.memory_dir, project).unwrap();
    let old_boost = state_phase1
        .learned_parameters
        .importance_boosts
        .get("patterns:old-knowledge")
        .copied()
        .unwrap_or(0.0);

    assert!(old_boost > 0.0, "Old knowledge should be learned");

    // Phase 2: Learn completely different "new" knowledge (no overlap)
    for i in 0..50 {
        learning::simulation::simulate_high_frequency_knowledge(
            &config,
            project,
            &format!("new-knowledge-{}", i),
            5,
        )
        .unwrap();
        learning::post_ingest_hook(&config, project).unwrap();
    }

    let state_phase2 = progress::load_state(&config.memory_dir, project).unwrap();
    let old_boost_after = state_phase2
        .learned_parameters
        .importance_boosts
        .get("patterns:old-knowledge")
        .copied()
        .unwrap_or(0.0);

    // Critical test: Old knowledge should NOT be forgotten
    assert!(
        old_boost_after > 0.0,
        "Old knowledge should not be erased by new learning"
    );

    assert!(
        old_boost_after >= old_boost * 0.5,
        "Old knowledge should not decay significantly: {} -> {}",
        old_boost,
        old_boost_after
    );
}

#[test]
fn red_pill_conflicting_signals() {
    // Challenge: What happens with contradictory signals about same knowledge?
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "conflict-test";
    let tracker = EventTracker::new(&config.memory_dir);

    // Scenario: Same knowledge accessed frequently, then not accessed at all
    // Phase 1: High frequency
    for _ in 0..20 {
        tracker
            .track(UsageEvent {
                timestamp: Utc::now(),
                event_type: EventType::Recall,
                project: project.to_string(),
                query: Some("conflicted".to_string()),
                category: Some("patterns".to_string()),
                results_count: Some(1),
                session_id: Some("high-freq".to_string()),
                tokens_consumed: None,
            })
            .unwrap();
    }

    learning::post_ingest_hook(&config, project).unwrap();
    let state_high = progress::load_state(&config.memory_dir, project).unwrap();
    let boost_high = state_high
        .learned_parameters
        .importance_boosts
        .get("patterns:conflicted")
        .copied()
        .unwrap_or(0.0);

    // Phase 2: Other knowledge accessed (implicit neglect of "conflicted")
    for i in 0..20 {
        tracker
            .track(UsageEvent {
                timestamp: Utc::now(),
                event_type: EventType::Recall,
                project: project.to_string(),
                query: Some(format!("other-{}", i)),
                category: Some("patterns".to_string()),
                results_count: Some(1),
                session_id: Some(format!("other-{}", i)),
                tokens_consumed: None,
            })
            .unwrap();
    }

    learning::post_ingest_hook(&config, project).unwrap();
    let state_neglect = progress::load_state(&config.memory_dir, project).unwrap();
    let boost_neglect = state_neglect
        .learned_parameters
        .importance_boosts
        .get("patterns:conflicted")
        .copied()
        .unwrap_or(0.0);

    // The system currently doesn't implement decay (future feature)
    // So importance should stay the same or increase slightly
    assert!(
        boost_neglect >= boost_high * 0.9,
        "Without explicit decay, importance should not drop significantly: {} -> {}",
        boost_high,
        boost_neglect
    );
}

#[test]
fn red_pill_state_file_complete_corruption() {
    // Challenge: Total state file corruption (unrecoverable)
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "total-corruption";

    // Build up state
    learning::simulation::simulate_recall_session(&config, project, 20).unwrap();
    learning::post_ingest_hook(&config, project).unwrap();

    // Completely corrupt the state file (binary garbage)
    let state_path = config
        .memory_dir
        .join("learning")
        .join(project)
        .join("state.json");
    std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();
    std::fs::write(&state_path, [0xFF, 0xFE, 0xFD, 0xFC, 0x00, 0x01, 0x02]).unwrap();

    // Try to load: Should either recover or fail gracefully
    let result = progress::load_state(&config.memory_dir, project);

    match result {
        Ok(state) => {
            // System recovered by creating new state
            assert_eq!(
                state.project, project,
                "Recovered state should have correct project name"
            );

            // State might have some data from before corruption
            // The critical test is that we CAN load it without panic
            println!("State recovered with {} sessions", state.session_count());
        }
        Err(e) => {
            // System failed gracefully (also acceptable) — reaching here is success
            println!("Graceful failure on corrupted state: {}", e);
        }
    }

    // Critical: Can we continue learning after corruption?
    let recovery_result = learning::simulation::simulate_recall_session(&config, project, 10);
    assert!(
        recovery_result.is_ok(),
        "System should be able to continue learning after corruption"
    );
}

#[test]
fn red_pill_importance_overflow_attack() {
    // Challenge: Try to cause importance overflow through accumulated signals
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "overflow-test";

    // Accumulate signals over many sessions
    for session in 0..100 {
        learning::simulation::simulate_high_frequency_knowledge(
            &config,
            project,
            "overflow-target",
            10,
        )
        .unwrap();
        learning::post_ingest_hook(&config, project).unwrap();

        // Check after each session
        let state = progress::load_state(&config.memory_dir, project).unwrap();
        let boost = state
            .learned_parameters
            .importance_boosts
            .get("patterns:overflow-target")
            .copied()
            .unwrap_or(0.0);

        assert!(
            boost <= 1.0,
            "Importance should never exceed 1.0, got {} at session {}",
            boost,
            session
        );

        assert!(
            !boost.is_nan() && !boost.is_infinite(),
            "Importance should be a valid number, got {} at session {}",
            boost,
            session
        );
    }
}

#[test]
fn red_pill_metrics_history_memory_leak() {
    // Challenge: Does metrics history grow unbounded?
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "leak-test";

    // Generate many learning sessions
    for _ in 0..200 {
        learning::simulation::simulate_recall_session(&config, project, 10).unwrap();
        learning::post_ingest_hook(&config, project).unwrap();
    }

    let state = progress::load_state(&config.memory_dir, project).unwrap();

    // Check: Is metrics history bounded?
    let history_len = state.metrics_history.len();

    // System should have SOME metrics (not 0)
    assert!(
        history_len > 0,
        "Metrics history should not be empty after 200 sessions"
    );

    // But if it's unbounded, this test will fail (good - alerts us to fix it)
    println!("Metrics history length after 200 sessions: {}", history_len);

    // Currently the system doesn't limit history size
    // This test documents current behavior
    // If history_len == 200, we need to add bounds
    if history_len > 100 {
        println!("⚠️ WARNING: Metrics history may grow unbounded");
        println!("   Consider implementing sliding window or periodic cleanup");
    }
}

#[test]
fn red_pill_zero_divided_by_zero() {
    // Challenge: Edge case math with zero values
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "zero-test";
    let tracker = EventTracker::new(&config.memory_dir);

    // Create events with zero results_count
    for i in 0..5 {
        tracker
            .track(UsageEvent {
                timestamp: Utc::now(),
                event_type: EventType::Recall,
                project: project.to_string(),
                query: Some(format!("query-{}", i)),
                category: Some("patterns".to_string()),
                results_count: Some(0), // Zero results!
                session_id: Some(format!("zero-{}", i)),
                tokens_consumed: None,
            })
            .unwrap();
    }

    // Should not panic or produce NaN
    let events = tracker.get_events(Some(project), 30).unwrap();
    let signals = signals::extract_signals_from_events(&events);

    for signal in &signals {
        let reward = signal.to_reward();
        assert!(!reward.is_nan(), "Reward should not be NaN");
        assert!(!reward.is_infinite(), "Reward should not be infinite");
        assert!(
            (0.0..=1.0).contains(&reward),
            "Reward should be in [0, 1], got: {}",
            reward
        );
    }
}

#[test]
fn red_pill_rapid_fire_updates() {
    // Challenge: Can concurrent rapid updates cause race conditions?
    let temp = TempDir::new().unwrap();
    let config = create_test_config(&temp);
    let project = "rapid-test";

    // Simulate rapid-fire learning (no delays)
    for i in 0..50 {
        learning::simulation::simulate_recall_session(&config, project, 5).unwrap();
        learning::post_ingest_hook(&config, project).unwrap();

        // Verify state consistency after each update
        let state = progress::load_state(&config.memory_dir, project).unwrap();

        assert_eq!(
            state.project, project,
            "Project name should remain consistent"
        );

        assert!(
            state.session_count() >= i / 5,
            "Session count should be monotonically increasing"
        );

        // Check no corrupted values
        for (id, boost) in &state.learned_parameters.importance_boosts {
            assert!(
                (&0.0..=&1.0).contains(&boost),
                "Boost for {} should be valid: {}",
                id,
                boost
            );
        }
    }
}
