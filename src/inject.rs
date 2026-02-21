//! Progressive disclosure injection for MEMORY.md
//!
//! Provides compact/full injection modes with line budgets,
//! preference deduplication, and pack summarization.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use crate::extractor::knowledge::{
    parse_session_blocks, partition_by_expiry, reconstruct_blocks, strip_private_tags, SessionBlock,
};
use crate::hive::PackInstaller;

/// Build a lightweight context string from raw knowledge files (no LLM).
/// Used as fallback when context.md doesn't exist but knowledge files do.
/// Returns None if no knowledge files exist or all are empty/expired.
pub fn build_raw_context(project: &str, project_knowledge_dir: &Path) -> Option<String> {
    let read_and_filter = |path: &Path| -> String {
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let (preamble, blocks) = parse_session_blocks(&raw);
        let (active, _) = partition_by_expiry(blocks);
        reconstruct_blocks(&preamble, &active)
    };

    let decisions = read_and_filter(&project_knowledge_dir.join("decisions.md"));
    let solutions = read_and_filter(&project_knowledge_dir.join("solutions.md"));
    let patterns = read_and_filter(&project_knowledge_dir.join("patterns.md"));
    let bugs = read_and_filter(&project_knowledge_dir.join("bugs.md"));
    let insights = read_and_filter(&project_knowledge_dir.join("insights.md"));
    let questions = read_and_filter(&project_knowledge_dir.join("questions.md"));
    let procedures = read_and_filter(&project_knowledge_dir.join("procedures.md"));

    if decisions.trim().is_empty()
        && solutions.trim().is_empty()
        && patterns.trim().is_empty()
        && bugs.trim().is_empty()
        && insights.trim().is_empty()
        && questions.trim().is_empty()
        && procedures.trim().is_empty()
    {
        return None;
    }

    let mut out = format!("# {} - Project Context (raw, not synthesized)\n\n", project);

    if !decisions.trim().is_empty() {
        out.push_str(&decisions);
        out.push_str("\n\n");
    }
    if !solutions.trim().is_empty() {
        out.push_str(&solutions);
        out.push_str("\n\n");
    }
    if !patterns.trim().is_empty() {
        out.push_str(&patterns);
        out.push_str("\n\n");
    }
    if !bugs.trim().is_empty() {
        out.push_str("## Known Bugs\n\n");
        out.push_str(&bugs);
        out.push_str("\n\n");
    }
    if !insights.trim().is_empty() {
        out.push_str("## Insights\n\n");
        out.push_str(&insights);
        out.push_str("\n\n");
    }
    if !questions.trim().is_empty() {
        out.push_str("## Open Questions\n\n");
        out.push_str(&questions);
        out.push_str("\n\n");
    }
    if !procedures.trim().is_empty() {
        out.push_str("## Workflows & Procedures\n\n");
        out.push_str(&procedures);
        out.push_str("\n\n");
    }

    Some(out)
}

/// Read non-preference global knowledge (decisions, solutions, patterns, bugs, insights)
/// from the _global directory. Returns None if nothing is stored there yet.
pub fn read_global_knowledge(memory_dir: &Path) -> Option<String> {
    let global_dir = memory_dir.join("knowledge").join(crate::config::GLOBAL_DIR);
    if !global_dir.exists() {
        return None;
    }

    let read_and_filter = |path: &Path| -> String {
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let (preamble, blocks) = parse_session_blocks(&raw);
        let (active, _) = partition_by_expiry(blocks);
        reconstruct_blocks(&preamble, &active)
    };

    let files = [
        ("decisions", "decisions.md"),
        ("patterns", "patterns.md"),
        ("solutions", "solutions.md"),
        ("insights", "insights.md"),
        ("bugs", "bugs.md"),
        ("procedures", "procedures.md"),
    ];

    let mut sections: Vec<String> = Vec::new();
    for (_category, filename) in &files {
        let content = read_and_filter(&global_dir.join(filename));
        if !content.trim().is_empty() {
            sections.push(content);
        }
    }

    if sections.is_empty() {
        return None;
    }

    Some(sections.join("\n\n"))
}

