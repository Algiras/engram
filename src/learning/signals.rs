use crate::analytics::tracker::{EventType, UsageEvent};
use crate::health::HealthReport;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Learning signals extracted from system usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LearningSignal {
    /// Health improvement detected
    HealthImprovement {
        before: u8,
        after: u8,
        knowledge_ids: Vec<String>,
    },

    /// Knowledge successfully recalled
    SuccessfulRecall {
        knowledge_id: String,
        relevance: f32,
    },

    /// Consolidation accepted by user
    ConsolidationAccepted {
        merged_count: usize,
        similarity_threshold: f32,
    },

    /// Knowledge accessed frequently
    HighFrequencyAccess {
        knowledge_id: String,
        access_count: usize,
        recency_score: f32,
    },

    /// Co-occurrence pattern detected
    CoOccurrence {
        knowledge_ids: Vec<String>,
        co_access_count: usize,
    },
}

impl LearningSignal {
    /// Normalize signal to a reward value between 0.0 and 1.0
    pub fn to_reward(&self) -> f32 {
        match self {
            LearningSignal::HealthImprovement { before, after, .. } => {
                let improvement = (*after as f32 - *before as f32) / 100.0;
                improvement.max(0.0).min(1.0)
            }
            LearningSignal::SuccessfulRecall { relevance, .. } => relevance.max(0.0).min(1.0),
            LearningSignal::ConsolidationAccepted {
                merged_count,
                similarity_threshold,
            } => {
                // More merges at higher thresholds = better consolidation
                let count_score = (*merged_count as f32 / 10.0).min(0.5);
                let threshold_score = *similarity_threshold * 0.5;
                (count_score + threshold_score).min(1.0)
            }
            LearningSignal::HighFrequencyAccess {
                access_count,
                recency_score,
                ..
            } => {
                // Combine frequency and recency
                let freq_score = (*access_count as f32 / 50.0).min(0.6);
                let recency_contrib = recency_score * 0.4;
                (freq_score + recency_contrib).min(1.0)
            }
            LearningSignal::CoOccurrence {
                co_access_count, ..
            } => {
                // Stronger co-occurrence = higher reward
                (*co_access_count as f32 / 20.0).min(1.0)
            }
        }
    }

    /// Get knowledge IDs affected by this signal
    pub fn knowledge_ids(&self) -> Vec<String> {
        match self {
            LearningSignal::HealthImprovement { knowledge_ids, .. } => knowledge_ids.clone(),
            LearningSignal::SuccessfulRecall { knowledge_id, .. } => vec![knowledge_id.clone()],
            LearningSignal::HighFrequencyAccess { knowledge_id, .. } => vec![knowledge_id.clone()],
            LearningSignal::CoOccurrence { knowledge_ids, .. } => knowledge_ids.clone(),
            LearningSignal::ConsolidationAccepted { .. } => Vec::new(),
        }
    }
}

/// Extract learning signals from analytics events
pub fn extract_signals_from_events(events: &[UsageEvent]) -> Vec<LearningSignal> {
    let mut signals = Vec::new();
    let mut access_counts: HashMap<String, usize> = HashMap::new();

    // Track access patterns
    for event in events {
        match event.event_type {
            EventType::Recall | EventType::Search | EventType::Lookup => {
                // Create a knowledge ID from category + query
                if let Some(category) = &event.category {
                    let id = format!(
                        "{}:{}",
                        category,
                        event.query.as_deref().unwrap_or("unknown")
                    );
                    *access_counts.entry(id).or_insert(0) += 1;
                }
            }
            EventType::Add => {
                // New knowledge added - weak positive signal
                if let Some(category) = &event.category {
                    let id = format!(
                        "{}:{}",
                        category,
                        event.query.as_deref().unwrap_or("unknown")
                    );
                    signals.push(LearningSignal::SuccessfulRecall {
                        knowledge_id: id,
                        relevance: 0.3,
                    });
                }
            }
            _ => {}
        }
    }

    // Convert access counts to high-frequency signals
    for (id, count) in access_counts {
        if count >= 3 {
            // Threshold for "high frequency"
            signals.push(LearningSignal::HighFrequencyAccess {
                knowledge_id: id,
                access_count: count,
                recency_score: 0.8, // Simplified - would compute from event timestamps
            });
        }
    }

    signals
}

/// Extract health improvement signal by comparing two health reports
pub fn extract_health_improvement_signal(
    before: &HealthReport,
    after: &HealthReport,
    knowledge_ids: Vec<String>,
) -> Option<LearningSignal> {
    let before_score = before.score;
    let after_score = after.score;

    if after_score > before_score {
        Some(LearningSignal::HealthImprovement {
            before: before_score,
            after: after_score,
            knowledge_ids,
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signal_to_reward() {
        let signal = LearningSignal::HealthImprovement {
            before: 70,
            after: 90,
            knowledge_ids: vec!["test".to_string()],
        };
        assert_eq!(signal.to_reward(), 0.2);

        let signal = LearningSignal::SuccessfulRecall {
            knowledge_id: "test".to_string(),
            relevance: 0.85,
        };
        assert_eq!(signal.to_reward(), 0.85);
    }

    #[test]
    fn test_extract_signals_from_events() {
        use chrono::Utc;

        let events = vec![
            UsageEvent {
                timestamp: Utc::now(),
                event_type: EventType::Recall,
                project: "test".to_string(),
                query: Some("pattern1".to_string()),
                category: Some("patterns".to_string()),
                results_count: None,
                session_id: None,
            },
            UsageEvent {
                timestamp: Utc::now(),
                event_type: EventType::Recall,
                project: "test".to_string(),
                query: Some("pattern1".to_string()),
                category: Some("patterns".to_string()),
                results_count: None,
                session_id: None,
            },
            UsageEvent {
                timestamp: Utc::now(),
                event_type: EventType::Recall,
                project: "test".to_string(),
                query: Some("pattern1".to_string()),
                category: Some("patterns".to_string()),
                results_count: None,
                session_id: None,
            },
        ];

        let signals = extract_signals_from_events(&events);

        // Should detect high-frequency access for patterns:pattern1
        assert!(signals.iter().any(|s| matches!(
            s,
            LearningSignal::HighFrequencyAccess { knowledge_id, .. }
            if knowledge_id == "patterns:pattern1"
        )));
    }
}
