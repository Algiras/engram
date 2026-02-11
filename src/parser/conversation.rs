use crate::parser::jsonl::{AssistantEntry, ContentBlock, JournalEntry, MessageContent, UserEntry};

/// A complete conversation session
#[derive(Debug, Clone)]
pub struct Conversation {
    pub session_id: String,
    pub project: String,
    pub turns: Vec<Turn>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub model: Option<String>,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

/// A single turn: user message + assistant response
#[derive(Debug, Clone)]
pub struct Turn {
    pub user_text: String,
    pub assistant_text: String,
    pub tool_interactions: Vec<ToolInteraction>,
    #[allow(dead_code)]
    pub timestamp: Option<String>,
}

/// A tool call and its result
#[derive(Debug, Clone)]
pub struct ToolInteraction {
    pub tool_name: String,
    pub input_summary: String,
    pub output_summary: String,
    pub is_error: bool,
}

/// Build a conversation model from parsed JSONL entries
pub fn build_conversation(
    entries: &[JournalEntry],
    session_id: &str,
    project: &str,
) -> Conversation {
    let mut turns = Vec::new();
    let mut current_user: Option<&UserEntry> = None;
    let mut assistant_chunks: Vec<&AssistantEntry> = Vec::new();
    let mut start_time: Option<String> = None;
    let mut end_time: Option<String> = None;
    let mut model: Option<String> = None;
    let mut total_input_tokens: u64 = 0;
    let mut total_output_tokens: u64 = 0;

    for entry in entries {
        match entry {
            JournalEntry::User(user) => {
                // If we have a pending user + assistant, flush the turn
                if let Some(prev_user) = current_user.take() {
                    if !assistant_chunks.is_empty() {
                        let turn = build_turn(prev_user, &assistant_chunks);
                        turns.push(turn);
                        assistant_chunks.clear();
                    }
                }
                current_user = Some(user);

                if start_time.is_none() {
                    start_time = user.timestamp.clone();
                }
                end_time = user.timestamp.clone();
            }
            JournalEntry::Assistant(assistant) => {
                // Track model and tokens
                if model.is_none() {
                    model = assistant.message.model.clone();
                }
                if let Some(ref usage) = assistant.message.usage {
                    total_input_tokens += usage.input_tokens.unwrap_or(0);
                    total_output_tokens += usage.output_tokens.unwrap_or(0);
                }

                end_time = assistant.timestamp.clone();
                assistant_chunks.push(assistant);
            }
            JournalEntry::FileHistorySnapshot(_) | JournalEntry::Progress(_) => {
                // Skip these
            }
        }
    }

    // Flush the last turn
    if let Some(user) = current_user {
        if !assistant_chunks.is_empty() {
            let turn = build_turn(user, &assistant_chunks);
            turns.push(turn);
        }
    }

    Conversation {
        session_id: session_id.to_string(),
        project: project.to_string(),
        turns,
        start_time,
        end_time,
        model,
        total_input_tokens,
        total_output_tokens,
    }
}

fn build_turn(user: &UserEntry, assistant_chunks: &[&AssistantEntry]) -> Turn {
    let user_text = user.message.content.text();

    // Merge assistant chunks — collect text and tool interactions
    let mut assistant_texts = Vec::new();
    let mut tool_interactions = Vec::new();

    // Collect all tool_use blocks first, then match with tool_result
    let mut pending_tool_uses: Vec<(&str, &str, String)> = Vec::new(); // (id, name, input_summary)

    for chunk in assistant_chunks {
        match &chunk.message.content {
            MessageContent::Text(s) => {
                if !s.is_empty() {
                    assistant_texts.push(s.clone());
                }
            }
            MessageContent::Blocks(blocks) => {
                for block in blocks {
                    match block {
                        ContentBlock::Text { text } => {
                            if !text.is_empty() {
                                assistant_texts.push(text.clone());
                            }
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            let tool_name = name.as_deref().unwrap_or("unknown");
                            let tool_id = id.as_deref().unwrap_or("");
                            let input_summary = summarize_tool_input(tool_name, input);
                            pending_tool_uses.push((tool_id, tool_name, input_summary));
                        }
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => {
                            let result_id = tool_use_id.as_deref().unwrap_or("");
                            let output_summary = content
                                .as_ref()
                                .map(summarize_tool_result)
                                .unwrap_or_default();

                            // Match with pending tool_use
                            if let Some(pos) = pending_tool_uses
                                .iter()
                                .position(|(id, _, _)| *id == result_id)
                            {
                                let (_, name, input_summary) = pending_tool_uses.remove(pos);
                                tool_interactions.push(ToolInteraction {
                                    tool_name: name.to_string(),
                                    input_summary,
                                    output_summary,
                                    is_error: *is_error,
                                });
                            } else {
                                // Orphaned tool result
                                tool_interactions.push(ToolInteraction {
                                    tool_name: "unknown".to_string(),
                                    input_summary: String::new(),
                                    output_summary,
                                    is_error: *is_error,
                                });
                            }
                        }
                        ContentBlock::Thinking { .. } => {
                            // Skip thinking blocks
                        }
                    }
                }
            }
        }
    }

    // Any unmatched tool_use blocks (no result received)
    for (_, name, input_summary) in pending_tool_uses {
        tool_interactions.push(ToolInteraction {
            tool_name: name.to_string(),
            input_summary,
            output_summary: String::new(),
            is_error: false,
        });
    }

    Turn {
        user_text,
        assistant_text: assistant_texts.join("\n"),
        tool_interactions,
        timestamp: user.timestamp.clone(),
    }
}

fn summarize_tool_input(tool_name: &str, input: &Option<serde_json::Value>) -> String {
    let input = match input {
        Some(v) => v,
        None => return String::new(),
    };

    match tool_name {
        "Bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| truncate(s, 200))
            .unwrap_or_default(),
        "Read" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Write" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string(),
        "Edit" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string(),
        "Glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Grep" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Task" => input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "WebSearch" => input
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "WebFetch" => input
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => {
            let s = serde_json::to_string(input).unwrap_or_default();
            truncate(&s, 150)
        }
    }
}

fn summarize_tool_result(content: &crate::parser::jsonl::ToolResultContent) -> String {
    let text = match content {
        crate::parser::jsonl::ToolResultContent::Text(s) => s.clone(),
        crate::parser::jsonl::ToolResultContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| b.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n"),
    };
    truncate(&text, 300)
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}
