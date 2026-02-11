use crate::parser::conversation::Conversation;

/// Render a conversation to clean markdown
pub fn render_conversation(conv: &Conversation) -> String {
    let mut out = String::with_capacity(8192);

    // Header
    out.push_str(&format!("# Session: {}\n\n", conv.session_id));
    out.push_str(&format!("**Project:** {}\n", conv.project));
    if let Some(ref model) = conv.model {
        out.push_str(&format!("**Model:** {}\n", model));
    }
    if let Some(ref start) = conv.start_time {
        out.push_str(&format!("**Started:** {}\n", start));
    }
    if let Some(ref end) = conv.end_time {
        out.push_str(&format!("**Ended:** {}\n", end));
    }
    out.push_str(&format!(
        "**Tokens:** {} in / {} out\n",
        conv.total_input_tokens, conv.total_output_tokens
    ));
    out.push_str(&format!("**Turns:** {}\n", conv.turns.len()));
    out.push_str("\n---\n\n");

    // Turns
    for (i, turn) in conv.turns.iter().enumerate() {
        out.push_str(&format!("## Turn {}\n\n", i + 1));

        // User message
        out.push_str("### User\n\n");
        let user_text = turn.user_text.trim();
        if user_text.len() > 2000 {
            out.push_str(&user_text[..2000]);
            out.push_str("\n\n*[truncated]*\n\n");
        } else {
            out.push_str(user_text);
            out.push_str("\n\n");
        }

        // Tool interactions
        if !turn.tool_interactions.is_empty() {
            out.push_str("### Tool Calls\n\n");
            for tool in &turn.tool_interactions {
                let status = if tool.is_error { " ❌" } else { "" };
                out.push_str(&format!(
                    "<details>\n<summary><strong>{}</strong>{}</summary>\n\n",
                    tool.tool_name, status
                ));
                if !tool.input_summary.is_empty() {
                    out.push_str(&format!(
                        "**Input:** `{}`\n\n",
                        tool.input_summary.replace('`', "'")
                    ));
                }
                if !tool.output_summary.is_empty() {
                    out.push_str("**Output:**\n```\n");
                    out.push_str(&tool.output_summary);
                    out.push_str("\n```\n\n");
                }
                out.push_str("</details>\n\n");
            }
        }

        // Assistant response
        let assistant_text = turn.assistant_text.trim();
        if !assistant_text.is_empty() {
            out.push_str("### Assistant\n\n");
            if assistant_text.len() > 5000 {
                out.push_str(&assistant_text[..5000]);
                out.push_str("\n\n*[truncated]*\n\n");
            } else {
                out.push_str(assistant_text);
                out.push_str("\n\n");
            }
        }

        out.push_str("---\n\n");
    }

    out
}

/// Render machine-readable metadata JSON
pub fn render_meta(conv: &Conversation) -> String {
    let tool_counts = count_tools(conv);

    let meta = serde_json::json!({
        "session_id": conv.session_id,
        "project": conv.project,
        "model": conv.model,
        "start_time": conv.start_time,
        "end_time": conv.end_time,
        "total_input_tokens": conv.total_input_tokens,
        "total_output_tokens": conv.total_output_tokens,
        "turn_count": conv.turns.len(),
        "tool_usage": tool_counts,
    });

    serde_json::to_string_pretty(&meta).unwrap_or_default()
}

/// Render a brief summary of the conversation
pub fn render_summary(conv: &Conversation) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {} - {}\n\n", conv.project, conv.session_id));

    if let Some(ref start) = conv.start_time {
        out.push_str(&format!("**Date:** {}\n", start));
    }
    out.push_str(&format!("**Turns:** {}\n", conv.turns.len()));
    out.push_str(&format!(
        "**Tokens:** {} in / {} out\n\n",
        conv.total_input_tokens, conv.total_output_tokens
    ));

    // First user message as topic hint
    if let Some(first_turn) = conv.turns.first() {
        let topic = first_turn.user_text.trim();
        let topic = if topic.len() > 500 {
            &topic[..500]
        } else {
            topic
        };
        out.push_str(&format!("**Initial request:** {}\n\n", topic));
    }

    // Tool usage summary
    let tool_counts = count_tools(conv);
    if !tool_counts.is_empty() {
        out.push_str("**Tools used:** ");
        let tools: Vec<String> = tool_counts
            .iter()
            .map(|(name, count)| format!("{}({})", name, count))
            .collect();
        out.push_str(&tools.join(", "));
        out.push('\n');
    }

    out
}

fn count_tools(conv: &Conversation) -> Vec<(String, usize)> {
    let mut counts = std::collections::HashMap::new();
    for turn in &conv.turns {
        for tool in &turn.tool_interactions {
            *counts.entry(tool.tool_name.clone()).or_insert(0) += 1;
        }
    }
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted
}
