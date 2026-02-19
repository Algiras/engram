use colored::Colorize;

use crate::analytics::tracker::{EventTracker, EventType, UsageEvent};
use crate::config::Config;
use crate::error::{MemoryError, Result};
use crate::extractor::knowledge::{
    find_sessions_by_topic, parse_session_blocks, partition_by_expiry, strip_private_tags,
};
use crate::inject::{build_raw_context, smart_search_sync, SmartEntry};
use crate::llm::client::LlmClient;
use crate::llm::prompts::{ask_prompt, SYSTEM_QA_ASSISTANT};

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
    // 1. Semantic search (graceful error → empty)
    let mut entries: Vec<SmartEntry> =
        smart_search_sync(project, &config.memory_dir, query, top_k, threshold)
            .unwrap_or_else(|_| vec![]);
    let used_semantic = !entries.is_empty();

    // 1b. Graph-augmented retrieval (opt-in via --use-graph)
    // For each concept in the knowledge graph that matches the query, retrieve
    // semantically similar entries for its 2-hop graph neighbors.
    if use_graph {
        let graph_path = config.memory_dir
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
                                        "Ask:".cyan(), concept.name, depth, rel_concept.name
                                    );
                                }
                            }
                        }
                    }
                }

                // Fetch entries for each augmented query (dedup by session_id)
                for aug_query in augmented_queries.iter().take(4) {
                    let aug_entries = smart_search_sync(
                        project, &config.memory_dir, aug_query, 2, threshold,
                    ).unwrap_or_default();
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
                "Ask:".yellow(), project
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
        (
            SYSTEM_QA_ASSISTANT,
            ask_prompt(query, &context_str),
        )
    };
    let answer = rt.block_on(async {
        client.chat(system, &prompt).await
    })?;

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
