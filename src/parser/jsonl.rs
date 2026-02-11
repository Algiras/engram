use std::io::BufRead;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// A single line parsed from a Claude JSONL conversation file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JournalEntry {
    #[serde(rename = "user")]
    User(UserEntry),
    #[serde(rename = "assistant")]
    Assistant(AssistantEntry),
    #[serde(rename = "file-history-snapshot")]
    FileHistorySnapshot(serde_json::Value),
    #[serde(rename = "progress")]
    Progress(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserEntry {
    pub uuid: Option<String>,
    pub parent_uuid: Option<String>,
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    pub message: UserMessage,
    pub cwd: Option<String>,
    #[serde(default)]
    pub is_sidechain: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub role: Option<String>,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantEntry {
    pub uuid: Option<String>,
    pub parent_uuid: Option<String>,
    pub session_id: Option<String>,
    pub timestamp: Option<String>,
    pub message: AssistantMessage,
    #[serde(default)]
    pub is_sidechain: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub role: Option<String>,
    pub content: MessageContent,
    pub model: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_input_tokens: Option<u64>,
    pub cache_creation_input_tokens: Option<u64>,
}

/// Content can be a plain string or an array of content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: Option<String>,
        #[serde(default)]
        summary: Option<Vec<SummaryBlock>>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: Option<String>,
        name: Option<String>,
        input: Option<serde_json::Value>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: Option<String>,
        content: Option<ToolResultContent>,
        #[serde(default)]
        is_error: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryBlock {
    #[serde(rename = "type")]
    pub block_type: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolResultContent {
    Text(String),
    Blocks(Vec<ToolResultBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultBlock {
    #[serde(rename = "type")]
    pub block_type: Option<String>,
    pub text: Option<String>,
}

impl MessageContent {
    /// Extract the plain text content, ignoring tool blocks and thinking
    pub fn text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// Get all content blocks (returns empty vec for plain text)
    pub fn blocks(&self) -> Vec<&ContentBlock> {
        match self {
            MessageContent::Text(_) => Vec::new(),
            MessageContent::Blocks(blocks) => blocks.iter().collect(),
        }
    }
}

/// Parse a JSONL file into journal entries, streaming line-by-line
pub fn parse_jsonl(path: &Path) -> Result<Vec<JournalEntry>> {
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::with_capacity(64 * 1024, file);
    let mut entries = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Try to parse; skip malformed lines
        match serde_json::from_str::<JournalEntry>(line) {
            Ok(entry) => entries.push(entry),
            Err(_) => {
                // Some lines may have unknown types; skip silently
                continue;
            }
        }
    }

    Ok(entries)
}
