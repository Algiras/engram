use crate::analytics::tracker::{EventTracker, EventType, UsageEvent};
use crate::config::Config;
use crate::error::Result;
use chrono::Utc;

/// Simulate a user session with recall events
pub fn simulate_recall_session(
    config: &Config,
    project: &str,
    recall_count: usize,
) -> Result<()> {
    let tracker = EventTracker::new(&config.memory_dir);

    for i in 0..recall_count {
        tracker.track(UsageEvent {
            timestamp: Utc::now(),
            event_type: EventType::Recall,
            project: project.to_string(),
            query: Some(format!("pattern-{}", i % 5)), // Simulate 5 different patterns
            category: Some("patterns".to_string()),
            results_count: Some(3),
            session_id: Some(format!("sim-{}", i)),
        })?;
    }

    Ok(())
}

/// Simulate mixed usage patterns (recall, search, add)
pub fn simulate_mixed_usage(
    config: &Config,
    project: &str,
    iterations: usize,
) -> Result<()> {
    let tracker = EventTracker::new(&config.memory_dir);

    for i in 0..iterations {
        let event_type = match i % 3 {
            0 => EventType::Recall,
            1 => EventType::Search,
            _ => EventType::Lookup,
        };

        tracker.track(UsageEvent {
            timestamp: Utc::now(),
            event_type,
            project: project.to_string(),
            query: Some(format!("query-{}", i % 10)),
            category: Some(if i % 2 == 0 { "patterns" } else { "decisions" }.to_string()),
            results_count: Some((i % 5) + 1),
            session_id: Some(format!("sim-{}", i)),
        })?;
    }

    Ok(())
}

/// Simulate high-frequency access to specific knowledge
pub fn simulate_high_frequency_knowledge(
    config: &Config,
    project: &str,
    knowledge_id: &str,
    access_count: usize,
) -> Result<()> {
    let tracker = EventTracker::new(&config.memory_dir);

    for i in 0..access_count {
        tracker.track(UsageEvent {
            timestamp: Utc::now(),
            event_type: EventType::Recall,
            project: project.to_string(),
            query: Some(knowledge_id.to_string()),
            category: Some("patterns".to_string()),
            results_count: Some(1),
            session_id: Some(format!("high-freq-{}", i)),
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::providers::{Provider, ResolvedProvider};
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
    fn test_simulate_recall_session() {
        let temp = TempDir::new().unwrap();
        let config = create_test_config(&temp);

        let result = simulate_recall_session(&config, "test-project", 10);
        assert!(result.is_ok());

        // Verify events were tracked
        let tracker = EventTracker::new(&config.memory_dir);
        let events = tracker.get_events(Some("test-project"), 1).unwrap();
        assert_eq!(events.len(), 10);
    }

    #[test]
    fn test_simulate_mixed_usage() {
        let temp = TempDir::new().unwrap();
        let config = create_test_config(&temp);

        let result = simulate_mixed_usage(&config, "test-project", 15);
        assert!(result.is_ok());

        // Verify events were tracked
        let tracker = EventTracker::new(&config.memory_dir);
        let events = tracker.get_events(Some("test-project"), 1).unwrap();
        assert_eq!(events.len(), 15);

        // Verify event types are mixed
        let has_recall = events.iter().any(|e| matches!(e.event_type, EventType::Recall));
        let has_search = events.iter().any(|e| matches!(e.event_type, EventType::Search));
        assert!(has_recall && has_search);
    }

    #[test]
    fn test_simulate_high_frequency_knowledge() {
        let temp = TempDir::new().unwrap();
        let config = create_test_config(&temp);

        let result = simulate_high_frequency_knowledge(&config, "test-project", "critical-pattern", 20);
        assert!(result.is_ok());

        // Verify all events have same query
        let tracker = EventTracker::new(&config.memory_dir);
        let events = tracker.get_events(Some("test-project"), 1).unwrap();
        assert_eq!(events.len(), 20);
        assert!(events.iter().all(|e| e.query.as_deref() == Some("critical-pattern")));
    }
}
