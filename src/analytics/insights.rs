use crate::analytics::metrics::{compute_knowledge_scores, get_stale_knowledge, get_top_knowledge};
use crate::analytics::tracker::UsageEvent;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Insights {
    pub total_events: usize,
    pub unique_projects: usize,
    pub most_active_project: Option<String>,
    pub most_common_event: String,
    pub top_knowledge: Vec<String>,
    pub stale_knowledge: Vec<String>,
    pub usage_trend: String,
}

pub fn generate_insights(events: &[UsageEvent]) -> Insights {
    if events.is_empty() {
        return Insights {
            total_events: 0,
            unique_projects: 0,
            most_active_project: None,
            most_common_event: "none".to_string(),
            top_knowledge: Vec::new(),
            stale_knowledge: Vec::new(),
            usage_trend: "no data".to_string(),
        };
    }

    // Project activity
    let mut project_counts: HashMap<String, usize> = HashMap::new();
    for event in events {
        *project_counts.entry(event.project.clone()).or_insert(0) += 1;
    }

    let unique_projects = project_counts.len();
    let most_active_project = project_counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(proj, _)| proj.clone());

    // Event type distribution
    let mut event_counts: HashMap<String, usize> = HashMap::new();
    for event in events {
        let event_name = format!("{:?}", event.event_type);
        *event_counts.entry(event_name).or_insert(0) += 1;
    }

    let most_common_event = event_counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(event, _)| event.clone())
        .unwrap_or_else(|| "unknown".to_string());

    // Knowledge scores
    let scores = compute_knowledge_scores(events);
    let top_k = get_top_knowledge(&scores, 5);
    let stale_k = get_stale_knowledge(&scores, 0.1);

    let top_knowledge = top_k
        .iter()
        .map(|s| {
            format!(
                "{} ({}x, {:.1}%)",
                s.label,
                s.access_count,
                s.importance * 100.0
            )
        })
        .collect();

    let stale_knowledge = stale_k
        .iter()
        .map(|s| format!("{} (last: {:.1}%)", s.label, s.recency_score * 100.0))
        .collect();

    // Usage trend (simple: compare first half vs second half)
    let mid = events.len() / 2;
    let first_half = mid;
    let second_half = events.len() - mid;

    let trend = if second_half > first_half {
        format!(
            "increasing (+{:.0}%)",
            ((second_half - first_half) as f32 / first_half as f32) * 100.0
        )
    } else if second_half < first_half {
        format!(
            "decreasing (-{:.0}%)",
            ((first_half - second_half) as f32 / first_half as f32) * 100.0
        )
    } else {
        "stable".to_string()
    };

    Insights {
        total_events: events.len(),
        unique_projects,
        most_active_project,
        most_common_event,
        top_knowledge,
        stale_knowledge,
        usage_trend: trend,
    }
}

pub fn format_insights(insights: &Insights) -> String {
    let mut output = String::new();

    output.push_str("📊 Usage Insights\n");
    output.push_str("============================================================\n\n");

    output.push_str(&format!("Total events: {}\n", insights.total_events));
    output.push_str(&format!("Unique projects: {}\n", insights.unique_projects));

    if let Some(proj) = &insights.most_active_project {
        output.push_str(&format!("Most active project: {}\n", proj));
    }

    output.push_str(&format!(
        "Most common action: {}\n",
        insights.most_common_event
    ));
    output.push_str(&format!("Usage trend: {}\n", insights.usage_trend));

    if !insights.top_knowledge.is_empty() {
        output.push_str("\n🔥 Top Knowledge (by usage):\n");
        for (i, k) in insights.top_knowledge.iter().enumerate() {
            output.push_str(&format!("  {}. {}\n", i + 1, k));
        }
    }

    if !insights.stale_knowledge.is_empty() {
        output.push_str("\n🕸️  Stale Knowledge (rarely accessed):\n");
        for (i, k) in insights.stale_knowledge.iter().enumerate() {
            output.push_str(&format!("  {}. {}\n", i + 1, k));
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::tracker::EventType;

    #[test]
    fn test_empty_insights() {
        let insights = generate_insights(&[]);
        assert_eq!(insights.total_events, 0);
        assert_eq!(insights.unique_projects, 0);
    }

    #[test]
    fn test_insights_with_data() {
        let events = vec![
            UsageEvent {
                timestamp: chrono::Utc::now(),
                event_type: EventType::Recall,
                project: "proj-a".to_string(),
                query: None,
                category: None,
                results_count: None,
                session_id: None,
            },
            UsageEvent {
                timestamp: chrono::Utc::now(),
                event_type: EventType::Search,
                project: "proj-a".to_string(),
                query: Some("test".to_string()),
                category: Some("patterns".to_string()),
                results_count: None,
                session_id: None,
            },
        ];

        let insights = generate_insights(&events);
        assert_eq!(insights.total_events, 2);
        assert_eq!(insights.unique_projects, 1);
        assert_eq!(insights.most_active_project, Some("proj-a".to_string()));
    }
}
