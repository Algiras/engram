use chrono::{DateTime, Utc};
use regex::Regex;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::Result;
use crate::llm::client::LlmClient;
use crate::llm::prompts;
use crate::parser::conversation::Conversation;

// ── Update Resolver ────────────────────────────────────────────────────

/// Decision returned by the Update Resolver for each new knowledge entry.
#[derive(Debug)]
pub enum UpdateAction {
    /// New information — append as a new block
    Add,
    /// Merges/updates an existing block with new content
    Update {
        existing_session_id: String,
        merged_content: String,
    },
    /// New entry supersedes and replaces an existing block
    Delete { existing_session_id: String },
    /// Duplicate — skip silently
    Noop,
}

/// Find top-N existing blocks that share enough word overlap with new_content to be candidates.
fn find_top_similar_blocks<'a>(
    new_content: &str,
    existing_blocks: &'a [SessionBlock],
    top_n: usize,
) -> Vec<(&'a SessionBlock, f32)> {
    let new_words: std::collections::HashSet<String> = new_content
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 4)
        .map(|w| w.to_lowercase())
        .collect();

    if new_words.len() < 3 {
        return Vec::new();
    }

    let mut scored: Vec<(&SessionBlock, f32)> = existing_blocks
        .iter()
        .filter(|b| !b.content.contains("<!-- superseded"))
        .filter_map(|block| {
            let block_words: std::collections::HashSet<String> = block
                .content
                .split(|c: char| !c.is_alphanumeric())
                .filter(|w| w.len() > 4)
                .map(|w| w.to_lowercase())
                .collect();
            if block_words.is_empty() {
                return None;
            }
            let overlap = new_words.intersection(&block_words).count();
            let min_len = new_words.len().min(block_words.len());
            let similarity = overlap as f32 / min_len as f32;
            if similarity > 0.15 {
                Some((block, similarity))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(top_n);
    scored
}

/// Parse an LLM resolver response line into an UpdateAction.
/// Fallback = Add on any parse error.
fn parse_resolver_response(response: &str) -> UpdateAction {
    let trimmed = response.trim();
    let first_line = trimmed.lines().next().unwrap_or("").trim();

    if first_line.eq_ignore_ascii_case("add") {
        return UpdateAction::Add;
    }
    if first_line.eq_ignore_ascii_case("noop") {
        return UpdateAction::Noop;
    }
    if let Some(rest) = first_line.strip_prefix("UPDATE ").or_else(|| first_line.strip_prefix("update ")) {
        let session_id = rest.trim().to_string();
        // Content is everything after the first line
        let merged_content = trimmed
            .lines()
            .skip(1)
            .collect::<Vec<_>>()
            .join("\n")
            .trim()
            .to_string();
        if !session_id.is_empty() && !merged_content.is_empty() {
            return UpdateAction::Update {
                existing_session_id: session_id,
                merged_content,
            };
        }
    }
    if let Some(rest) = first_line.strip_prefix("DELETE ").or_else(|| first_line.strip_prefix("delete ")) {
        let session_id = rest.trim().to_string();
        if !session_id.is_empty() {
            return UpdateAction::Delete {
                existing_session_id: session_id,
            };
        }
    }

    // Fallback — never silently lose data
    UpdateAction::Add
}

/// Resolve what action to take when ingesting new content into a category.
/// Returns Add immediately (no LLM call) when there are no similar existing blocks.
pub async fn resolve_update(
    client: &LlmClient,
    category: &str,
    new_content: &str,
    existing_blocks: &[SessionBlock],
) -> UpdateAction {
    let candidates = find_top_similar_blocks(new_content, existing_blocks, 3);
    if candidates.is_empty() {
        return UpdateAction::Add;
    }

    // Build existing entries snippet for the LLM
    let existing_snippet: String = candidates
        .iter()
        .map(|(b, _)| {
            format!(
                "[session_id: {}]\n{}",
                b.session_id,
                b.content.trim().chars().take(400).collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    match client
        .chat(
            prompts::SYSTEM_UPDATE_RESOLVER,
            &prompts::update_resolver_prompt(
                category,
                &new_content.trim().chars().take(600).collect::<String>(),
                &existing_snippet,
            ),
        )
        .await
    {
        Ok(response) => {
            let action = parse_resolver_response(&response);
            eprintln!("  [resolver] {} → {:?}", category, action);
            action
        }
        Err(_) => UpdateAction::Add, // Fallback on LLM failure
    }
}

// ── Session block parsing ──────────────────────────────────────────────

/// A parsed session block from a knowledge file
pub struct SessionBlock {
    pub session_id: String,
    pub timestamp: String,
    pub ttl: Option<String>,
    pub confidence: Option<String>,
    /// FadeMem strength: how "strong" this memory is (1.0 = fresh, decays over time unless recalled)
    pub strength: Option<f32>,
    /// Number of times this block has been accessed/recalled
    pub access_count: Option<u32>,
    pub header: String,
    pub content: String,
    pub preview: String,
}

/// Parse a knowledge file into (preamble, Vec<SessionBlock>).
/// Preamble = everything before first "## Session:" header (e.g., "# Decisions\n").
/// Supports optional metadata tags in any order: [ttl:...] [confidence:...] [strength:...] [access:N]
pub fn parse_session_blocks(file_content: &str) -> (String, Vec<SessionBlock>) {
    // Match the core header; all bracket tags are captured separately below
    let header_re = Regex::new(
        r"(?m)^## Session: (\S+) \(([^)]+)\)((?:\s*\[[^\]]+\])*)",
    )
    .unwrap();
    // Individual tag extractors
    let ttl_re = Regex::new(r"\[ttl:([^\]]+)\]").unwrap();
    let conf_re = Regex::new(r"\[confidence:([^\]]+)\]").unwrap();
    let strength_re = Regex::new(r"\[strength:([\d.]+)\]").unwrap();
    let access_re = Regex::new(r"\[access:(\d+)\]").unwrap();

    let mut blocks = Vec::new();

    let first_match = header_re.find(file_content);
    let preamble = match first_match {
        Some(m) => file_content[..m.start()].to_string(),
        None => return (file_content.to_string(), blocks),
    };

    let matches: Vec<_> = header_re.captures_iter(file_content).collect();
    let match_positions: Vec<_> = header_re.find_iter(file_content).collect();

    for (i, caps) in matches.iter().enumerate() {
        let session_id = caps[1].to_string();
        let timestamp = caps[2].to_string();
        let tags = caps.get(3).map(|m| m.as_str()).unwrap_or("");

        let ttl = ttl_re.captures(tags).map(|c| c[1].to_string());
        let confidence = conf_re.captures(tags).map(|c| c[1].to_string());
        let strength = strength_re
            .captures(tags)
            .and_then(|c| c[1].parse::<f32>().ok());
        let access_count = access_re
            .captures(tags)
            .and_then(|c| c[1].parse::<u32>().ok());

        let header_start = match_positions[i].start();
        let content_start = match_positions[i].end();
        let block_end = if i + 1 < match_positions.len() {
            match_positions[i + 1].start()
        } else {
            file_content.len()
        };

        let header = file_content[header_start..content_start].to_string();
        let content = file_content[content_start..block_end].to_string();

        let preview = content
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .chars()
            .take(80)
            .collect::<String>();

        blocks.push(SessionBlock {
            session_id,
            timestamp,
            ttl,
            confidence,
            strength,
            access_count,
            header,
            content,
            preview,
        });
    }

    (preamble, blocks)
}

/// Remove blocks matching session_ids. Returns None if nothing matched.
pub fn remove_session_blocks(file_content: &str, session_ids: &[&str]) -> Option<String> {
    let (preamble, blocks) = parse_session_blocks(file_content);

    let before_count = blocks.len();
    let remaining: Vec<&SessionBlock> = blocks
        .iter()
        .filter(|b| !session_ids.contains(&b.session_id.as_str()))
        .collect();

    if remaining.len() == before_count {
        return None;
    }

    let mut result = preamble;
    for block in remaining {
        result.push_str(&block.header);
        result.push_str(&block.content);
    }

    Some(result)
}

/// Find session IDs whose content matches query (case-insensitive substring).
pub fn find_sessions_by_topic(file_content: &str, query: &str) -> Vec<String> {
    let (_preamble, blocks) = parse_session_blocks(file_content);
    let query_lower = query.to_lowercase();

    blocks
        .into_iter()
        .filter(|b| b.content.to_lowercase().contains(&query_lower))
        .map(|b| b.session_id)
        .collect()
}

/// Parse "7d", "30d", "2w", "1h", "30m" into chrono::Duration
pub fn parse_ttl(s: &str) -> Option<chrono::Duration> {
    let s = s.trim();
    if s.len() < 2 {
        return None;
    }
    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str.parse().ok()?;
    match unit {
        "m" => Some(chrono::Duration::minutes(num)),
        "h" => Some(chrono::Duration::hours(num)),
        "d" => Some(chrono::Duration::days(num)),
        "w" => Some(chrono::Duration::weeks(num)),
        _ => None,
    }
}

/// Parse duration string, returning an error if the format is invalid.
pub fn parse_duration_strict(s: &str) -> crate::error::Result<chrono::Duration> {
    parse_ttl(s).ok_or_else(|| crate::error::MemoryError::InvalidDuration(s.to_string()))
}

/// Returns true if block has TTL and is expired (permanent entries → false)
pub fn is_expired(block: &SessionBlock) -> bool {
    let ttl_str = match &block.ttl {
        Some(t) => t,
        None => return false, // permanent
    };
    let duration = match parse_ttl(ttl_str) {
        Some(d) => d,
        None => return false, // unparseable TTL → treat as permanent
    };
    let timestamp = match DateTime::parse_from_rfc3339(&block.timestamp) {
        Ok(ts) => ts.with_timezone(&Utc),
        Err(_) => return false, // unparseable timestamp → treat as permanent
    };
    Utc::now() > timestamp + duration
}

/// Partition blocks into (active, expired)
pub fn partition_by_expiry(blocks: Vec<SessionBlock>) -> (Vec<SessionBlock>, Vec<SessionBlock>) {
    let mut active = Vec::new();
    let mut expired = Vec::new();
    for block in blocks {
        if is_expired(&block) {
            expired.push(block);
        } else {
            active.push(block);
        }
    }
    (active, expired)
}

/// Reconstruct markdown content from a preamble and list of blocks
pub fn reconstruct_blocks(preamble: &str, blocks: &[SessionBlock]) -> String {
    let mut result = preamble.to_string();
    for block in blocks {
        result.push_str(&block.header);
        result.push_str(&block.content);
    }
    result
}

/// Replace an existing session block with new content. Returns None if session not found.
pub fn replace_session_block(
    file_content: &str,
    session_id: &str,
    new_header: &str,
    new_content: &str,
) -> Option<String> {
    let (preamble, blocks) = parse_session_blocks(file_content);

    if !blocks.iter().any(|b| b.session_id == session_id) {
        return None;
    }

    let mut result = preamble;
    for block in &blocks {
        if block.session_id == session_id {
            result.push_str(new_header);
            result.push_str(new_content);
            // Ensure trailing newline
            if !new_content.ends_with('\n') {
                result.push('\n');
            }
        } else {
            result.push_str(&block.header);
            result.push_str(&block.content);
        }
    }

    Some(result)
}

/// Strip `<private>…</private>` blocks from text before storage or injection.
/// Matching is case-insensitive and supports multi-line content.
pub fn strip_private_tags(text: &str) -> String {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re =
        RE.get_or_init(|| regex::Regex::new(r"(?is)<private>.*?</private>").expect("static regex"));
    let result = re.replace_all(text, "");
    // Collapse runs of blank lines left behind by removal
    static BLANK_RE: OnceLock<regex::Regex> = OnceLock::new();
    let blank_re = BLANK_RE.get_or_init(|| regex::Regex::new(r"\n{3,}").expect("static regex"));
    blank_re.replace_all(&result, "\n\n").to_string()
}

/// Parse the CONFIDENCE: HIGH|MEDIUM|LOW line from LLM extraction output.
/// Returns (content_without_confidence_line, confidence_level_string).
pub fn parse_confidence(text: &str) -> (String, Option<String>) {
    let mut conf_line_idx = None;
    for (i, line) in text.lines().enumerate() {
        let upper = line.trim().to_ascii_uppercase();
        if upper.starts_with("CONFIDENCE:") {
            conf_line_idx = Some((i, upper));
        }
    }

    if let Some((pos, conf_line)) = conf_line_idx {
        let level = if conf_line.contains("HIGH") {
            Some("high".to_string())
        } else if conf_line.contains("MEDIUM") {
            Some("medium".to_string())
        } else if conf_line.contains("LOW") {
            Some("low".to_string())
        } else {
            None
        };

        let content: String = text
            .lines()
            .enumerate()
            .filter(|(i, _)| *i != pos)
            .map(|(_, l)| l)
            .collect::<Vec<_>>()
            .join("\n")
            .trim_end()
            .to_string();

        (content, level)
    } else {
        (text.trim_end().to_string(), None)
    }
}

/// Check if new content is a near-duplicate of any active block (>75% word overlap).
/// Used to skip re-storing patterns that already exist.
pub fn is_near_duplicate(new_content: &str, existing_blocks: &[SessionBlock]) -> bool {
    let new_words: std::collections::HashSet<String> = new_content
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 4)
        .map(|w| w.to_lowercase())
        .collect();

    if new_words.len() < 5 {
        return false; // Too short to meaningfully dedup
    }

    for block in existing_blocks {
        if block.content.contains("<!-- superseded") {
            continue;
        }

        let block_words: std::collections::HashSet<String> = block
            .content
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 4)
            .map(|w| w.to_lowercase())
            .collect();

        if block_words.is_empty() {
            continue;
        }

        let overlap = new_words.intersection(&block_words).count();
        let min_len = new_words.len().min(block_words.len());
        let similarity = overlap as f32 / min_len as f32;

        if similarity > 0.75 {
            return true;
        }
    }

    false
}

/// Build a session block header with optional tags: ttl, confidence, strength, access_count.
pub(crate) fn build_header(
    session_id: &str,
    ts: &str,
    ttl: Option<&str>,
    confidence: Option<&str>,
    strength: Option<f32>,
    access_count: Option<u32>,
) -> String {
    let mut h = format!("\n\n## Session: {} ({})", session_id, ts);
    if let Some(t) = ttl {
        h.push_str(&format!(" [ttl:{}]", t));
    }
    if let Some(c) = confidence {
        h.push_str(&format!(" [confidence:{}]", c));
    }
    if let Some(s) = strength {
        h.push_str(&format!(" [strength:{:.2}]", s));
    }
    if let Some(a) = access_count {
        h.push_str(&format!(" [access:{}]", a));
    }
    h.push_str("\n\n");
    h
}

/// Increment the access count for a session block in a file's content.
/// Returns the updated file content, or None if the session was not found.
pub fn increment_access_count(file_content: &str, session_id: &str) -> Option<String> {
    let (preamble, blocks) = parse_session_blocks(file_content);

    let found = blocks.iter().any(|b| b.session_id == session_id);
    if !found {
        return None;
    }

    let mut result = preamble;
    for block in &blocks {
        if block.session_id == session_id {
            let new_count = block.access_count.unwrap_or(0) + 1;
            let new_header = build_header(
                &block.session_id,
                &block.timestamp,
                block.ttl.as_deref(),
                block.confidence.as_deref(),
                block.strength,
                Some(new_count),
            );
            result.push_str(&new_header);
            result.push_str(&block.content);
        } else {
            result.push_str(&block.header);
            result.push_str(&block.content);
        }
    }

    Some(result)
}

/// Extract session ID from a header string like "\n\n## Session: abc-123 (2025-01-01)\n\n"
fn extract_session_id_from_header(header: &str) -> Option<String> {
    let re = Regex::new(r"## Session: (\S+) \(").unwrap();
    re.captures(header).map(|c| c[1].to_string())
}

/// Load files edited in a session from today's and yesterday's observations JSONL.
/// Returns a deduplicated list of file paths that were touched in the given session.
fn load_session_observations(
    memory_dir: &Path,
    project_name: &str,
    session_id: &str,
) -> Vec<String> {
    let obs_dir = memory_dir.join("observations").join(project_name);
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let yesterday = (chrono::Utc::now() - chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    let mut files: Vec<String> = Vec::new();
    for date in &[today, yesterday] {
        let path = obs_dir.join(format!("{}.jsonl", date));
        if let Ok(content) = std::fs::read_to_string(&path) {
            for line in content.lines() {
                if let Ok(rec) = serde_json::from_str::<serde_json::Value>(line) {
                    let matches_session = rec
                        .get("session")
                        .and_then(|v| v.as_str())
                        .map(|s| s == session_id)
                        .unwrap_or(false);
                    if matches_session {
                        if let Some(f) = rec.get("file").and_then(|v| v.as_str()) {
                            if !f.is_empty() && !files.contains(&f.to_string()) {
                                files.push(f.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    files
}

/// Extract knowledge from a conversation and merge into project knowledge files
pub async fn extract_and_merge_knowledge(
    config: &Config,
    project_name: &str,
    conversation: &Conversation,
    ttl: Option<&str>,
) -> Result<()> {
    let client = LlmClient::new(&config.llm);

    // Build a text representation of the conversation for LLM input
    let base_text = conversation_to_text(conversation);
    let obs_files =
        load_session_observations(&config.memory_dir, project_name, &conversation.session_id);
    let conv_text = if !obs_files.is_empty() {
        format!(
            "[Files edited in this session: {}]\n\n{}",
            obs_files.join(", "),
            base_text
        )
    } else {
        base_text
    };

    if conv_text.trim().is_empty() {
        return Ok(());
    }

    // Extract different knowledge types in sequence (be gentle on local models)
    let decisions_raw = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::decisions_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    let solutions_raw = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::solutions_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    let patterns_raw = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::patterns_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    let preferences_raw = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::preferences_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    let bugs_raw = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::bugs_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    let insights_raw = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::insights_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    let questions_raw = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::questions_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    let procedures_raw = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::procedures_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    let summary = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::summary_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));

    // Write to knowledge directory
    let knowledge_dir = config.memory_dir.join("knowledge").join(project_name);
    std::fs::create_dir_all(&knowledge_dir)?;

    // Append to per-project knowledge files
    let session_header = if let Some(ttl_val) = ttl {
        format!(
            "\n\n## Session: {} ({}) [ttl:{}]\n\n",
            conversation.session_id,
            conversation.start_time.as_deref().unwrap_or("unknown date"),
            ttl_val
        )
    } else {
        format!(
            "\n\n## Session: {} ({})\n\n",
            conversation.session_id,
            conversation.start_time.as_deref().unwrap_or("unknown date")
        )
    };

    let (decisions_text, decisions_conf) = parse_confidence(&decisions_raw);
    let (solutions_text, solutions_conf) = parse_confidence(&solutions_raw);
    let (patterns_text, patterns_conf) = parse_confidence(&patterns_raw);
    let (preferences_text, _prefs_conf) = parse_confidence(&preferences_raw);
    let (bugs_text, bugs_conf) = parse_confidence(&bugs_raw);
    let (insights_text, insights_conf) = parse_confidence(&insights_raw);
    let (questions_text, questions_conf) = parse_confidence(&questions_raw);
    let (procedures_text, procedures_conf) = parse_confidence(&procedures_raw);

    let decisions = clean_extraction(&decisions_text);
    let solutions = clean_extraction(&solutions_text);
    let patterns = clean_extraction(&patterns_text);
    let preferences = clean_extraction(&preferences_text);
    let bugs = clean_extraction(&bugs_text);
    let insights = clean_extraction(&insights_text);
    let questions = clean_extraction(&questions_text);
    let procedures = clean_extraction(&procedures_text);

    // Entity extraction: no dedup — entities aggregate across sessions
    let entities_raw = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::entities_prompt(&conv_text),
        )
        .await
        .unwrap_or_else(|e| format!("(extraction failed: {})", e));
    let (entities_text, _entities_conf) = parse_confidence(&entities_raw);
    let entities = clean_extraction_entities(&entities_text);

    // Write review inbox candidates (short-term memory)
    let inbox_path = knowledge_dir.join("inbox.md");
    if !inbox_path.exists() {
        std::fs::write(&inbox_path, "# Inbox\n")?;
    }

    let ts = conversation.start_time.as_deref().unwrap_or("unknown date");

    // Per-category headers with confidence tags
    let decisions_header =
        build_header(&conversation.session_id, ts, ttl, decisions_conf.as_deref(), None, None);
    let solutions_header =
        build_header(&conversation.session_id, ts, ttl, solutions_conf.as_deref(), None, None);
    let patterns_header =
        build_header(&conversation.session_id, ts, ttl, patterns_conf.as_deref(), None, None);
    let bugs_header =
        build_header(&conversation.session_id, ts, ttl, bugs_conf.as_deref(), None, None);
    let insights_header =
        build_header(&conversation.session_id, ts, ttl, insights_conf.as_deref(), None, None);
    let questions_header =
        build_header(&conversation.session_id, ts, ttl, questions_conf.as_deref(), None, None);
    let procedures_header =
        build_header(&conversation.session_id, ts, ttl, procedures_conf.as_deref(), None, None);

    // Add inbox entries for review
    for (cat_name, content_opt) in &[
        ("decisions", decisions.as_deref()),
        ("solutions", solutions.as_deref()),
        ("patterns", patterns.as_deref()),
        ("bugs", bugs.as_deref()),
        ("insights", insights.as_deref()),
        ("questions", questions.as_deref()),
        ("procedures", procedures.as_deref()),
    ] {
        if let Some(content) = content_opt {
            let inbox_header = if let Some(ttl_val) = ttl {
                format!(
                    "\n\n## Session: {}:{} ({}) [ttl:{}]\n\n",
                    conversation.session_id, cat_name, ts, ttl_val
                )
            } else {
                format!(
                    "\n\n## Session: {}:{} ({})\n\n",
                    conversation.session_id, cat_name, ts
                )
            };
            let inbox_content =
                format!("- category: {}\n- scope: project\n\n{}", cat_name, content);
            append_knowledge(&inbox_path, &inbox_header, &inbox_content)?;
        }
    }
    // Preferences go to inbox with global scope
    if let Some(ref preferences) = preferences {
        let inbox_header = if let Some(ttl_val) = ttl {
            format!(
                "\n\n## Session: {}:preferences ({}) [ttl:{}]\n\n",
                conversation.session_id, ts, ttl_val
            )
        } else {
            format!(
                "\n\n## Session: {}:preferences ({})\n\n",
                conversation.session_id, ts
            )
        };
        let inbox_content = format!(
            "- category: preferences\n- scope: global\n\n{}",
            preferences
        );
        append_knowledge(&inbox_path, &inbox_header, &inbox_content)?;
    }

    // Update Resolver: smart dedup/merge for each category
    // Replaces old contradiction_checks + is_near_duplicate blocks
    {
        let resolver_data: Vec<(String, std::path::PathBuf, Option<String>, String)> = vec![
            ("decisions".to_string(), knowledge_dir.join("decisions.md"), decisions.clone(), decisions_header.clone()),
            ("solutions".to_string(), knowledge_dir.join("solutions.md"), solutions.clone(), solutions_header.clone()),
            ("patterns".to_string(), knowledge_dir.join("patterns.md"), patterns.clone(), patterns_header.clone()),
            ("bugs".to_string(), knowledge_dir.join("bugs.md"), bugs.clone(), bugs_header.clone()),
            ("insights".to_string(), knowledge_dir.join("insights.md"), insights.clone(), insights_header.clone()),
            ("questions".to_string(), knowledge_dir.join("questions.md"), questions.clone(), questions_header.clone()),
            ("procedures".to_string(), knowledge_dir.join("procedures.md"), procedures.clone(), procedures_header.clone()),
        ];

        for (cat_name, cat_path, new_content_opt, header) in &resolver_data {
            let Some(new_content) = new_content_opt else { continue };

            let existing = read_or_default(cat_path);
            let (_, ex_blocks) = parse_session_blocks(&existing);
            let (active, _) = partition_by_expiry(ex_blocks);

            let action = resolve_update(&client, cat_name, new_content, &active).await;

            match action {
                UpdateAction::Add => {
                    append_knowledge(cat_path, header, new_content)?;
                }
                UpdateAction::Update { existing_session_id, merged_content } => {
                    // Rebuild header for the existing block with new content
                    let existing_block = active
                        .iter()
                        .find(|b| b.session_id == existing_session_id);
                    let replacement_header = if let Some(b) = existing_block {
                        build_header(
                            &b.session_id,
                            &b.timestamp,
                            b.ttl.as_deref(),
                            b.confidence.as_deref(),
                            b.strength,
                            b.access_count,
                        )
                    } else {
                        header.clone()
                    };
                    let current = std::fs::read_to_string(cat_path).unwrap_or_default();
                    if let Some(updated) = replace_session_block(
                        &current,
                        &existing_session_id,
                        &replacement_header,
                        &merged_content,
                    ) {
                        std::fs::write(cat_path, updated)?;
                    } else {
                        // Fallback: just append
                        append_knowledge(cat_path, header, new_content)?;
                    }
                    eprintln!(
                        "  [resolver] updated {} entry '{}'",
                        cat_name, existing_session_id
                    );
                }
                UpdateAction::Delete { existing_session_id } => {
                    let current = std::fs::read_to_string(cat_path).unwrap_or_default();
                    if let Some(removed) =
                        remove_session_blocks(&current, &[existing_session_id.as_str()])
                    {
                        std::fs::write(cat_path, removed)?;
                    }
                    // Add the new (superseding) entry
                    append_knowledge(cat_path, header, new_content)?;
                    eprintln!(
                        "  [resolver] deleted superseded {} entry '{}' and added new entry",
                        cat_name, existing_session_id
                    );
                }
                UpdateAction::Noop => {
                    eprintln!("  [resolver] skipped duplicate {} entry", cat_name);
                }
            }
        }
    }

    // Global preferences (no resolver needed — preferences are session-specific)
    let global_dir = config.memory_dir.join("knowledge").join("_global");
    std::fs::create_dir_all(&global_dir)?;
    if let Some(ref preferences) = preferences {
        append_knowledge(
            &global_dir.join("preferences.md"),
            &session_header,
            preferences,
        )?;
    }

    // Entities: no dedup — entities aggregate across sessions
    if let Some(ref entities) = entities {
        append_knowledge(
            &knowledge_dir.join("entities.md"),
            &session_header,
            entities,
        )?;
    }

    // Write summary
    let summary_dir = config.memory_dir.join("summaries").join(project_name);
    std::fs::create_dir_all(&summary_dir)?;
    let summary_with_meta = format!(
        "# {} - {}\n\n**Date:** {}\n\n{}\n",
        project_name,
        conversation.session_id,
        conversation.start_time.as_deref().unwrap_or("unknown"),
        summary
    );
    std::fs::write(
        summary_dir.join(format!("{}.md", conversation.session_id)),
        &summary_with_meta,
    )?;

    // Generate context.md — the key output
    // Read all existing knowledge to synthesize
    let all_decisions = read_or_default(&knowledge_dir.join("decisions.md"));
    let all_solutions = read_or_default(&knowledge_dir.join("solutions.md"));
    let all_patterns = read_or_default(&knowledge_dir.join("patterns.md"));
    let all_bugs = read_or_default(&knowledge_dir.join("bugs.md"));
    let all_insights = read_or_default(&knowledge_dir.join("insights.md"));
    let all_questions = read_or_default(&knowledge_dir.join("questions.md"));
    let all_procedures = read_or_default(&knowledge_dir.join("procedures.md"));
    let all_summaries = collect_summaries(&summary_dir)?;

    let context = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::context_prompt_with_procedures(
                project_name,
                &all_decisions,
                &all_solutions,
                &all_patterns,
                &all_bugs,
                &all_insights,
                &all_questions,
                &all_procedures,
                &all_summaries,
            ),
        )
        .await
        .unwrap_or_else(|_| {
            // Fallback: simple concatenation
            format!(
                "# {} - Project Context\n\n## Summary\n{}\n\n## Key Decisions\n{}\n\n## Patterns\n{}\n",
                project_name,
                summary,
                decisions.as_deref().unwrap_or("No significant decisions."),
                patterns.as_deref().unwrap_or("No significant patterns.")
            )
        });

    let context_with_header = format!("# {} - Project Context\n\n{}\n", project_name, context);
    std::fs::write(knowledge_dir.join("context.md"), &context_with_header)?;

    Ok(())
}

fn conversation_to_text(conv: &Conversation) -> String {
    let mut text = String::with_capacity(4096);

    for turn in &conv.turns {
        text.push_str("USER: ");
        // Limit user message to avoid flooding
        let user = if turn.user_text.len() > 1000 {
            truncate_at_char_boundary(&turn.user_text, 1000)
        } else {
            &turn.user_text
        };
        text.push_str(user);
        text.push('\n');

        // Include tool names but not full output
        for tool in &turn.tool_interactions {
            text.push_str(&format!(
                "[Tool: {} -> {}]\n",
                tool.tool_name, tool.input_summary
            ));
        }

        if !turn.assistant_text.is_empty() {
            text.push_str("ASSISTANT: ");
            let assistant = if turn.assistant_text.len() > 1500 {
                truncate_at_char_boundary(&turn.assistant_text, 1500)
            } else {
                &turn.assistant_text
            };
            text.push_str(assistant);
            text.push('\n');
        }

        text.push('\n');
    }

    // Strip private blocks before the LLM ever sees the text
    strip_private_tags(&text)
}

fn append_knowledge(path: &Path, header: &str, content: &str) -> Result<()> {
    use std::io::Write;

    // Initialize file with title if it doesn't exist
    if !path.exists() {
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Knowledge");
        std::fs::write(path, format!("# {}\n", capitalize(title)))?;
    }

    // Dedup: if this session already exists, replace it instead of appending
    if let Some(session_id) = extract_session_id_from_header(header) {
        let existing = std::fs::read_to_string(path)?;
        if let Some(replaced) = replace_session_block(&existing, &session_id, header, content) {
            std::fs::write(path, replaced)?;
            return Ok(());
        }
    }

    // Fallback: append as before
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    writeln!(file, "{}{}", header, content)?;

    Ok(())
}

fn read_or_default(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn collect_summaries(dir: &Path) -> Result<String> {
    let mut summaries = String::new();

    if !dir.exists() {
        return Ok(summaries);
    }

    let mut files: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();

    for path in files {
        let content = std::fs::read_to_string(path)?;
        summaries.push_str(&content);
        summaries.push('\n');
    }

    Ok(summaries)
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

fn truncate_at_char_boundary(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut idx = max_bytes;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    &s[..idx]
}

/// Check new extracted content against existing blocks in the same category for contradictions.
/// Uses word-overlap as a lightweight pre-filter, then calls the LLM for likely candidates.
/// Returns a list of `(old_session_id, contradiction_description)` pairs.
pub async fn check_for_contradictions_with_existing(
    client: &LlmClient,
    new_content: &str,
    new_session_id: &str,
    category: &str,
    existing_blocks: &[SessionBlock],
) -> Vec<(String, String)> {
    let mut contradictions = Vec::new();
    if new_content.trim().is_empty() || existing_blocks.is_empty() {
        return contradictions;
    }

    // Build normalized word set for pre-filtering
    let new_words: std::collections::HashSet<String> = new_content
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 4)
        .map(|w| w.to_lowercase())
        .collect();

    if new_words.len() < 3 {
        return contradictions;
    }

    for block in existing_blocks {
        // Skip blocks already marked as superseded
        if block.content.contains("<!-- superseded") {
            continue;
        }

        let block_words: std::collections::HashSet<String> = block
            .content
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 4)
            .map(|w| w.to_lowercase())
            .collect();

        if block_words.is_empty() {
            continue;
        }

        let overlap = new_words.intersection(&block_words).count();
        let min_len = new_words.len().min(block_words.len());
        let similarity = overlap as f32 / min_len as f32;

        // Only run LLM check for entries with enough semantic overlap to possibly contradict
        if similarity < 0.15 {
            continue;
        }

        let existing_snippet = format!(
            "[{}:{}]\n{}",
            category,
            block.session_id,
            block.content.trim().chars().take(400).collect::<String>()
        );
        let new_snippet = format!(
            "[{}:{}]\n{}",
            category,
            new_session_id,
            new_content.trim().chars().take(400).collect::<String>()
        );

        if let Ok(response) = client
            .chat(
                prompts::SYSTEM_CONTRADICTION_CHECKER,
                &prompts::contradiction_check_prompt(&new_snippet, &existing_snippet),
            )
            .await
        {
            let resp = response.trim();
            if resp != "No contradictions detected."
                && !resp.is_empty()
                && resp.contains("CONTRADICTS")
            {
                contradictions.push((block.session_id.clone(), resp.to_string()));
            }
        }
    }

    contradictions
}

/// Mark a session block as superseded by a newer session.
/// Prepends a superseded comment to the matching block's content.
/// Returns None if session_id was not found or was already marked.
pub fn mark_superseded(
    file_content: &str,
    session_id: &str,
    superseded_by: &str,
    reason: &str,
) -> Option<String> {
    let (preamble, mut blocks) = parse_session_blocks(file_content);

    let mut modified = false;
    for block in &mut blocks {
        if block.session_id == session_id && !block.content.contains("<!-- superseded") {
            let short_reason = reason.chars().take(120).collect::<String>();
            block.content = format!(
                "<!-- superseded by: {} — {} -->\n{}",
                superseded_by, short_reason, block.content
            );
            modified = true;
        }
    }

    if !modified {
        return None;
    }

    Some(reconstruct_blocks(&preamble, &blocks))
}

/// Check new knowledge content against an existing knowledge file and mark superseded blocks.
/// Returns the count of blocks marked as superseded. Non-fatal on errors.
async fn check_and_mark_contradictions(
    client: &LlmClient,
    category: &str,
    cat_path: &Path,
    new_content: &str,
    new_session_id: &str,
) -> usize {
    if !cat_path.exists() || new_content.trim().is_empty() {
        return 0;
    }

    let existing = match std::fs::read_to_string(cat_path) {
        Ok(s) => s,
        Err(_) => return 0,
    };

    let (_, existing_blocks) = parse_session_blocks(&existing);
    let (active_blocks, _) = partition_by_expiry(existing_blocks);
    if active_blocks.is_empty() {
        return 0;
    }

    let contradictions = check_for_contradictions_with_existing(
        client,
        new_content,
        new_session_id,
        category,
        &active_blocks,
    )
    .await;

    if contradictions.is_empty() {
        return 0;
    }

    let mut updated = existing;
    let mut count = 0;
    for (old_session_id, reason) in &contradictions {
        if let Some(marked) = mark_superseded(&updated, old_session_id, new_session_id, reason) {
            updated = marked;
            count += 1;
        }
    }

    if count > 0 {
        if let Err(e) = std::fs::write(cat_path, &updated) {
            eprintln!(
                "Warning: could not write contradiction marks to {:?}: {}",
                cat_path, e
            );
        }
    }

    count
}

fn clean_extraction_entities(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();
    if lower.contains("no significant entities") || lower.contains("(extraction failed:") {
        return None;
    }
    Some(trimmed.to_string())
}

fn clean_extraction(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lower = trimmed.to_lowercase();
    let is_placeholder = [
        "no significant decisions",
        "no significant problems solved",
        "no significant patterns",
        "no clear preferences",
        "no bugs encountered",
        "no significant insights",
        "no open questions",
        "no significant procedures",
        "(extraction failed:",
    ]
    .iter()
    .any(|p| lower.contains(p));

    if is_placeholder {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Result of auto-cleanup operation
#[derive(Debug, Default)]
pub struct CleanupResult {
    pub removed_count: usize,
    pub removed_session_ids: Vec<String>,
    pub files_modified: Vec<PathBuf>,
}

/// Auto-cleanup expired entries from project knowledge files, inbox.md, and global preferences.
/// Persists changes to disk using atomic writes.
pub fn auto_cleanup_expired(
    memory_dir: &Path,
    project: &str,
    verbose: bool,
) -> crate::Result<CleanupResult> {
    let mut result = CleanupResult::default();

    let knowledge_dir = memory_dir.join("knowledge").join(project);
    let global_prefs = memory_dir
        .join("knowledge")
        .join("_global")
        .join("preferences.md");

    // Files to clean
    let files_to_clean = [
        knowledge_dir.join("decisions.md"),
        knowledge_dir.join("solutions.md"),
        knowledge_dir.join("patterns.md"),
        knowledge_dir.join("inbox.md"),
        global_prefs,
    ];

    for file_path in files_to_clean.iter().filter(|p| p.exists()) {
        let cleanup_info = cleanup_file_expired(file_path, verbose)?;
        if cleanup_info.removed_count > 0 {
            result.removed_count += cleanup_info.removed_count;
            result
                .removed_session_ids
                .extend(cleanup_info.removed_session_ids);
            result.files_modified.push(file_path.clone());
        }
    }

    // Delete stale context.md if any entries removed
    if result.removed_count > 0 {
        let context_path = knowledge_dir.join("context.md");
        if context_path.exists() {
            std::fs::remove_file(&context_path)?;
            if verbose {
                println!("  Deleted stale context.md");
            }
        }
    }

    Ok(result)
}

/// Cleanup expired entries from a single file atomically.
fn cleanup_file_expired(file_path: &Path, verbose: bool) -> crate::Result<CleanupResult> {
    let content = std::fs::read_to_string(file_path)?;
    let (preamble, blocks) = parse_session_blocks(&content);
    let (active, expired_blocks) = partition_by_expiry(blocks);

    if expired_blocks.is_empty() {
        return Ok(CleanupResult::default());
    }

    let removed_session_ids: Vec<String> = expired_blocks
        .iter()
        .map(|b| b.session_id.clone())
        .collect();

    if verbose {
        println!(
            "  {} - removing {} expired entries",
            file_path.file_name().unwrap().to_string_lossy(),
            expired_blocks.len()
        );
        for id in &removed_session_ids {
            println!("    - {}", id);
        }
    }

    let rebuilt = reconstruct_blocks(&preamble, &active);

    // Atomic write
    atomic_write(file_path, &rebuilt)?;

    Ok(CleanupResult {
        removed_count: expired_blocks.len(),
        removed_session_ids,
        files_modified: vec![file_path.to_path_buf()],
    })
}

/// Atomic file write to prevent corruption (write to .tmp, then rename).
fn atomic_write(target: &Path, content: &str) -> crate::Result<()> {
    use std::io::Write;

    let temp_path = target.with_extension("tmp");

    // Write to temp file
    let mut temp_file = std::fs::File::create(&temp_path)?;
    temp_file.write_all(content.as_bytes())?;
    temp_file.sync_all()?; // Ensure data is written to disk
    drop(temp_file);

    // Atomic rename
    #[cfg(unix)]
    std::fs::rename(&temp_path, target)?;

    #[cfg(not(unix))]
    {
        // Windows: remove target first, then rename
        if target.exists() {
            std::fs::remove_file(target)?;
        }
        std::fs::rename(&temp_path, target)?;
    }

    Ok(())
}

#[cfg(test)]
mod improvement_tests {
    use super::*;

    // ── Improvement 1: Procedures category ────────────────────────────────

    #[test]
    fn test_categories_contains_procedures() {
        assert!(crate::config::CATEGORIES.contains(&"procedures"));
        assert_eq!(crate::config::CATEGORIES.len(), 7);
    }

    #[test]
    fn test_category_files_contains_procedures() {
        assert!(crate::config::CATEGORY_FILES.contains(&"procedures.md"));
        assert_eq!(crate::config::CATEGORY_FILES.len(), 7);
    }

    // ── Improvement 2: Access count tracking ──────────────────────────────

    #[test]
    fn test_parse_session_blocks_access_count() {
        let content =
            "# Decisions\n\n## Session: abc-123 (2024-01-01T00:00:00Z) [access:5]\n\nSome content\n";
        let (_, blocks) = parse_session_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].access_count, Some(5));
    }

    #[test]
    fn test_parse_session_blocks_no_access_tag() {
        let content =
            "# Decisions\n\n## Session: abc-123 (2024-01-01T00:00:00Z)\n\nSome content\n";
        let (_, blocks) = parse_session_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].access_count, None);
    }

    #[test]
    fn test_increment_access_count_from_none() {
        let content =
            "# Decisions\n\n## Session: abc-123 (2024-01-01T00:00:00Z)\n\nSome content\n";
        let result = increment_access_count(content, "abc-123");
        assert!(result.is_some());
        let updated = result.unwrap();
        assert!(updated.contains("[access:1]"), "Expected [access:1] in: {}", updated);
    }

    #[test]
    fn test_increment_access_count_from_existing() {
        let content =
            "# Decisions\n\n## Session: abc-123 (2024-01-01T00:00:00Z) [access:3]\n\nSome content\n";
        let result = increment_access_count(content, "abc-123");
        assert!(result.is_some());
        let updated = result.unwrap();
        assert!(updated.contains("[access:4]"), "Expected [access:4] in: {}", updated);
    }

    #[test]
    fn test_increment_access_count_not_found() {
        let content =
            "# Decisions\n\n## Session: abc-123 (2024-01-01T00:00:00Z)\n\nSome content\n";
        let result = increment_access_count(content, "nonexistent-id");
        assert!(result.is_none());
    }

    // ── Improvement 3: FadeMem strength field ─────────────────────────────

    #[test]
    fn test_parse_session_blocks_strength() {
        let content =
            "# Patterns\n\n## Session: xyz (2024-01-01T00:00:00Z) [strength:2.50]\n\nContent\n";
        let (_, blocks) = parse_session_blocks(content);
        assert_eq!(blocks.len(), 1);
        let s = blocks[0].strength.expect("strength should be parsed");
        assert!((s - 2.5).abs() < 0.01, "strength was {}", s);
    }

    #[test]
    fn test_parse_session_blocks_all_tags() {
        let content = "# Decisions\n\n## Session: s1 (2024-01-01T00:00:00Z) [ttl:7d] [confidence:high] [strength:1.30] [access:2]\n\nContent\n";
        let (_, blocks) = parse_session_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].ttl.as_deref(), Some("7d"));
        assert_eq!(blocks[0].confidence.as_deref(), Some("high"));
        assert!((blocks[0].strength.unwrap() - 1.3).abs() < 0.01);
        assert_eq!(blocks[0].access_count, Some(2));
    }

    // ── Improvement 5: Update Resolver ───────────────────────────────────

    #[test]
    fn test_parse_resolver_response_add() {
        let action = parse_resolver_response("ADD");
        assert!(matches!(action, UpdateAction::Add));
    }

    #[test]
    fn test_parse_resolver_response_noop() {
        let action = parse_resolver_response("NOOP");
        assert!(matches!(action, UpdateAction::Noop));
    }

    #[test]
    fn test_parse_resolver_response_update() {
        let action = parse_resolver_response("UPDATE abc-123\nMerged text here.");
        match action {
            UpdateAction::Update { existing_session_id, merged_content } => {
                assert_eq!(existing_session_id, "abc-123");
                assert_eq!(merged_content, "Merged text here.");
            }
            _ => panic!("Expected Update, got {:?}", action),
        }
    }

    #[test]
    fn test_parse_resolver_response_delete() {
        let action = parse_resolver_response("DELETE old-session-456");
        match action {
            UpdateAction::Delete { existing_session_id } => {
                assert_eq!(existing_session_id, "old-session-456");
            }
            _ => panic!("Expected Delete, got {:?}", action),
        }
    }

    #[test]
    fn test_parse_resolver_response_fallback() {
        let action = parse_resolver_response("unexpected gobbledygook");
        assert!(matches!(action, UpdateAction::Add), "Fallback should be Add");
    }

    #[test]
    fn test_find_top_similar_blocks_no_overlap() {
        let blocks = vec![SessionBlock {
            session_id: "s1".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            ttl: None,
            confidence: None,
            strength: None,
            access_count: None,
            header: "header".to_string(),
            content: "completely unrelated content about widgets".to_string(),
            preview: String::new(),
        }];
        let result = find_top_similar_blocks("python async await coroutine", &blocks, 3);
        assert!(result.is_empty(), "Expected no results for zero overlap");
    }

    #[test]
    fn test_find_top_similar_blocks_returns_top3() {
        let make_block = |id: &str, text: &str| SessionBlock {
            session_id: id.to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            ttl: None,
            confidence: None,
            strength: None,
            access_count: None,
            header: String::new(),
            content: text.to_string(),
            preview: String::new(),
        };
        let blocks = vec![
            make_block("s1", "tokio async runtime executor thread"),
            make_block("s2", "tokio async runtime spawn future"),
            make_block("s3", "tokio async runtime channel sender"),
            make_block("s4", "tokio async runtime select macro"),
            make_block("s5", "tokio async runtime join handle"),
        ];
        let result = find_top_similar_blocks("tokio async runtime configuration", &blocks, 3);
        assert!(result.len() <= 3, "Should return at most top 3");
        assert!(!result.is_empty(), "Should find overlapping blocks");
    }
}

#[cfg(test)]
mod private_tag_tests {
    use super::strip_private_tags;

    #[test]
    fn test_strip_inline() {
        let input = "Keep this. <private>secret token</private> Keep this too.";
        let out = strip_private_tags(input);
        assert!(!out.contains("secret token"));
        assert!(out.contains("Keep this."));
        assert!(out.contains("Keep this too."));
    }

    #[test]
    fn test_strip_multiline() {
        let input = "Before\n<private>\nline1\nline2\n</private>\nAfter";
        let out = strip_private_tags(input);
        assert!(!out.contains("line1"));
        assert!(out.contains("Before"));
        assert!(out.contains("After"));
    }

    #[test]
    fn test_strip_case_insensitive() {
        let out = strip_private_tags("x <PRIVATE>secret</PRIVATE> y");
        assert!(!out.contains("secret"));
        assert!(out.contains("x"));
        assert!(out.contains("y"));
    }

    #[test]
    fn test_no_private_tags_unchanged() {
        let input = "Normal text with no private blocks.";
        assert_eq!(strip_private_tags(input), input);
    }

    #[test]
    fn test_multiple_blocks_stripped() {
        let input = "A <private>one</private> B <private>two</private> C";
        let out = strip_private_tags(input);
        assert!(!out.contains("one"));
        assert!(!out.contains("two"));
        assert!(out.contains("A"));
        assert!(out.contains("B"));
        assert!(out.contains("C"));
    }
}
