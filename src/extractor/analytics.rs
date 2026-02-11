use std::collections::HashMap;

use crate::config::Config;
use crate::error::Result;
use crate::parser::conversation::Conversation;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionAnalytics {
    pub session_id: String,
    pub project: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub turn_count: usize,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub tool_usage: HashMap<String, usize>,
}

/// Extract analytics from a conversation
pub fn extract_session_analytics(conv: &Conversation) -> SessionAnalytics {
    let mut tool_usage = HashMap::new();
    for turn in &conv.turns {
        for tool in &turn.tool_interactions {
            *tool_usage.entry(tool.tool_name.clone()).or_insert(0) += 1;
        }
    }

    SessionAnalytics {
        session_id: conv.session_id.clone(),
        project: conv.project.clone(),
        start_time: conv.start_time.clone(),
        end_time: conv.end_time.clone(),
        turn_count: conv.turns.len(),
        total_input_tokens: conv.total_input_tokens,
        total_output_tokens: conv.total_output_tokens,
        tool_usage,
    }
}

/// Aggregate and write analytics from multiple sessions
pub fn write_aggregated_analytics(config: &Config, sessions: &[SessionAnalytics]) -> Result<()> {
    let analytics_dir = config.memory_dir.join("analytics");
    std::fs::create_dir_all(&analytics_dir)?;

    // Usage: tool usage counts per project
    let mut project_tool_usage: HashMap<String, HashMap<String, usize>> = HashMap::new();
    let mut global_tool_usage: HashMap<String, usize> = HashMap::new();

    for session in sessions {
        let project_tools = project_tool_usage
            .entry(session.project.clone())
            .or_default();
        for (tool, count) in &session.tool_usage {
            *project_tools.entry(tool.clone()).or_insert(0) += count;
            *global_tool_usage.entry(tool.clone()).or_insert(0) += count;
        }
    }

    let usage = serde_json::json!({
        "global": global_tool_usage,
        "per_project": project_tool_usage,
    });

    // Merge with existing analytics if present
    let usage_path = analytics_dir.join("usage.json");
    let merged_usage = if usage_path.exists() {
        let existing: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&usage_path)?).unwrap_or_default();
        merge_usage(&existing, &usage)
    } else {
        usage
    };

    std::fs::write(&usage_path, serde_json::to_string_pretty(&merged_usage)?)?;

    // Activity: project timeline
    let mut activity: Vec<serde_json::Value> = Vec::new();
    for session in sessions {
        activity.push(serde_json::json!({
            "session_id": session.session_id,
            "project": session.project,
            "start_time": session.start_time,
            "end_time": session.end_time,
            "turns": session.turn_count,
            "input_tokens": session.total_input_tokens,
            "output_tokens": session.total_output_tokens,
        }));
    }

    // Merge with existing activity
    let activity_path = analytics_dir.join("activity.json");
    let mut existing_activity: Vec<serde_json::Value> = if activity_path.exists() {
        serde_json::from_str(&std::fs::read_to_string(&activity_path)?).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Deduplicate by session_id
    let existing_ids: std::collections::HashSet<String> = existing_activity
        .iter()
        .filter_map(|v| v.get("session_id").and_then(|s| s.as_str()).map(String::from))
        .collect();

    for a in activity {
        let id = a
            .get("session_id")
            .and_then(|s| s.as_str())
            .unwrap_or("");
        if !existing_ids.contains(id) {
            existing_activity.push(a);
        }
    }

    std::fs::write(
        &activity_path,
        serde_json::to_string_pretty(&existing_activity)?,
    )?;

    Ok(())
}

fn merge_usage(existing: &serde_json::Value, new: &serde_json::Value) -> serde_json::Value {
    let mut merged = existing.clone();

    // Merge global counts
    if let (Some(existing_global), Some(new_global)) = (
        existing.get("global").and_then(|v| v.as_object()),
        new.get("global").and_then(|v| v.as_object()),
    ) {
        let mut global = existing_global.clone();
        for (key, new_val) in new_global {
            let existing_count = global
                .get(key)
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let new_count = new_val.as_u64().unwrap_or(0);
            global.insert(key.clone(), serde_json::json!(existing_count + new_count));
        }
        merged["global"] = serde_json::Value::Object(global);
    }

    // Merge per-project counts
    if let (Some(existing_pp), Some(new_pp)) = (
        existing.get("per_project").and_then(|v| v.as_object()),
        new.get("per_project").and_then(|v| v.as_object()),
    ) {
        let mut pp = existing_pp.clone();
        for (project, new_tools) in new_pp {
            if let Some(new_tools) = new_tools.as_object() {
                let existing_tools = pp
                    .entry(project.clone())
                    .or_insert_with(|| serde_json::json!({}));
                if let Some(existing_tools) = existing_tools.as_object_mut() {
                    for (tool, count) in new_tools {
                        let old = existing_tools
                            .get(tool)
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let new_count = count.as_u64().unwrap_or(0);
                        existing_tools.insert(tool.clone(), serde_json::json!(old + new_count));
                    }
                }
            }
        }
        merged["per_project"] = serde_json::Value::Object(pp);
    }

    merged
}
