use crate::analytics::tracker::UsageEvent;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct KnowledgeScore {
    pub category: String,
    pub label: String,
    pub access_count: usize,
    pub last_accessed: Option<chrono::DateTime<chrono::Utc>>,
    pub recency_score: f32,   // 0.0-1.0 based on last access
    pub frequency_score: f32, // 0.0-1.0 based on access count
    pub importance: f32,      // Combined score 0.0-1.0
}

pub fn compute_knowledge_scores(events: &[UsageEvent]) -> HashMap<String, KnowledgeScore> {
    let mut scores: HashMap<String, KnowledgeScore> = HashMap::new();

    if events.is_empty() {
        return scores;
    }

    // Track access patterns per category
    for event in events {
        if let Some(category) = &event.category {
            let key = format!(
                "{}:{}",
                category,
                event.query.as_deref().unwrap_or("unknown")
            );

            scores
                .entry(key.clone())
                .and_modify(|score| {
                    score.access_count += 1;
                    if let Some(ts) = event.timestamp.into() {
                        if score.last_accessed.is_none() || score.last_accessed < Some(ts) {
                            score.last_accessed = Some(ts);
                        }
                    }
                })
                .or_insert_with(|| KnowledgeScore {
                    category: category.clone(),
                    label: event.query.clone().unwrap_or_default(),
                    access_count: 1,
                    last_accessed: Some(event.timestamp),
                    recency_score: 0.0,
                    frequency_score: 0.0,
                    importance: 0.0,
                });
        }
    }

    // Compute scores
    let now = chrono::Utc::now();
    let max_count = scores.values().map(|s| s.access_count).max().unwrap_or(1);

    for score in scores.values_mut() {
        // Frequency score (0.0-1.0)
        score.frequency_score = (score.access_count as f32) / (max_count as f32);

        // Recency score (exponential decay over 30 days)
        if let Some(last) = score.last_accessed {
            let days_ago: f32 = (now - last).num_days() as f32;
            score.recency_score = (-days_ago / 30.0_f32).exp();
        }

        // Combined importance (weighted average)
        score.importance = 0.6 * score.frequency_score + 0.4 * score.recency_score;
    }

    scores
}

pub fn get_top_knowledge(
    scores: &HashMap<String, KnowledgeScore>,
    limit: usize,
) -> Vec<KnowledgeScore> {
    let mut sorted: Vec<_> = scores.values().cloned().collect();
    sorted.sort_by(|a, b| {
        b.importance
            .partial_cmp(&a.importance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    sorted.into_iter().take(limit).collect()
}

pub fn get_stale_knowledge(
    scores: &HashMap<String, KnowledgeScore>,
    threshold: f32,
) -> Vec<KnowledgeScore> {
    scores
        .values()
        .filter(|s| s.recency_score < threshold)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::tracker::EventType;

    #[test]
    fn test_compute_scores() {
        let events = vec![
            UsageEvent {
                timestamp: chrono::Utc::now(),
                event_type: EventType::Recall,
                project: "test".to_string(),
                query: Some("auth".to_string()),
                category: Some("patterns".to_string()),
                results_count: None,
                session_id: None,
            },
            UsageEvent {
                timestamp: chrono::Utc::now(),
                event_type: EventType::Search,
                project: "test".to_string(),
                query: Some("auth".to_string()),
                category: Some("patterns".to_string()),
                results_count: None,
                session_id: None,
            },
        ];

        let scores = compute_knowledge_scores(&events);
        assert!(!scores.is_empty());

        let top = get_top_knowledge(&scores, 5);
        assert_eq!(top[0].access_count, 2);
    }
}
