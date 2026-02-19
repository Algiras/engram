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
) -> Result<()> {
    // 1. Semantic search (graceful error → empty)
    let mut entries: Vec<SmartEntry> =
        smart_search_sync(project, &config.memory_dir, query, top_k, threshold)
            .unwrap_or_else(|_| vec![]);
    let used_semantic = !entries.is_empty();

    // 2. Lexical fallback if semantic returned fewer than 2 results
    if entries.len() < 2 {
        let knowledge_dir = config.memory_dir.join("knowledge").join(project);
        for cat in &[
            "decisions",
            "solutions",
            "patterns",
            "bugs",
            "insights",
            "questions",
        ] {
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
    let answer = rt.block_on(async {
        client
            .chat(SYSTEM_QA_ASSISTANT, &ask_prompt(query, &context_str))
            .await
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
