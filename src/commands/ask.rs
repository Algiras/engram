use colored::Colorize;

use crate::analytics::tracker::{EventTracker, EventType, UsageEvent};
use crate::config::{Config, CATEGORIES};
use crate::error::{MemoryError, Result};
use crate::extractor::knowledge::{
    find_sessions_by_topic, parse_session_blocks, partition_by_expiry, strip_private_tags,
};
use crate::inject::{build_raw_context, smart_search_sync, SmartEntry};
use crate::llm::client::LlmClient;
use crate::llm::prompts::{ask_prompt, SYSTEM_QA_ASSISTANT};

/// Categories that benefit from recursive (index→select→fetch) retrieval.
pub const RECURSIVE_CATEGORIES: &[&str] = &["decisions", "patterns", "procedures"];
/// Categories that benefit from HyDE+semantic retrieval.
pub const STANDARD_CATEGORIES: &[&str] = &["insights", "bugs", "solutions"];

/// Strip a leading `category:` prefix from an ID returned by the LLM selector.
/// The index format is `[category:session_id]`, so the model often echoes the prefix.
pub fn strip_category_prefix(id: &str) -> String {
    for cat in CATEGORIES {
        if let Some(rest) = id.strip_prefix(&format!("{}:", cat)) {
            // Recursively strip in case of double-prefix like "decisions:decisions:uuid"
            return strip_category_prefix(rest);
        }
    }
    id.to_string()
}

