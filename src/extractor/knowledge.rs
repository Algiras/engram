use chrono::{DateTime, Utc};
use regex::Regex;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::Result;
use crate::llm::client::LlmClient;
use crate::llm::prompts;
use crate::parser::conversation::Conversation;

// ── Session block parsing ──────────────────────────────────────────────

/// A parsed session block from a knowledge file
pub struct SessionBlock {
    pub session_id: String,
    pub timestamp: String,
    pub ttl: Option<String>,
    pub header: String,
    pub content: String,
    pub preview: String,
}

/// Parse a knowledge file into (preamble, Vec<SessionBlock>).
/// Preamble = everything before first "## Session:" header (e.g., "# Decisions\n").
pub fn parse_session_blocks(file_content: &str) -> (String, Vec<SessionBlock>) {
    let header_re =
        Regex::new(r"(?m)^## Session: (\S+) \(([^)]+)\)(?: \[ttl:([^\]]+)\])?").unwrap();
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
        let ttl = caps.get(3).map(|m| m.as_str().to_string());

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

    let decisions = clean_extraction(&decisions_raw);
    let solutions = clean_extraction(&solutions_raw);
    let patterns = clean_extraction(&patterns_raw);
    let preferences = clean_extraction(&preferences_raw);
    let bugs = clean_extraction(&bugs_raw);
    let insights = clean_extraction(&insights_raw);
    let questions = clean_extraction(&questions_raw);

    // Write review inbox candidates (short-term memory)
    let inbox_path = knowledge_dir.join("inbox.md");
    if !inbox_path.exists() {
        std::fs::write(&inbox_path, "# Inbox\n")?;
    }

    let ts = conversation.start_time.as_deref().unwrap_or("unknown date");
    if let Some(ref decisions) = decisions {
        let inbox_header = if let Some(ttl_val) = ttl {
            format!(
                "\n\n## Session: {}:decisions ({}) [ttl:{}]\n\n",
                conversation.session_id, ts, ttl_val
            )
        } else {
            format!(
                "\n\n## Session: {}:decisions ({})\n\n",
                conversation.session_id, ts
            )
        };
        let inbox_content = format!("- category: decisions\n- scope: project\n\n{}", decisions);
        append_knowledge(&inbox_path, &inbox_header, &inbox_content)?;
    }
    if let Some(ref solutions) = solutions {
        let inbox_header = if let Some(ttl_val) = ttl {
            format!(
                "\n\n## Session: {}:solutions ({}) [ttl:{}]\n\n",
                conversation.session_id, ts, ttl_val
            )
        } else {
            format!(
                "\n\n## Session: {}:solutions ({})\n\n",
                conversation.session_id, ts
            )
        };
        let inbox_content = format!("- category: solutions\n- scope: project\n\n{}", solutions);
        append_knowledge(&inbox_path, &inbox_header, &inbox_content)?;
    }
    if let Some(ref patterns) = patterns {
        let inbox_header = if let Some(ttl_val) = ttl {
            format!(
                "\n\n## Session: {}:patterns ({}) [ttl:{}]\n\n",
                conversation.session_id, ts, ttl_val
            )
        } else {
            format!(
                "\n\n## Session: {}:patterns ({})\n\n",
                conversation.session_id, ts
            )
        };
        let inbox_content = format!("- category: patterns\n- scope: project\n\n{}", patterns);
        append_knowledge(&inbox_path, &inbox_header, &inbox_content)?;
    }
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

    if let Some(ref decisions) = decisions {
        append_knowledge(
            &knowledge_dir.join("decisions.md"),
            &session_header,
            decisions,
        )?;
    }
    if let Some(ref solutions) = solutions {
        append_knowledge(
            &knowledge_dir.join("solutions.md"),
            &session_header,
            solutions,
        )?;
    }
    if let Some(ref patterns) = patterns {
        append_knowledge(
            &knowledge_dir.join("patterns.md"),
            &session_header,
            patterns,
        )?;
    }

    // Global preferences
    let global_dir = config.memory_dir.join("knowledge").join("_global");
    std::fs::create_dir_all(&global_dir)?;
    if let Some(ref preferences) = preferences {
        append_knowledge(
            &global_dir.join("preferences.md"),
            &session_header,
            preferences,
        )?;
    }

    // New: bugs, insights, questions
    if let Some(ref bugs) = bugs {
        append_knowledge(&knowledge_dir.join("bugs.md"), &session_header, bugs)?;
    }
    if let Some(ref insights) = insights {
        append_knowledge(
            &knowledge_dir.join("insights.md"),
            &session_header,
            insights,
        )?;
    }
    if let Some(ref questions) = questions {
        append_knowledge(
            &knowledge_dir.join("questions.md"),
            &session_header,
            questions,
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
    let all_summaries = collect_summaries(&summary_dir)?;

    let context = client
        .chat(
            prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &prompts::context_prompt(
                project_name,
                &all_decisions,
                &all_solutions,
                &all_patterns,
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
