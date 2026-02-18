//! Progressive disclosure injection for MEMORY.md
//!
//! Provides compact/full injection modes with line budgets,
//! preference deduplication, and pack summarization.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::extractor::knowledge::{
    parse_session_blocks, partition_by_expiry, reconstruct_blocks, SessionBlock,
};
use crate::hive::PackInstaller;

/// Line budget for compact MEMORY.md sections
pub const COMPACT_MAX_LINES: usize = 180;
pub const BUDGET_PREFERENCES: usize = 25;
pub const BUDGET_PROJECT: usize = 60;
pub const BUDGET_SHARED: usize = 40;
pub const BUDGET_PACKS: usize = 20;
pub const BUDGET_GUIDE: usize = 15;

/// Load learned importance boosts from learning state (graceful fallback to empty HashMap)
pub fn load_importance_boosts(memory_dir: &Path, project: &str) -> HashMap<String, f32> {
    use crate::learning::progress;
    match progress::load_state(memory_dir, project) {
        Ok(state) => state.learned_parameters.importance_boosts,
        Err(_) => HashMap::new(), // Graceful degradation
    }
}

/// Lookup importance boost with multiple key format fallbacks.
/// Tries: exact session_id → category:session_id → project:session_id → 0.0
fn lookup_boost(
    boosts: &HashMap<String, f32>,
    session_id: &str,
    category: Option<&str>,
    project: &str,
) -> f32 {
    // Try exact match first
    if let Some(&boost) = boosts.get(session_id) {
        return boost;
    }

    // Try category-prefixed (e.g., "decisions:abc-123")
    if let Some(cat) = category {
        let key = format!("{}:{}", cat, session_id);
        if let Some(&boost) = boosts.get(&key) {
            return boost;
        }
    }

    // Try project-prefixed (e.g., "my-project:abc-123")
    let key = format!("{}:{}", project, session_id);
    if let Some(&boost) = boosts.get(&key) {
        return boost;
    }

    // Default: no boost (neutral)
    0.0
}

/// Compute hybrid importance score: 40% recency + 60% learned boost
fn compute_importance_score(
    timestamp: &str,
    boost: f32,
    oldest_timestamp: &str,
    newest_timestamp: &str,
) -> f32 {
    use chrono::DateTime;

    // Parse timestamps
    let parse_ts = |ts: &str| -> Option<DateTime<chrono::Utc>> {
        DateTime::parse_from_rfc3339(ts)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc))
    };

    let recency_score = match (
        parse_ts(timestamp),
        parse_ts(oldest_timestamp),
        parse_ts(newest_timestamp),
    ) {
        (Some(t), Some(o), Some(n)) if n > o => {
            let total_range = (n - o).num_seconds() as f32;
            if total_range == 0.0 {
                1.0
            } else {
                let age = (n - t).num_seconds() as f32;
                (1.0 - (age / total_range)).clamp(0.0, 1.0)
            }
        }
        _ => 0.5, // Fallback if timestamps unparseable
    };

    // Hybrid: 40% recency, 60% learned boost
    (recency_score * 0.4) + (boost * 0.6)
}

