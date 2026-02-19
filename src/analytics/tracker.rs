use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventType {
    Recall,
    Search,
    Lookup,
    Add,
    Promote,
    Forget,
    Export,
    GraphQuery,
    SemanticSearch,
    Context,
    Inject,
    Ingest,
    Ask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,
    pub project: String,
    pub query: Option<String>,
    pub category: Option<String>,
    pub results_count: Option<usize>,
    pub session_id: Option<String>,
    #[serde(default)]
    pub tokens_consumed: Option<u64>,
}

pub struct EventTracker {
    analytics_dir: PathBuf,
}

impl EventTracker {
    pub fn new(memory_dir: &Path) -> Self {
        let analytics_dir = memory_dir.join("analytics");
        Self { analytics_dir }
    }

    pub fn track(&self, event: UsageEvent) -> Result<()> {
        fs::create_dir_all(&self.analytics_dir)?;

        let date = event.timestamp.format("%Y-%m-%d").to_string();
        let log_file = self.analytics_dir.join(format!("{}.jsonl", date));

        let line = serde_json::to_string(&event)?;
        let mut content = String::new();

        if log_file.exists() {
            content = fs::read_to_string(&log_file)?;
        }

        content.push_str(&line);
        content.push('\n');

        fs::write(&log_file, content)?;
        Ok(())
    }

    pub fn get_events(&self, project: Option<&str>, since_days: u32) -> Result<Vec<UsageEvent>> {
        if !self.analytics_dir.exists() {
            return Ok(Vec::new());
        }

        let cutoff = Utc::now() - chrono::Duration::days(since_days as i64);
        let mut events = Vec::new();

        for entry in fs::read_dir(&self.analytics_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }

            let content = fs::read_to_string(&path)?;
            for line in content.lines() {
                if line.trim().is_empty() {
                    continue;
                }

                if let Ok(event) = serde_json::from_str::<UsageEvent>(line) {
                    if event.timestamp >= cutoff {
                        if let Some(proj) = project {
                            if event.project == proj {
                                events.push(event);
                            }
                        } else {
                            events.push(event);
                        }
                    }
                }
            }
        }

        events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(events)
    }

    pub fn clear_old_events(&self, days_to_keep: u32) -> Result<usize> {
        if !self.analytics_dir.exists() {
            return Ok(0);
        }

        let cutoff = Utc::now() - chrono::Duration::days(days_to_keep as i64);
        let mut removed = 0;

        for entry in fs::read_dir(&self.analytics_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }

            // Extract date from filename (YYYY-MM-DD.jsonl)
            if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                if let Ok(file_date) = chrono::NaiveDate::parse_from_str(filename, "%Y-%m-%d") {
                    let file_datetime = file_date
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_local_timezone(Utc)
                        .unwrap();

                    if file_datetime < cutoff {
                        fs::remove_file(&path)?;
                        removed += 1;
                    }
                }
            }
        }

        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_track_and_retrieve() {
        let temp = TempDir::new().unwrap();
        let tracker = EventTracker::new(temp.path());

        let event = UsageEvent {
            timestamp: Utc::now(),
            event_type: EventType::Recall,
            project: "test-project".to_string(),
            query: None,
            category: None,
            results_count: None,
            session_id: None,
            tokens_consumed: None,
        };

        tracker.track(event.clone()).unwrap();

        let events = tracker.get_events(Some("test-project"), 1).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, EventType::Recall));
    }

    #[test]
    fn test_project_filter() {
        let temp = TempDir::new().unwrap();
        let tracker = EventTracker::new(temp.path());

        for project in &["proj-a", "proj-b", "proj-a"] {
            tracker
                .track(UsageEvent {
                    timestamp: Utc::now(),
                    event_type: EventType::Search,
                    project: project.to_string(),
                    query: Some("test".to_string()),
                    category: None,
                    results_count: Some(5),
                    session_id: None,
                    tokens_consumed: None,
                })
                .unwrap();
        }

        let events = tracker.get_events(Some("proj-a"), 1).unwrap();
        assert_eq!(events.len(), 2);
    }
}