/// Line budget for compact MEMORY.md sections
pub const COMPACT_MAX_LINES: usize = 180;
pub const BUDGET_PREFERENCES: usize = 25;
pub const BUDGET_PROJECT: usize = 60;
pub const BUDGET_SHARED: usize = 40;
pub const BUDGET_PACKS: usize = 20;
pub const BUDGET_GUIDE: usize = 30;
pub const BUDGET_GLOBAL: usize = 40;

/// Load learned importance boosts from learning state (graceful fallback to empty HashMap)
pub fn load_importance_boosts(memory_dir: &Path, project: &str) -> HashMap<String, f32> {
    use crate::learning::progress;
    match progress::load_state(memory_dir, project) {
        Ok(state) => state.learned_parameters.importance_boosts,
        Err(_) => HashMap::new(), // Graceful degradation
    }
}

/// Return importance multiplier based on confidence level stored in the block.
fn confidence_multiplier(confidence: Option<&str>) -> f32 {
    match confidence {
        Some("high") => 1.2,
        Some("low") => 0.5,
        _ => 1.0,
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

/// FadeMem retention function: R = e^(-age_days / (strength * 30))
/// Returns a value in (0, 1] — never fully decays to 0.
fn fadem_retention(strength: Option<f32>, age_days: f32) -> f32 {
    let s = strength.unwrap_or(1.0).clamp(0.1, 5.0);
    (-age_days / (s * 30.0_f32))
        .exp()
        .clamp(f32::MIN_POSITIVE, 1.0)
}

/// Compute hybrid importance score: 40% recency + 60% learned boost, then multiply by FadeMem retention.
fn compute_importance_score(
    timestamp: &str,
    boost: f32,
    oldest_timestamp: &str,
    newest_timestamp: &str,
    strength: Option<f32>,
) -> f32 {
    use chrono::DateTime;

    // Parse timestamps
    let parse_ts = |ts: &str| -> Option<DateTime<chrono::Utc>> {
        DateTime::parse_from_rfc3339(ts)
            .ok()
            .map(|dt| dt.with_timezone(&chrono::Utc))
    };

    let block_ts = parse_ts(timestamp);

    let recency_score = match (
        block_ts,
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

    // Compute age in days for FadeMem
    let age_days = if let Some(ts) = parse_ts(timestamp) {
        let now = chrono::Utc::now();
        let age_secs = (now - ts).num_seconds().max(0) as f32;
        age_secs / 86400.0
    } else {
        0.0
    };

    // Hybrid: 40% recency, 60% learned boost — multiplied by FadeMem retention
    let base_score = (recency_score * 0.4) + (boost * 0.6);
    base_score * fadem_retention(strength, age_days)
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

        let score_a = compute_importance_score(&a.timestamp, boost_a, &oldest, &newest, a.strength)
            * confidence_multiplier(a.confidence.as_deref());
        let score_b = compute_importance_score(&b.timestamp, boost_b, &oldest, &newest, b.strength)
            * confidence_multiplier(b.confidence.as_deref());

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

// ── Smart injection ────────────────────────────────────────────────────

/// A single entry selected by smart inject for preview/inclusion.
#[derive(Clone, Debug)]
pub struct SmartEntry {
    pub category: String,
    pub session_id: String,
    pub preview: String,            // first 120 chars of content
    pub content: String,
    pub score: f32,                 // semantic relevance 0.0–1.0 (after recency decay)
    pub selected: bool,             // toggled in TUI preview
    pub timestamp: Option<String>,  // ISO-8601 from chunk metadata
}

/// Exponential recency decay: score × exp(-λ × days_since_written).
/// λ = 0.005 → half-life ≈ 139 days (gentle; avoids penalising mature knowledge).
/// Returns 1.0 if timestamp is absent or unparseable.
fn decay_factor(timestamp: &str) -> f32 {
    const LAMBDA: f64 = 0.005;
    let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp) else {
        return 1.0;
    };
    let days = (chrono::Utc::now() - dt.to_utc())
        .num_days()
        .max(0) as f64;
    (-LAMBDA * days).exp() as f32
}

/// Penalty multiplier for global (cross-project) entries to prefer project knowledge
/// when cosine scores are close.
const GLOBAL_PENALTY: f32 = 0.85;

impl SmartEntry {
    /// Rough token estimate (~4 chars per token).
    pub fn estimated_tokens(&self) -> usize {
        (self.content.len() / 4).max(1)
    }
}

/// Detect what the user is currently working on from git + CWD.
/// Returns a natural-language context signal for semantic search.
pub fn detect_work_context(project: &str) -> String {
    let mut parts = vec![format!("project: {}", project)];

    // Changed / staged files
    let changed = std::process::Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let staged = std::process::Command::new("git")
        .args(["diff", "--name-only", "--cached"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let all_changed: Vec<&str> = changed
        .lines()
        .chain(staged.lines())
        .filter(|l| !l.is_empty())
        .take(12)
        .collect();
    if !all_changed.is_empty() {
        parts.push(format!("changed files: {}", all_changed.join(", ")));
    }

    // Recent commit messages
    let log = std::process::Command::new("git")
        .args(["log", "--oneline", "-5"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    let msgs: Vec<&str> = log.lines().take(3).collect();
    if !msgs.is_empty() {
        parts.push(format!("recent commits: {}", msgs.join("; ")));
    }

    // Current branch
    let branch = std::process::Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(b) = branch {
        if b != "master" && b != "main" {
            parts.push(format!("branch: {}", b));
        }
    }

    // Augment with observations JSONL (files edited today, even without git)
    if let Some(home) = dirs::home_dir() {
        let obs_path = home
            .join("memory")
            .join("observations")
            .join(project)
            .join(format!("{}.jsonl", chrono::Utc::now().format("%Y-%m-%d")));
        if let Ok(content) = std::fs::read_to_string(&obs_path) {
            let mut obs_files: std::collections::HashSet<String> = std::collections::HashSet::new();
            for line in content.lines() {
                if let Ok(rec) = serde_json::from_str::<serde_json::Value>(line) {
                    if let Some(f) = rec.get("file").and_then(|v| v.as_str()) {
                        if !f.is_empty() {
                            obs_files.insert(f.to_string());
                        }
                    }
                }
            }
            let obs_vec: Vec<_> = obs_files.into_iter().take(8).collect();
            if !obs_vec.is_empty() {
                parts.push(format!("recently observed: {}", obs_vec.join(", ")));
            }
        }
    }

    parts.join(". ")
}

/// Load the embedding store and run semantic search against the work context.
/// Returns entries sorted by relevance, highest first.
/// Falls back to empty vec if no embedding index exists.
pub async fn smart_search(
    project: &str,
    memory_dir: &std::path::Path,
    signal: &str,
    top_k: usize,
    threshold: f32,
) -> crate::error::Result<Vec<SmartEntry>> {
    use crate::config::Config;
    use crate::embeddings::{provider::EmbeddingProvider, store::EmbeddingStore};

    let index_path = memory_dir
        .join("knowledge")
        .join(project)
        .join("embeddings.json");
    if !index_path.exists() {
        return Ok(vec![]);
    }

    let store = EmbeddingStore::load(&index_path)?;
    if store.chunks.is_empty() {
        return Ok(vec![]);
    }

    // Build embedding provider from stored config
    let config = Config::load(None)?;
    let embed_provider = EmbeddingProvider::from_config(&config);

    // Embed the search signal and use cosine similarity directly.
    // `search_text` / `hybrid_search` returns RRF scores (max ~0.033), which are
    // incompatible with the cosine-calibrated `threshold` (default 0.15).
    // Using raw cosine via `store.search()` keeps the threshold semantics consistent.
    let query_embedding = embed_provider.embed(signal).await?;
    let results = store.search(&query_embedding, top_k * 3);

    // Deduplicate by session_id (keep highest scoring chunk per session)
    let mut seen: std::collections::HashMap<String, SmartEntry> = std::collections::HashMap::new();
    let mut relaxed_candidates: Vec<(f32, crate::embeddings::EmbeddedChunk)> = Vec::new();
    for (raw_score, chunk) in results {
        let ts = chunk.metadata.timestamp.clone();
        let score = raw_score * decay_factor(&ts);
        if score < threshold {
            // Collect for adaptive fallback (score in [threshold×0.67, threshold))
            if score >= threshold * 0.67 {
                relaxed_candidates.push((score, chunk.clone()));
            }
            continue;
        }
        let session_id = chunk
            .metadata
            .session_id
            .clone()
            .unwrap_or_else(|| chunk.id.clone());
        let entry = seen.entry(session_id.clone()).or_insert(SmartEntry {
            category: chunk.metadata.category.clone(),
            session_id: session_id.clone(),
            preview: chunk
                .text
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .trim()
                .chars()
                .take(120)
                .collect(),
            content: chunk.text.clone(),
            score,
            selected: true,
            timestamp: Some(ts.clone()),
        });
        if score > entry.score {
            entry.score = score;
            entry.timestamp = Some(ts);
            entry.preview = chunk
                .text
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .trim()
                .chars()
                .take(120)
                .collect();
        }
    }

    // Adaptive threshold fallback: recovers short/conceptual entries (bugs, insights)
    // that score just below the strict threshold. Fires before BM25 to reduce noise.
    if seen.is_empty() {
        for (score, chunk) in relaxed_candidates {
            let ts = chunk.metadata.timestamp.clone();
            let session_id = chunk
                .metadata
                .session_id
                .clone()
                .unwrap_or_else(|| chunk.id.clone());
            seen.entry(session_id.clone()).or_insert(SmartEntry {
                category: chunk.metadata.category.clone(),
                session_id,
                preview: chunk
                    .text
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("")
                    .trim()
                    .chars()
                    .take(120)
                    .collect(),
                content: chunk.text.clone(),
                score,
                selected: true,
                timestamp: Some(ts),
            });
        }
    }

    // BM25 lexical fallback: fires ONLY when cosine returned 0 entries.
    // Acts as a last-resort retrieval when embeddings are absent or too dissimilar.
    // No extra API call — runs in-memory on the same store.
    if seen.is_empty() {
        let bm25_results = store.bm25_search(signal, top_k);
        for (_raw_score, chunk) in bm25_results {
            let session_id = chunk
                .metadata
                .session_id
                .clone()
                .unwrap_or_else(|| chunk.id.clone());
            seen.entry(session_id.clone()).or_insert(SmartEntry {
                category: chunk.metadata.category.clone(),
                session_id,
                preview: chunk
                    .text
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("")
                    .trim()
                    .chars()
                    .take(120)
                    .collect(),
                content: chunk.text.clone(),
                score: threshold, // assign threshold score so entries pass downstream filters
                selected: true,
                timestamp: Some(chunk.metadata.timestamp.clone()),
            });
        }
    }

    // Also search global knowledge store (if not already searching global)
    if project != crate::config::GLOBAL_DIR {
        let global_index_path = memory_dir
            .join("knowledge")
            .join(crate::config::GLOBAL_DIR)
            .join("embeddings.json");
        if global_index_path.exists() {
            if let Ok(global_store) = EmbeddingStore::load(&global_index_path) {
                if !global_store.chunks.is_empty() {
                    if let Ok(global_embedding) = embed_provider.embed(signal).await {
                        let global_results = global_store.search(&global_embedding, top_k * 2);
                        for (score, chunk) in global_results {
                            if score < threshold {
                                continue;
                            }
                            let raw_id = chunk
                                .metadata
                                .session_id
                                .clone()
                                .unwrap_or_else(|| chunk.id.clone());
                            // Prefix with "global:" to avoid collisions with project session IDs
                            let key = format!("global:{}", raw_id);
                            let first_line = chunk
                                .text
                                .lines()
                                .find(|l| !l.trim().is_empty())
                                .unwrap_or("")
                                .trim()
                                .chars()
                                .take(120)
                                .collect::<String>();
                            let global_ts = chunk.metadata.timestamp.clone();
                            let global_score = score * decay_factor(&global_ts) * GLOBAL_PENALTY;
                            let entry = seen.entry(key.clone()).or_insert(SmartEntry {
                                category: chunk.metadata.category.clone(),
                                session_id: key,
                                preview: first_line.clone(),
                                content: format!("[Global] {}", chunk.text.trim()),
                                score: global_score,
                                selected: true,
                                timestamp: Some(global_ts.clone()),
                            });
                            if global_score > entry.score {
                                entry.score = global_score;
                                entry.timestamp = Some(global_ts);
                                entry.preview = first_line;
                            }
                        }
                    }
                }
            }
        }
    }

    let mut entries: Vec<SmartEntry> = seen.into_values().collect();
    entries.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    entries.truncate(top_k);

    Ok(entries)
}

/// Sync wrapper for smart_search.
pub fn smart_search_sync(
    project: &str,
    memory_dir: &std::path::Path,
    signal: &str,
    top_k: usize,
    threshold: f32,
) -> crate::error::Result<Vec<SmartEntry>> {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime")
        .block_on(smart_search(project, memory_dir, signal, top_k, threshold))
}

/// Build a smart MEMORY.md from selected entries + standard header/footer.
pub fn format_smart_memory(
    project: &str,
    signal: &str,
    entries: &[SmartEntry],
    token_budget: usize,
    memory_dir: &std::path::Path,
) -> crate::error::Result<String> {
    let selected: Vec<&SmartEntry> = entries.iter().filter(|e| e.selected).collect();
    let total_tokens: usize = selected.iter().map(|e| e.estimated_tokens()).sum();

    let mut out = String::new();
    out.push_str("# Project Memory (auto-injected by engram)\n\n");
    out.push_str(&format!(
        "<!-- Smart inject: {}/{} entries · ~{} tokens · signal: {} -->\n\n",
        selected.len(),
        entries.len(),
        total_tokens,
        &signal[..signal.len().min(80)]
    ));
    out.push_str(&format!("## Project: {}\n\n", project));

    // Grouped by category
    let categories = [
        "decisions",
        "solutions",
        "patterns",
        "bugs",
        "insights",
        "questions",
        "procedures",
        "context",
    ];
    for cat in &categories {
        let cat_entries: Vec<&&SmartEntry> =
            selected.iter().filter(|e| e.category == *cat).collect();
        if cat_entries.is_empty() {
            continue;
        }

        out.push_str(&format!("### {}\n\n", cat));
        let mut tokens_used = 0;
        for e in cat_entries {
            let entry_tokens = e.estimated_tokens();
            if tokens_used + entry_tokens > token_budget {
                break;
            }
            out.push_str(&format!(
                "<!-- relevance: {:.0}% -->\n{}\n\n",
                e.score * 100.0,
                e.content.trim()
            ));
            tokens_used += entry_tokens;
        }
    }

    out.push_str("---\n\n");
    out.push_str(&retrieval_guide());

    // Append compact prefs + pack summary
    let global_prefs_path = memory_dir
        .join("knowledge")
        .join("_global")
        .join("preferences.md");
    let raw_prefs = std::fs::read_to_string(&global_prefs_path).ok();
    if let Some(ref p) = raw_prefs {
        let prefs = compact_preferences(p, memory_dir, project);
        if !prefs.is_empty() {
            out.push_str("\n## User Preferences (consolidated)\n\n");
            out.push_str(&trim_to_budget(&prefs, BUDGET_PREFERENCES));
            out.push_str("\n\n---\n\n");
        }
    }

    // Track inject event
    {
        use crate::analytics::{EventTracker, EventType, UsageEvent};
        let tracker = EventTracker::new(memory_dir);
        let _ = tracker.track(UsageEvent {
            timestamp: chrono::Utc::now(),
            event_type: EventType::Inject,
            project: project.to_string(),
            query: Some(signal[..signal.len().min(80)].to_string()),
            category: None,
            results_count: Some(selected.len()),
            session_id: None,
            tokens_consumed: Some(total_tokens as u64),
        });
    }

    Ok(strip_private_tags(&out))
}

/// Build the retrieval guide footer for MEMORY.md.
pub fn retrieval_guide() -> String {
    r#"## Retrieving More Context

For detailed knowledge beyond this summary:
- `engram lookup <project> <query>` - find specific knowledge entries
- `engram search <query>` - full-text search across all knowledge
- `engram recall <project>` - full project context with pack knowledge
- `engram search-semantic <query>` - semantic/vector search

### Progressive Recall (MCP / long-context pattern)

For infinite-context workflows use the hierarchy: **index → recall → search**.
1. `engram inject --lines 360` - scale compact budget (2× default) for large context windows
2. Via MCP: call `index` tool (~100 tokens) to get a map, then `recall(session_ids=[...])` for specifics
3. `engram inject --smart` - semantic search injects only what's relevant to current git context

### Maintenance

- `engram drain <project>` - bulk-promote all inbox entries to knowledge files
- `engram consolidate <project>` - detect and merge near-duplicate entries
- `engram forget <project> --stale 90d` - prune entries not accessed in 90 days

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
    build_compact_memory_with_budget(
        project_name,
        context_content,
        raw_preferences,
        raw_shared,
        memory_dir,
        None,
    )
}

/// Build compact MEMORY.md with an optional line budget override.
/// When `line_budget` is provided, all section budgets scale proportionally
/// relative to the default COMPACT_MAX_LINES (180). Example: `line_budget=360`
/// doubles every section budget, giving 2x more context for long-context models.
pub fn build_compact_memory_with_budget(
    project_name: &str,
    context_content: &str,
    raw_preferences: &Option<String>,
    raw_shared: &Option<String>,
    memory_dir: &Path,
    line_budget: Option<usize>,
) -> crate::Result<String> {
    // Scale section budgets proportionally when a custom line budget is requested
    let scale = line_budget
        .map(|n| n as f64 / COMPACT_MAX_LINES as f64)
        .unwrap_or(1.0_f64);
    let scaled = |base: usize| -> usize { ((base as f64 * scale).round() as usize).max(1) };

    let mut combined = String::new();
    combined.push_str("# Project Memory (auto-injected by engram)\n\n");
    let mode_hint = if let Some(budget) = line_budget {
        format!(
            "<!-- Compact mode ({}L budget): run `engram inject --full` for complete dump -->\n\n",
            budget
        )
    } else {
        "<!-- Compact mode: run `engram inject --full` for complete dump -->\n\n".to_string()
    };
    combined.push_str(&mode_hint);

    // 1. Project context first (most valuable)
    combined.push_str(&format!("## Project: {}\n\n", project_name));
    combined.push_str(&trim_to_budget(context_content, scaled(BUDGET_PROJECT)));
    combined.push_str("\n\n---\n\n");

    // 2. Consolidated preferences (deduplicated, importance-sorted)
    if let Some(raw_prefs) = raw_preferences {
        let prefs = compact_preferences(raw_prefs, memory_dir, project_name);
        if !prefs.is_empty() {
            combined.push_str("## User Preferences (consolidated)\n\n");
            combined.push_str(&trim_to_budget(&prefs, scaled(BUDGET_PREFERENCES)));
            combined.push_str("\n\n---\n\n");
        }
    }

    // 3. Shared memory (trimmed to budget, importance-prioritized)
    if let Some(raw_sh) = raw_shared {
        let shared = compact_shared(raw_sh, scaled(BUDGET_SHARED), memory_dir, project_name);
        if !shared.is_empty() {
            combined.push_str("## Shared Knowledge\n\n");
            combined.push_str(&shared);
            combined.push_str("\n\n---\n\n");
        }
    }

    // 4. Global knowledge (cross-project patterns, decisions, solutions)
    let global_knowledge = read_global_knowledge(memory_dir);
    if let Some(ref gk) = global_knowledge {
        combined.push_str("## Global Knowledge\n\n");
        combined.push_str(&trim_to_budget(gk, scaled(BUDGET_GLOBAL)));
        combined.push_str("\n\n---\n\n");
    }

    // 5. Pack index (summary, not full content)
    let pack_summary = compact_pack_summary(memory_dir)?;
    if !pack_summary.is_empty() {
        combined.push_str("## Installed Packs\n\n");
        combined.push_str(&pack_summary);
        combined.push('\n');
    }

    // 6. Retrieval guide footer
    combined.push_str(&retrieval_guide());

    Ok(strip_private_tags(&combined))
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

    // Append global knowledge (decisions, solutions, patterns, etc.)
    let global_knowledge = read_global_knowledge(memory_dir);
    if let Some(gk) = global_knowledge {
        combined.push_str("\n\n---\n\n## Global Knowledge\n\n");
        combined.push_str(&gk);
    }

    let pack_content = crate::hive::get_installed_pack_knowledge(memory_dir)?;
    if !pack_content.is_empty() {
        combined.push_str("\n\n---\n\n## Installed Pack Knowledge\n\n");
        combined.push_str(&pack_content);
    }

    Ok(strip_private_tags(&combined))
}

#[cfg(test)]
mod fadem_tests {
    use super::fadem_retention;

    #[test]
    fn test_fadem_retention_fresh_block() {
        let r = fadem_retention(Some(1.0), 0.0);
        assert!(
            (r - 1.0).abs() < 0.001,
            "Fresh block should have retention ≈ 1.0, got {}",
            r
        );
    }

    #[test]
    fn test_fadem_retention_one_period() {
        // e^(-30/(1.0*30)) = e^-1 ≈ 0.368
        let r = fadem_retention(Some(1.0), 30.0);
        assert!((r - 0.368).abs() < 0.01, "Expected ≈0.368, got {}", r);
    }

    #[test]
    fn test_fadem_retention_high_strength_decays_slower() {
        let r_high = fadem_retention(Some(5.0), 30.0);
        let r_low = fadem_retention(Some(1.0), 30.0);
        assert!(
            r_high > 0.8,
            "High strength should retain well, got {}",
            r_high
        );
        assert!(
            r_high > r_low,
            "High strength decays slower than low strength"
        );
    }

    #[test]
    fn test_fadem_retention_never_zero() {
        let r = fadem_retention(None, 9999.0);
        assert!(r > 0.0, "Retention should never be exactly zero");
    }
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
        // Fresh block (age_days ≈ 0) so FadeMem ≈ 1.0
        let now = chrono::Utc::now();
        let ts = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let oldest = "2024-02-01T00:00:00Z";
        let newest = ts.as_str();
        let score = compute_importance_score(&ts, 0.9, oldest, newest, None);
        // recency ≈ 1.0 (newest), high boost 0.9 → base ≈ 0.94, fadem ≈ 1.0
        assert!(score > 0.8, "score was {}", score);
    }

    #[test]
    fn test_compute_importance_score_old_high_boost() {
        // Old block with high boost — FadeMem reduces it somewhat
        let score = compute_importance_score(
            "2024-02-01T00:00:00Z",
            0.9,
            "2024-02-01T00:00:00Z",
            "2024-02-13T12:00:00Z",
            None,
        );
        // base = 0.54, fadem significantly reduces old blocks
        // just check it's positive and less than 0.54
        assert!(score > 0.0 && score <= 0.54 + 0.01, "score was {}", score);
    }

    #[test]
    fn test_compute_importance_score_recent_low_boost() {
        let now = chrono::Utc::now();
        let ts = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let oldest = "2024-02-01T00:00:00Z";
        let newest = ts.as_str();
        let score = compute_importance_score(&ts, 0.1, oldest, newest, None);
        // fresh block, low boost → base ≈ 0.46
        assert!(score > 0.3, "score was {}", score);
    }

    #[test]
    fn test_compute_importance_score_malformed_timestamp() {
        let score = compute_importance_score(
            "invalid-timestamp",
            0.8,
            "2024-02-01T00:00:00Z",
            "2024-02-13T12:00:00Z",
            None,
        );
        // Fallback recency=0.5, age_days=0 → fadem=1.0 → base=0.68
        assert!((score - 0.68).abs() < 0.02, "score was {}", score);
    }

    #[test]
    fn test_sort_by_importance_prioritizes_high_boost() {
        // Use recent timestamps so FadeMem doesn't zero out old entries
        let mut blocks = vec![
            SessionBlock {
                session_id: "recent-important".to_string(),
                timestamp: "2024-02-12T00:00:00Z".to_string(),
                ttl: None,
                confidence: None,
                strength: Some(5.0), // Very high strength keeps it alive
                access_count: None,
                header: "## Session: recent-important (2024-02-12T00:00:00Z)\n".to_string(),
                content: "High-value knowledge".to_string(),
                preview: "High-value".to_string(),
            },
            SessionBlock {
                session_id: "recent-unimportant".to_string(),
                timestamp: "2024-02-13T00:00:00Z".to_string(),
                ttl: None,
                confidence: None,
                strength: None,
                access_count: None,
                header: "## Session: recent-unimportant (2024-02-13T00:00:00Z)\n".to_string(),
                content: "Low-value recent".to_string(),
                preview: "Low-value".to_string(),
            },
        ];

        let mut boosts = HashMap::new();
        boosts.insert("recent-important".to_string(), 0.9); // High boost
        boosts.insert("recent-unimportant".to_string(), 0.1); // Low boost

        sort_by_importance(&mut blocks, &boosts, "test", None);

        // High boost should rank first
        assert_eq!(blocks[0].session_id, "recent-important");
        assert_eq!(blocks[1].session_id, "recent-unimportant");
    }

    #[test]
    fn test_sort_by_importance_fallback_no_boosts() {
        let mut blocks = vec![
            SessionBlock {
                session_id: "old".to_string(),
                timestamp: "2024-01-01T00:00:00Z".to_string(),
                ttl: None,
                confidence: None,
                strength: None,
                access_count: None,
                header: "## Session: old (2024-01-01T00:00:00Z)\n".to_string(),
                content: "Old".to_string(),
                preview: "Old".to_string(),
            },
            SessionBlock {
                session_id: "recent".to_string(),
                timestamp: "2024-02-13T00:00:00Z".to_string(),
                ttl: None,
                confidence: None,
                strength: None,
                access_count: None,
                header: "## Session: recent (2024-02-13T00:00:00Z)\n".to_string(),
                content: "Recent".to_string(),
                preview: "Recent".to_string(),
            },
        ];

        let boosts = HashMap::new(); // No boosts

        sort_by_importance(&mut blocks, &boosts, "test", None);

        // With no boosts and FadeMem, very old blocks should rank lower than newer ones
        // (recency wins at equal boost=0.0, even with FadeMem factored in)
        assert_eq!(blocks[0].session_id, "recent");
        assert_eq!(blocks[1].session_id, "old");
    }
}