/// Sort session blocks by boosted importance (descending: highest first)
fn sort_by_importance(
    blocks: &mut [SessionBlock],
    boosts: &HashMap<String, f32>,
    project: &str,
    category: Option<&str>,
) {
    if blocks.is_empty() {
        return;
    }

    // Find timestamp range for normalization (clone to avoid borrow conflicts)
    let oldest = blocks
        .iter()
        .map(|b| b.timestamp.clone())
        .min()
        .unwrap_or_default();
    let newest = blocks
        .iter()
        .map(|b| b.timestamp.clone())
        .max()
        .unwrap_or_default();

    // Sort by importance score descending
    blocks.sort_by(|a, b| {
        let boost_a = lookup_boost(boosts, &a.session_id, category, project);
        let boost_b = lookup_boost(boosts, &b.session_id, category, project);

        let score_a = compute_importance_score(&a.timestamp, boost_a, &oldest, &newest);
        let score_b = compute_importance_score(&b.timestamp, boost_b, &oldest, &newest);

        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Deduplicate preferences from multiple session blocks into consolidated bullets.
/// Extracts `**Key:** Value` patterns, groups by key, and keeps unique values.
/// Blocks are processed in importance order (boost + recency) so high-value preferences surface first.
pub fn compact_preferences(raw_prefs: &str, memory_dir: &Path, project: &str) -> String {
    let (_preamble, blocks) = parse_session_blocks(raw_prefs);
    let (mut active, _) = partition_by_expiry(blocks);

    if active.is_empty() {
        return String::new();
    }

    // Sort by importance so high-value sessions contribute first
    let boosts = load_importance_boosts(memory_dir, project);
    if !boosts.is_empty() {
        sort_by_importance(&mut active, &boosts, project, Some("preferences"));
    }

    // Extract **Key:** Value pairs from all blocks
    use std::sync::OnceLock;
    static BOLD_RE: OnceLock<regex::Regex> = OnceLock::new();
    let bold_re = BOLD_RE
        .get_or_init(|| regex::Regex::new(r"\*\*([^*]+):\*\*\s*(.+)").expect("static regex"));

    let mut merged: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for block in &active {
        for line in block.content.lines() {
            let line = line.trim();
            // Match top-level bullets: `* **Key:** Value` or `- **Key:** Value`
            if let Some(caps) = bold_re.captures(line) {
                let key = match caps.get(1) {
                    Some(m) => normalize_pref_key(m.as_str().trim()),
                    None => continue,
                };
                let value = match caps.get(2) {
                    Some(m) => m.as_str().trim().to_string(),
                    None => continue,
                };
                // Skip very generic intro phrases
                if value.starts_with("Here's a breakdown")
                    || value.starts_with("The user")
                    || value.is_empty()
                {
                    continue;
                }
                merged.entry(key).or_default().push(value);
            }
        }
    }

    if merged.is_empty() {
        return String::new();
    }

    // Deduplicate values within each key: extract comma-separated items, unique them
    let mut out = String::new();
    for (key, values) in &merged {
        let items = deduplicate_values(values);
        // Format: keep first ~6 items to stay concise
        let display_items: Vec<&str> = items.iter().map(|s| s.as_str()).take(6).collect();
        let suffix = if items.len() > 6 {
            format!(" (+{} more)", items.len() - 6)
        } else {
            String::new()
        };
        out.push_str(&format!(
            "- **{}:** {}{}\n",
            key,
            display_items.join(", "),
            suffix
        ));
    }

    out
}

/// Normalize category names to merge similar preference keys.
pub fn normalize_pref_key(key: &str) -> String {
    let k = key.to_lowercase();
    if k.contains("tool") {
        return "Tools".to_string();
    }
    if k.contains("language") || k.contains("framework") {
        return "Languages/Frameworks".to_string();
    }
    if k.contains("coding style") || k.contains("style pref") {
        return "Coding Style".to_string();
    }
    if k.contains("workflow") {
        return "Workflow".to_string();
    }
    if k.contains("communication") {
        return "Communication".to_string();
    }
    if k.contains("testing") || k.contains("test pref") {
        return "Testing".to_string();
    }
    // Keep original for less common keys
    key.to_string()
}

/// Deduplicate comma-separated values across multiple strings, case-insensitive.
pub fn deduplicate_values(values: &[String]) -> Vec<String> {
    let mut items: Vec<String> = Vec::new();
    for val in values {
        for item in val.split(',') {
            let item = item.trim().trim_end_matches('.');
            if !item.is_empty() {
                let lower = item.to_lowercase();
                if !items
                    .iter()
                    .any(|existing| existing.to_lowercase() == lower)
                {
                    items.push(item.to_string());
                }
            }
        }
    }
    items
}

/// Create a compact index of installed pack knowledge (names + entry counts).
pub fn compact_pack_summary(memory_dir: &Path) -> crate::Result<String> {
    let installer = PackInstaller::new(memory_dir);
    let knowledge_dirs = installer.get_installed_knowledge_dirs()?;

    if knowledge_dirs.is_empty() {
        return Ok(String::new());
    }

    let mut out = String::new();
    for (pack_name, knowledge_dir) in knowledge_dirs {
        let mut categories = Vec::new();
        for category in &[
            "patterns.md",
            "solutions.md",
            "decisions.md",
            "preferences.md",
        ] {
            let file_path = knowledge_dir.join(category);
            if file_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    let count = content.matches("## Session:").count();
                    if count > 0 {
                        categories.push(format!(
                            "{} ({} entries)",
                            category.replace(".md", ""),
                            count
                        ));
                    }
                }
            }
        }
        if !categories.is_empty() {
            out.push_str(&format!("- **{}**: {}\n", pack_name, categories.join(", ")));
        }
    }

    Ok(out)
}

/// Trim shared memory blocks to fit within a line budget, prioritizing by importance.
/// Sorts by hybrid score (60% learned boost + 40% recency) when learning state exists,
/// falls back to timestamp-only sorting otherwise.
pub fn compact_shared(
    raw_shared: &str,
    max_lines: usize,
    memory_dir: &Path,
    project: &str,
) -> String {
    let (preamble, blocks) = parse_session_blocks(raw_shared);
    let (mut active, _) = partition_by_expiry(blocks);

    if active.is_empty() {
        return String::new();
    }

    // Sort by importance (boost + recency) or fallback to timestamp
    let boosts = load_importance_boosts(memory_dir, project);
    if !boosts.is_empty() {
        sort_by_importance(&mut active, &boosts, project, Some("shared"));
    } else {
        // Fallback: timestamp descending (original behavior)
        active.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    }

    // Greedily include blocks until we hit the line budget
    let mut out = String::new();
    let mut lines_used = 0;

    for block in &active {
        let block_text = format!("{}{}", block.header, block.content);
        let block_lines = block_text.lines().count();
        if lines_used + block_lines > max_lines && lines_used > 0 {
            break;
        }
        out.push_str(&block_text);
        lines_used += block_lines;
    }

    // If we skipped some, add a note
    let included = out.matches("## Session:").count();
    let total = active.len();
    if included < total {
        let priority_note = if !boosts.is_empty() {
            ", prioritized by importance"
        } else {
            ""
        };
        out.push_str(&format!(
            "\n_(Showing {} of {} entries{}. Run `engram recall` for all.)_\n",
            included, total, priority_note
        ));
    }

    if !preamble.trim().is_empty() {
        format!("{}{}", preamble, out)
    } else {
        out
    }
}

/// Trim any section to a maximum number of lines, adding a truncation note.
pub fn trim_to_budget(content: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= max_lines {
        return content.to_string();
    }
    let mut out: String = lines[..max_lines.saturating_sub(2)].join("\n");
    out.push_str("\n\n_(Truncated. Run `engram recall` for full content.)_");
    out
}

/// Build the retrieval guide footer for MEMORY.md.
pub fn retrieval_guide() -> String {
    r#"## Retrieving More Context

For detailed knowledge beyond this summary:
- `engram lookup <project> <query>` - find specific knowledge entries
- `engram search <query>` - full-text search across all knowledge
- `engram recall <project>` - full project context with pack knowledge
- `engram search-semantic <query>` - semantic/vector search

## Sharing & Collaboration

- `engram sync push <project>` - share knowledge to private GitHub Gist (backup/collaboration)
- `engram sync pull <project> <gist-id>` - import knowledge from a shared gist
- See docs/GIST_SHARING.md for details
"#
    .to_string()
}

/// Build compact MEMORY.md with progressive disclosure (≤180 lines).
/// Prioritizes: project context > consolidated prefs > shared summary > pack index > retrieval guide.
pub fn build_compact_memory(
    project_name: &str,
    context_content: &str,
    raw_preferences: &Option<String>,
    raw_shared: &Option<String>,
    memory_dir: &Path,
) -> crate::Result<String> {
    let mut combined = String::new();
    combined.push_str("# Project Memory (auto-injected by engram)\n\n");
    combined.push_str("<!-- Compact mode: run `engram inject --full` for complete dump -->\n\n");

    // 1. Project context first (most valuable)
    combined.push_str(&format!("## Project: {}\n\n", project_name));
    combined.push_str(&trim_to_budget(context_content, BUDGET_PROJECT));
    combined.push_str("\n\n---\n\n");

    // 2. Consolidated preferences (deduplicated, importance-sorted)
    if let Some(raw_prefs) = raw_preferences {
        let prefs = compact_preferences(raw_prefs, memory_dir, project_name);
        if !prefs.is_empty() {
            combined.push_str("## User Preferences (consolidated)\n\n");
            combined.push_str(&trim_to_budget(&prefs, BUDGET_PREFERENCES));
            combined.push_str("\n\n---\n\n");
        }
    }

    // 3. Shared memory (trimmed to budget, importance-prioritized)
    if let Some(raw_sh) = raw_shared {
        let shared = compact_shared(raw_sh, BUDGET_SHARED, memory_dir, project_name);
        if !shared.is_empty() {
            combined.push_str("## Shared Knowledge\n\n");
            combined.push_str(&shared);
            combined.push_str("\n\n---\n\n");
        }
    }

    // 4. Pack index (summary, not full content)
    let pack_summary = compact_pack_summary(memory_dir)?;
    if !pack_summary.is_empty() {
        combined.push_str("## Installed Packs\n\n");
        combined.push_str(&pack_summary);
        combined.push('\n');
    }

    // 5. Retrieval guide footer
    combined.push_str(&retrieval_guide());

    Ok(combined)
}

/// Build full MEMORY.md (legacy behavior, no truncation).
pub fn build_full_memory(
    project_name: &str,
    context_content: &str,
    raw_preferences: &Option<String>,
    raw_shared: &Option<String>,
    memory_dir: &Path,
) -> crate::Result<String> {
    let mut combined = String::new();
    combined.push_str("# Project Memory (auto-injected by engram)\n\n");
    combined.push_str(
        "<!-- This file is auto-generated. Edit knowledge sources, not this file. -->\n\n",
    );

    if let Some(raw_prefs) = raw_preferences {
        let (preamble, blocks) = parse_session_blocks(raw_prefs);
        let (active, _) = partition_by_expiry(blocks);
        let prefs = reconstruct_blocks(&preamble, &active);
        combined.push_str("## Global Preferences\n\n");
        combined.push_str(&prefs);
        combined.push_str("\n\n---\n\n");
    }

    if let Some(raw_sh) = raw_shared {
        let (preamble, blocks) = parse_session_blocks(raw_sh);
        let (active, _) = partition_by_expiry(blocks);
        let shared = reconstruct_blocks(&preamble, &active);
        combined.push_str("## Global Shared Memory\n\n");
        combined.push_str(&shared);
        combined.push_str("\n\n---\n\n");
    }

    combined.push_str(&format!("## Project: {}\n\n", project_name));
    combined.push_str(context_content);

    let pack_content = crate::hive::get_installed_pack_knowledge(memory_dir)?;
    if !pack_content.is_empty() {
        combined.push_str("\n\n---\n\n## Installed Pack Knowledge\n\n");
        combined.push_str(&pack_content);
    }

    Ok(combined)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_lookup_boost_exact_match() {
        let mut boosts = HashMap::new();
        boosts.insert("session-123".to_string(), 0.8);

        let result = lookup_boost(&boosts, "session-123", None, "test-project");
        assert_eq!(result, 0.8);
    }

    #[test]
    fn test_lookup_boost_category_prefix() {
        let mut boosts = HashMap::new();
        boosts.insert("decisions:session-123".to_string(), 0.9);

        let result = lookup_boost(&boosts, "session-123", Some("decisions"), "test-project");
        assert_eq!(result, 0.9);
    }

    #[test]
    fn test_lookup_boost_project_prefix() {
        let mut boosts = HashMap::new();
        boosts.insert("test-project:session-123".to_string(), 0.7);

        let result = lookup_boost(&boosts, "session-123", None, "test-project");
        assert_eq!(result, 0.7);
    }

    #[test]
    fn test_lookup_boost_no_match() {
        let boosts = HashMap::new();
        let result = lookup_boost(&boosts, "unknown", None, "test-project");
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_compute_importance_score_high_boost() {
        let score = compute_importance_score(
            "2024-02-13T12:00:00Z",
            0.9, // High boost
            "2024-02-01T00:00:00Z",
            "2024-02-13T12:00:00Z",
        );
        // Most recent (recency=1.0) + high boost (0.9)
        // = 1.0 * 0.4 + 0.9 * 0.6 = 0.4 + 0.54 = 0.94
        assert!((score - 0.94).abs() < 0.01);
    }

    #[test]
    fn test_compute_importance_score_old_high_boost() {
        let score = compute_importance_score(
            "2024-02-01T00:00:00Z",
            0.9, // High boost
            "2024-02-01T00:00:00Z",
            "2024-02-13T12:00:00Z",
        );
        // Oldest (recency=0.0) + high boost (0.9)
        // = 0.0 * 0.4 + 0.9 * 0.6 = 0.54
        assert!((score - 0.54).abs() < 0.01);
    }

    #[test]
    fn test_compute_importance_score_recent_low_boost() {
        let score = compute_importance_score(
            "2024-02-13T12:00:00Z",
            0.1, // Low boost
            "2024-02-01T00:00:00Z",
            "2024-02-13T12:00:00Z",
        );
        // Most recent (recency=1.0) + low boost (0.1)
        // = 1.0 * 0.4 + 0.1 * 0.6 = 0.4 + 0.06 = 0.46
        assert!((score - 0.46).abs() < 0.01);
    }

    #[test]
    fn test_compute_importance_score_malformed_timestamp() {
        let score = compute_importance_score(
            "invalid-timestamp",
            0.8,
            "2024-02-01T00:00:00Z",
            "2024-02-13T12:00:00Z",
        );
        // Fallback recency=0.5 + boost=0.8
        // = 0.5 * 0.4 + 0.8 * 0.6 = 0.2 + 0.48 = 0.68
        assert!((score - 0.68).abs() < 0.01);
    }

    #[test]
    fn test_sort_by_importance_prioritizes_high_boost() {
        let mut blocks = vec![
            SessionBlock {
                session_id: "old-important".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                ttl: None,
                header: "## Session: old-important (2024-01-01T00:00:00Z)\n".to_string(),
                content: "High-value old knowledge".to_string(),
                preview: "High-value".to_string(),
            },
            SessionBlock {
                session_id: "recent-unimportant".to_string(),
                timestamp: "2024-02-13T00:00:00Z".to_string(),
                ttl: None,
                header: "## Session: recent-unimportant (2024-02-13T00:00:00Z)\n".to_string(),
                content: "Low-value recent".to_string(),
                preview: "Low-value".to_string(),
            },
        ];

        let mut boosts = HashMap::new();
        boosts.insert("old-important".to_string(), 0.9); // High boost
        boosts.insert("recent-unimportant".to_string(), 0.1); // Low boost

        sort_by_importance(&mut blocks, &boosts, "test", None);

        // Old but important should rank first
        assert_eq!(blocks[0].session_id, "old-important");
        assert_eq!(blocks[1].session_id, "recent-unimportant");
    }

    #[test]
    fn test_sort_by_importance_fallback_no_boosts() {
        let mut blocks = vec![
            SessionBlock {
                session_id: "old".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                ttl: None,
                header: "## Session: old (2024-01-01T00:00:00Z)\n".to_string(),
                content: "Old".to_string(),
                preview: "Old".to_string(),
            },
            SessionBlock {
                session_id: "recent".to_string(),
                timestamp: "2024-02-13T00:00:00Z".to_string(),
                ttl: None,
                header: "## Session: recent (2024-02-13T00:00:00Z)\n".to_string(),
                content: "Recent".to_string(),
                preview: "Recent".to_string(),
            },
        ];

        let boosts = HashMap::new(); // No boosts

        sort_by_importance(&mut blocks, &boosts, "test", None);

        // With no boosts, both get boost=0.0, so recency wins
        assert_eq!(blocks[0].session_id, "recent");
        assert_eq!(blocks[1].session_id, "old");
    }
}