/// Parse comma-separated IDs from LLM selector response, normalising away category prefixes.
pub fn parse_selected_ids(response: &str) -> Vec<String> {
    response
        .split(',')
        .map(|s| strip_category_prefix(s.trim()))
        .filter(|s| !s.is_empty())
        .take(5)
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn cmd_ask(
    config: &Config,
    query: &str,
    project: &str,
    top_k: usize,
    threshold: f32,
    verbose: bool,
    use_graph: bool,
    concise: bool,
) -> Result<()> {
    // 1. HyDE: generate a hypothetical answer to improve semantic search signal
    // Uses a small LLM call to produce a document that "would answer" the query,
    // then embeds query + hypothetical together for better recall.
    let search_signal: String = {
        let client = LlmClient::new(&config.llm);
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;
        match rt.block_on(client.chat(
            crate::llm::prompts::SYSTEM_HYDE_GENERATOR,
            &crate::llm::prompts::hyde_prompt(query),
        )) {
            Ok(hypothetical) => {
                if verbose {
                    eprintln!(
                        "{} HyDE: {}",
                        "Ask:".cyan(),
                        hypothetical.trim().chars().take(100).collect::<String>()
                    );
                }
                format!("{}\n\nQuery: {}", hypothetical.trim(), query)
            }
            Err(_) => query.to_string(), // fallback to raw query
        }
    };

    // 2. Semantic search using HyDE-enhanced signal
    let mut entries: Vec<SmartEntry> = smart_search_sync(
        project,
        &config.memory_dir,
        &search_signal,
        top_k,
        threshold,
    )
    .unwrap_or_else(|_| vec![]);
    let used_semantic = !entries.is_empty();

    // 1b. Graph-augmented retrieval (opt-in via --use-graph)
    // For each concept in the knowledge graph that matches the query, retrieve
    // semantically similar entries for its 2-hop graph neighbors.
    if use_graph {
        let graph_path = config
            .memory_dir
            .join("knowledge")
            .join(project)
            .join("graph.json");
        if graph_path.exists() {
            if let Ok(graph) = crate::graph::KnowledgeGraph::load(&graph_path) {
                let query_lower = query.to_lowercase();
                let mut augmented_queries: Vec<String> = Vec::new();

                // Find concepts mentioned in the query
                for concept in graph.concepts.values() {
                    if query_lower.contains(&concept.name.to_lowercase()) {
                        // BFS 2-hop neighbors
                        let related = crate::graph::query::find_related(&graph, &concept.id, 2);
                        for (related_id, depth) in related {
                            if let Some(rel_concept) = graph.concepts.get(&related_id) {
                                // Use neighbor name as an additional search query
                                augmented_queries.push(rel_concept.name.clone());
                                if verbose {
                                    eprintln!(
                                        "{} graph: {} →[{}]→ {}",
                                        "Ask:".cyan(),
                                        concept.name,
                                        depth,
                                        rel_concept.name
                                    );
                                }
                            }
                        }
                    }
                }

                // Fetch entries for each augmented query (dedup by session_id)
                for aug_query in augmented_queries.iter().take(4) {
                    let aug_entries =
                        smart_search_sync(project, &config.memory_dir, aug_query, 2, threshold)
                            .unwrap_or_default();
                    for entry in aug_entries {
                        if !entries.iter().any(|e| e.session_id == entry.session_id) {
                            entries.push(SmartEntry {
                                score: entry.score * 0.85, // slight discount for graph-sourced
                                ..entry
                            });
                        }
                    }
                }
            }
        } else if verbose {
            eprintln!(
                "{} No graph.json found. Run 'engram graph build {}' first.",
                "Ask:".yellow(),
                project
            );
        }
    }

    // 2. Lexical fallback if semantic returned fewer than 2 results
    if entries.len() < 2 {
        let knowledge_dir = config.memory_dir.join("knowledge").join(project);
        for cat in crate::config::CATEGORIES {
            let path = knowledge_dir.join(format!("{}.md", cat));
            if !path.exists() {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            let matching_ids = find_sessions_by_topic(&content, query);
            if matching_ids.is_empty() {
                continue;
            }
            let (_preamble, blocks) = parse_session_blocks(&content);
            let (active, _) = partition_by_expiry(blocks);
            for block in active {
                if matching_ids.contains(&block.session_id)
                    && !entries.iter().any(|e| e.session_id == block.session_id)
                {
                    entries.push(SmartEntry {
                        category: cat.to_string(),
                        session_id: block.session_id,
                        preview: block.preview,
                        content: block.content,
                        score: 0.5,
                        selected: true,
                    });
                }
            }
        }
    }

    // 3. Build context string (raw context.md fallback if still empty)
    let context_str = if !entries.is_empty() {
        entries
            .iter()
            .take(top_k)
            .map(|e| format!("[{}:{}]\n{}", e.category, e.session_id, e.content.trim()))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n")
    } else {
        let knowledge_dir = config.memory_dir.join("knowledge").join(project);
        match build_raw_context(project, &knowledge_dir) {
            Some(ctx) => ctx,
            None => {
                println!(
                    "{} No knowledge found for '{}'. Run 'engram ingest --project {}' first.",
                    "Not found:".yellow(),
                    project,
                    project
                );
                return Ok(());
            }
        }
    };

    let context_str = strip_private_tags(&context_str);

    if verbose {
        eprintln!(
            "{} {} entries (semantic: {})",
            "Ask:".cyan(),
            entries.len().max(1),
            used_semantic
        );
    }

    // 4. LLM synthesis
    let client = LlmClient::new(&config.llm);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;
    let (system, prompt) = if concise {
        (
            crate::llm::prompts::SYSTEM_QA_CONCISE,
            crate::llm::prompts::ask_concise_prompt(query, &context_str),
        )
    } else {
        (SYSTEM_QA_ASSISTANT, ask_prompt(query, &context_str))
    };
    let answer = rt.block_on(async { client.chat(system, &prompt).await })?;

    // 5. Output
    println!("{}", answer);
    if !used_semantic {
        println!(
            "\n{} Run 'engram embed {}' to enable semantic search.",
            "Hint:".dimmed(),
            project
        );
    }

    // 6. Analytics (fire-and-forget)
    let tracker = EventTracker::new(&config.memory_dir);
    let _ = tracker.track(UsageEvent {
        timestamp: chrono::Utc::now(),
        event_type: EventType::Ask,
        project: project.to_string(),
        query: Some(query.to_string()),
        category: None,
        results_count: Some(entries.len()),
        session_id: None,
        tokens_consumed: None,
    });

    Ok(())
}

/// Build a compact index string for knowledge entries in a project.
/// Format: "[category:session_id] first 100 chars of content\n"
/// Pass `CATEGORIES` to include all categories, or a subset like `RECURSIVE_CATEGORIES`.
pub fn build_project_index(
    project: &str,
    memory_dir: &std::path::Path,
    categories: &[&str],
) -> String {
    let knowledge_dir = memory_dir.join("knowledge").join(project);
    let mut lines = Vec::new();

    for cat in categories {
        let path = knowledge_dir.join(format!("{}.md", cat));
        if !path.exists() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (_preamble, blocks) = parse_session_blocks(&content);
        let (active, _) = partition_by_expiry(blocks);
        for block in active {
            let preview: String = block
                .content
                .trim()
                .chars()
                .take(100)
                .collect::<String>()
                .replace('\n', " ");
            lines.push(format!("[{}:{}] {}", cat, block.session_id, preview));
        }
    }

    lines.join("\n")
}

/// Fetch full content for a list of session IDs across all knowledge category files.
pub fn fetch_entries_by_ids(
    project: &str,
    memory_dir: &std::path::Path,
    ids: &[String],
) -> Vec<SmartEntry> {
    let knowledge_dir = memory_dir.join("knowledge").join(project);
    let mut results = Vec::new();

    for cat in CATEGORIES {
        let path = knowledge_dir.join(format!("{}.md", cat));
        if !path.exists() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (_preamble, blocks) = parse_session_blocks(&content);
        let (active, _) = partition_by_expiry(blocks);
        for block in active {
            if ids.contains(&block.session_id) {
                results.push(SmartEntry {
                    category: cat.to_string(),
                    session_id: block.session_id,
                    preview: block.preview,
                    content: block.content,
                    score: 1.0,
                    selected: true,
                });
            }
        }
    }

    results
}

/// Recursive retrieval (RLM-style): build index → LLM selects IDs → fetch full entries → answer.
/// Falls back to regular cmd_ask if selection fails or returns no valid entries.
pub fn cmd_ask_recursive(
    config: &Config,
    query: &str,
    project: &str,
    verbose: bool,
    concise: bool,
) -> Result<()> {
    use crate::llm::prompts::{recursive_select_prompt, SYSTEM_RECURSIVE_SELECTOR};

    // 1. Build compact index (all categories)
    let index = build_project_index(project, &config.memory_dir, CATEGORIES);
    if index.is_empty() {
        // No knowledge — fall through to regular ask which will print a helpful message
        return cmd_ask(config, query, project, 12, 0.15, verbose, false, concise);
    }

    if verbose {
        let line_count = index.lines().count();
        eprintln!("{} index: {} entries", "Recursive:".cyan(), line_count);
    }

    // 2. LLM Step 1 — selection
    let client = LlmClient::new(&config.llm);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    let selection_response = rt.block_on(client.chat(
        SYSTEM_RECURSIVE_SELECTOR,
        &recursive_select_prompt(query, &index),
    ));

    let selected_ids: Vec<String> = match selection_response {
        Ok(resp) => {
            let ids = parse_selected_ids(&resp);
            if verbose {
                eprintln!("{} selected IDs: {:?}", "Recursive:".cyan(), ids);
            }
            ids
        }
        Err(_) => Vec::new(),
    };

    // 3. Fetch full content for selected IDs
    let entries = if !selected_ids.is_empty() {
        fetch_entries_by_ids(project, &config.memory_dir, &selected_ids)
    } else {
        Vec::new()
    };

    // 4. Graceful degradation — fall back to regular ask if nothing was selected/found
    if entries.is_empty() {
        if verbose {
            eprintln!(
                "{} no entries found for selection, falling back to regular ask",
                "Recursive:".yellow()
            );
        }
        return cmd_ask(config, query, project, 12, 0.15, verbose, false, concise);
    }

    // 5. Build context and answer
    let context_str = entries
        .iter()
        .map(|e| format!("[{}:{}]\n{}", e.category, e.session_id, e.content.trim()))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let context_str = strip_private_tags(&context_str);

    let (system, prompt) = if concise {
        (
            crate::llm::prompts::SYSTEM_QA_CONCISE,
            crate::llm::prompts::ask_concise_prompt(query, &context_str),
        )
    } else {
        (SYSTEM_QA_ASSISTANT, ask_prompt(query, &context_str))
    };
    let answer = rt.block_on(async { client.chat(system, &prompt).await })?;

    println!("{}", answer);

    // 6. Analytics
    let tracker = EventTracker::new(&config.memory_dir);
    let _ = tracker.track(UsageEvent {
        timestamp: chrono::Utc::now(),
        event_type: EventType::Ask,
        project: project.to_string(),
        query: Some(query.to_string()),
        category: None,
        results_count: Some(entries.len()),
        session_id: None,
        tokens_consumed: None,
    });

    Ok(())
}

/// Hybrid retrieval: recursive arm for decisions/patterns/procedures,
/// HyDE+semantic arm for insights/bugs/solutions. Both arms run and results are merged.
#[allow(clippy::too_many_arguments)]
pub fn cmd_ask_hybrid(
    config: &Config,
    query: &str,
    project: &str,
    top_k: usize,
    threshold: f32,
    verbose: bool,
    concise: bool,
) -> Result<()> {
    use crate::llm::prompts::{recursive_select_prompt, SYSTEM_RECURSIVE_SELECTOR};

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;
    let client = LlmClient::new(&config.llm);

    // ── Arm 1: Recursive over decisions/patterns/procedures ──────────────────
    let index = build_project_index(project, &config.memory_dir, RECURSIVE_CATEGORIES);
    let mut rec_entries: Vec<SmartEntry> = Vec::new();

    if !index.is_empty() {
        if verbose {
            let n = index.lines().count();
            eprintln!("{} recursive index: {} entries", "Hybrid:".cyan(), n);
        }
        let selected_ids: Vec<String> = rt
            .block_on(client.chat(
                SYSTEM_RECURSIVE_SELECTOR,
                &recursive_select_prompt(query, &index),
            ))
            .map(|resp| parse_selected_ids(&resp))
            .unwrap_or_default();

        if verbose {
            eprintln!("{} recursive selected IDs: {:?}", "Hybrid:".cyan(), selected_ids);
        }
        if !selected_ids.is_empty() {
            rec_entries = fetch_entries_by_ids(project, &config.memory_dir, &selected_ids);
        }
    }

    // ── Arm 2: HyDE+semantic over insights/bugs/solutions ───────────────────
    let search_signal: String = {
        match rt.block_on(client.chat(
            crate::llm::prompts::SYSTEM_HYDE_GENERATOR,
            &crate::llm::prompts::hyde_prompt(query),
        )) {
            Ok(hypothetical) => {
                if verbose {
                    eprintln!(
                        "{} HyDE: {}",
                        "Hybrid:".cyan(),
                        hypothetical.trim().chars().take(100).collect::<String>()
                    );
                }
                format!("{}\n\nQuery: {}", hypothetical.trim(), query)
            }
            Err(_) => query.to_string(),
        }
    };

    let mut std_entries: Vec<SmartEntry> =
        smart_search_sync(project, &config.memory_dir, &search_signal, top_k, threshold)
            .unwrap_or_default();

    // Lexical fallback for standard arm — covers all categories so no question is left empty
    if std_entries.len() < 2 {
        let knowledge_dir = config.memory_dir.join("knowledge").join(project);
        for cat in CATEGORIES {
            let path = knowledge_dir.join(format!("{}.md", cat));
            if !path.exists() {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            let matching_ids = find_sessions_by_topic(&content, query);
            if matching_ids.is_empty() {
                continue;
            }
            let (_preamble, blocks) = parse_session_blocks(&content);
            let (active, _) = partition_by_expiry(blocks);
            for block in active {
                if matching_ids.contains(&block.session_id)
                    && !std_entries.iter().any(|e| e.session_id == block.session_id)
                {
                    std_entries.push(SmartEntry {
                        category: cat.to_string(),
                        session_id: block.session_id,
                        preview: block.preview,
                        content: block.content,
                        score: 0.5,
                        selected: true,
                    });
                }
            }
        }
    }

    // ── Merge ────────────────────────────────────────────────────────────────
    let mut merged: Vec<SmartEntry> = rec_entries;
    for entry in std_entries {
        if !merged.iter().any(|e| e.session_id == entry.session_id) {
            merged.push(entry);
        }
    }

    if verbose {
        eprintln!(
            "{} merged {} entries (recursive + semantic)",
            "Hybrid:".cyan(),
            merged.len()
        );
    }

    // If nothing found, fall back to regular ask
    if merged.is_empty() {
        return cmd_ask(config, query, project, top_k, threshold, verbose, false, concise);
    }

    // ── Synthesis ────────────────────────────────────────────────────────────
    let context_str = merged
        .iter()
        .take(top_k)
        .map(|e| format!("[{}:{}]\n{}", e.category, e.session_id, e.content.trim()))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let context_str = strip_private_tags(&context_str);

    let (system, prompt) = if concise {
        (
            crate::llm::prompts::SYSTEM_QA_CONCISE,
            crate::llm::prompts::ask_concise_prompt(query, &context_str),
        )
    } else {
        (SYSTEM_QA_ASSISTANT, ask_prompt(query, &context_str))
    };
    let answer = rt.block_on(async { client.chat(system, &prompt).await })?;

    println!("{}", answer);

    // Analytics
    let tracker = EventTracker::new(&config.memory_dir);
    let _ = tracker.track(UsageEvent {
        timestamp: chrono::Utc::now(),
        event_type: EventType::Ask,
        project: project.to_string(),
        query: Some(query.to_string()),
        category: None,
        results_count: Some(merged.len()),
        session_id: None,
        tokens_consumed: None,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── strip_category_prefix ───────────────────────────────────────────────

    #[test]
    fn test_strip_no_prefix() {
        assert_eq!(strip_category_prefix("abc-123"), "abc-123");
    }

    #[test]
    fn test_strip_single_prefix() {
        assert_eq!(strip_category_prefix("decisions:abc-123"), "abc-123");
        assert_eq!(strip_category_prefix("solutions:xyz-456"), "xyz-456");
        assert_eq!(strip_category_prefix("patterns:uuid-789"), "uuid-789");
    }

    #[test]
    fn test_strip_double_prefix() {
        assert_eq!(
            strip_category_prefix("decisions:decisions:abc-123"),
            "abc-123"
        );
    }

    #[test]
    fn test_strip_unknown_prefix_unchanged() {
        // Unknown prefix should be left as-is
        assert_eq!(strip_category_prefix("unknown:abc-123"), "unknown:abc-123");
    }

    // ── parse_selected_ids ──────────────────────────────────────────────────

    #[test]
    fn test_parse_bare_ids() {
        let ids = parse_selected_ids("abc-1, def-2, ghi-3");
        assert_eq!(ids, vec!["abc-1", "def-2", "ghi-3"]);
    }

    #[test]
    fn test_parse_ids_with_category_prefix() {
        let ids = parse_selected_ids("decisions:abc-1, solutions:def-2");
        assert_eq!(ids, vec!["abc-1", "def-2"]);
    }

    #[test]
    fn test_parse_ids_capped_at_five() {
        let ids =
            parse_selected_ids("a, b, c, d, e, f, g");
        assert_eq!(ids.len(), 5);
        assert_eq!(ids, vec!["a", "b", "c", "d", "e"]);
    }

    #[test]
    fn test_parse_ids_empty_string() {
        let ids = parse_selected_ids("");
        assert!(ids.is_empty());
    }

    #[test]
    fn test_parse_ids_filters_empty_segments() {
        let ids = parse_selected_ids("abc-1, , , def-2");
        assert_eq!(ids, vec!["abc-1", "def-2"]);
    }
}
