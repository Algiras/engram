//! Progressive disclosure injection for MEMORY.md
//!
//! Provides compact/full injection modes with line budgets,
//! preference deduplication, and pack summarization.

use std::collections::BTreeMap;
use std::path::Path;

use crate::extractor::knowledge::{parse_session_blocks, partition_by_expiry, reconstruct_blocks};
use crate::hive::PackInstaller;

/// Line budget for compact MEMORY.md sections
pub const COMPACT_MAX_LINES: usize = 180;
pub const BUDGET_PREFERENCES: usize = 25;
pub const BUDGET_PROJECT: usize = 60;
pub const BUDGET_SHARED: usize = 40;
pub const BUDGET_PACKS: usize = 20;
pub const BUDGET_GUIDE: usize = 15;

/// Deduplicate preferences from multiple session blocks into consolidated bullets.
/// Extracts `**Key:** Value` patterns, groups by key, and keeps unique values.
pub fn compact_preferences(raw_prefs: &str) -> String {
    let (_preamble, blocks) = parse_session_blocks(raw_prefs);
    let (active, _) = partition_by_expiry(blocks);

    if active.is_empty() {
        return String::new();
    }

    // Extract **Key:** Value pairs from all blocks
    let bold_re = regex::Regex::new(r"\*\*([^*]+):\*\*\s*(.+)").unwrap();

    let mut merged: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for block in &active {
        for line in block.content.lines() {
            let line = line.trim();
            // Match top-level bullets: `* **Key:** Value` or `- **Key:** Value`
            if let Some(caps) = bold_re.captures(line) {
                let key = normalize_pref_key(caps.get(1).unwrap().as_str().trim());
                let value = caps.get(2).unwrap().as_str().trim().to_string();
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

/// Trim shared memory blocks to fit within a line budget, keeping most recent.
pub fn compact_shared(raw_shared: &str, max_lines: usize) -> String {
    let (preamble, blocks) = parse_session_blocks(raw_shared);
    let (mut active, _) = partition_by_expiry(blocks);

    if active.is_empty() {
        return String::new();
    }

    // Sort by timestamp descending (most recent first)
    active.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

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
        out.push_str(&format!(
            "\n_(Showing {} of {} entries. Run `claude-memory recall` for all.)_\n",
            included, total
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
    out.push_str("\n\n_(Truncated. Run `claude-memory recall` for full content.)_");
    out
}

/// Build the retrieval guide footer for MEMORY.md.
pub fn retrieval_guide() -> String {
    r#"## Retrieving More Context

For detailed knowledge beyond this summary:
- `claude-memory lookup <project> <query>` - find specific knowledge entries
- `claude-memory search <query>` - full-text search across all knowledge
- `claude-memory recall <project>` - full project context with pack knowledge
- `claude-memory search-semantic <query>` - semantic/vector search
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
    combined.push_str("# Project Memory (auto-injected by claude-memory)\n\n");
    combined
        .push_str("<!-- Compact mode: run `claude-memory inject --full` for complete dump -->\n\n");

    // 1. Project context first (most valuable)
    combined.push_str(&format!("## Project: {}\n\n", project_name));
    combined.push_str(&trim_to_budget(context_content, BUDGET_PROJECT));
    combined.push_str("\n\n---\n\n");

    // 2. Consolidated preferences (deduplicated)
    if let Some(raw_prefs) = raw_preferences {
        let prefs = compact_preferences(raw_prefs);
        if !prefs.is_empty() {
            combined.push_str("## User Preferences (consolidated)\n\n");
            combined.push_str(&trim_to_budget(&prefs, BUDGET_PREFERENCES));
            combined.push_str("\n\n---\n\n");
        }
    }

    // 3. Shared memory (trimmed to budget, most recent)
    if let Some(raw_sh) = raw_shared {
        let shared = compact_shared(raw_sh, BUDGET_SHARED);
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
    combined.push_str("# Project Memory (auto-injected by claude-memory)\n\n");
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
