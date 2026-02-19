use crate::config::Config;
use crate::embeddings;
use crate::embeddings::store::SearchFilter;
use crate::error::{MemoryError, Result};
use crate::extractor::knowledge::parse_ttl;
use crate::llm::client::LlmClient;
use colored::Colorize;

pub fn cmd_embed(
    config: &Config,
    project: &str,
    provider_override: Option<&str>,
    verbose: bool,
) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        println!(
            "{} Building embeddings for '{}'...",
            "Embedding".green().bold(),
            project
        );

        let provider = if let Some(prov) = provider_override {
            // Read stored embed_model from auth.json for all explicit-provider paths
            let stored_embed_model = crate::auth::AuthStore::load()
                .ok()
                .and_then(|s| s.embed_model);

            match prov {
                "openai" => {
                    let key = std::env::var("OPENAI_API_KEY")
                        .map_err(|_| MemoryError::Config("OPENAI_API_KEY not set".into()))?;
                    let model =
                        stored_embed_model.unwrap_or_else(|| "text-embedding-3-small".to_string());
                    if verbose {
                        println!("{} Provider: OpenAI ({})", "Embed:".cyan(), model);
                    }
                    embeddings::EmbeddingProvider::OpenAI {
                        api_key: key,
                        model,
                    }
                }
                "gemini" => {
                    let key = std::env::var("GEMINI_API_KEY")
                        .map_err(|_| MemoryError::Config("GEMINI_API_KEY not set".into()))?;
                    let model =
                        stored_embed_model.unwrap_or_else(|| "gemini-embedding-001".to_string());
                    if verbose {
                        println!("{} Provider: Gemini ({})", "Embed:".cyan(), model);
                    }
                    embeddings::EmbeddingProvider::Gemini {
                        api_key: key,
                        model,
                    }
                }
                "ollama" => {
                    let model =
                        stored_embed_model.unwrap_or_else(|| "nomic-embed-text".to_string());
                    if verbose {
                        println!("{} Provider: Ollama ({})", "Embed:".cyan(), model);
                    }
                    embeddings::EmbeddingProvider::OllamaLocal { model }
                }
                _ => return Err(MemoryError::Config(format!("Unknown provider: {}", prov))),
            }
        } else {
            let p = embeddings::EmbeddingProvider::from_config(config);
            if verbose {
                let name = match &p {
                    embeddings::EmbeddingProvider::OpenAI { .. } => {
                        "OpenAI (text-embedding-3-small)"
                    }
                    embeddings::EmbeddingProvider::Gemini { .. } => "Gemini (text-embedding-004)",
                    embeddings::EmbeddingProvider::OllamaLocal { .. } => {
                        "Ollama (nomic-embed-text)"
                    }
                };
                println!("{} Provider: {}", "Embed:".cyan(), name);
            }
            p
        };

        let store =
            embeddings::search::SemanticSearch::build_index(&config.memory_dir, project, &provider)
                .await?;

        let stats = store.stats();

        println!("{} Embeddings created:", "Done!".green().bold());
        println!("  Total chunks: {}", stats.total_chunks);
        println!("  By category:");
        for (cat, count) in stats.by_category {
            println!("    {}: {}", cat, count);
        }
        println!("\nSearch with:");
        println!(
            "  {}",
            format!(
                "engram search-semantic \"your query\" --project {}",
                project
            )
            .cyan()
        );

        Ok(())
    })
}

#[allow(clippy::too_many_arguments)]
pub fn cmd_search_semantic(
    config: &Config,
    query: &str,
    project: Option<&str>,
    top_k: usize,
    threshold: f32,
    verbose: bool,
    since: Option<&str>,
    category: Option<&str>,
    file: Option<&str>,
) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    // Build search filter from optional arguments
    let filter = {
        let since_dt = since.and_then(|s| parse_ttl(s).map(|dur| chrono::Utc::now() - dur));
        if since.is_some() && since_dt.is_none() {
            return Err(MemoryError::Config(format!(
                "Invalid --since value '{}'. Use format like 7d, 2h, 30m",
                since.unwrap_or("")
            )));
        }
        SearchFilter {
            since: since_dt,
            category: category.map(|s| s.to_string()),
            file_hint: file.map(|s| s.to_string()),
        }
    };

    let has_filter = since.is_some() || category.is_some() || file.is_some();

    rt.block_on(async {
        let provider = embeddings::EmbeddingProvider::from_config(config);

        if verbose {
            let name = match &provider {
                embeddings::EmbeddingProvider::OpenAI { .. } => "OpenAI (text-embedding-3-small)",
                embeddings::EmbeddingProvider::Gemini { .. } => "Gemini (text-embedding-004)",
                embeddings::EmbeddingProvider::OllamaLocal { .. } => "Ollama (nomic-embed-text)",
            };
            println!("{} Provider: {}", "Search:".cyan(), name);
            if has_filter {
                println!(
                    "{} Filters: since={:?}  category={:?}  file={:?}",
                    "Search:".cyan(),
                    since,
                    category,
                    file
                );
            }
        }

        // Build optional LLM client for HyDE
        let llm_client = LlmClient::new(&config.llm);

        if let Some(proj) = project {
            // Search specific project (filtered)
            let results =
                search_project_filtered(&config.memory_dir, proj, query, &provider, top_k, &filter, Some(&llm_client), verbose)
                    .await?;

            println!(
                "{} Semantic search results for '{}':\n",
                "Search".green().bold(),
                query
            );

            for (score, text, cat) in results {
                if score >= threshold {
                    println!("  {} [{}] ({:.1}%)", ">".green(), cat.cyan(), score * 100.0);
                    if verbose {
                        println!("    similarity: {:.4}", score);
                    }
                    println!("    {}\n", truncate_text(&text, 150));
                }
            }
        } else {
            // Search all projects with embeddings
            let knowledge_dir = config.memory_dir.join("knowledge");
            let mut all_results = Vec::new();

            for entry in std::fs::read_dir(&knowledge_dir)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }

                let project_name = entry.file_name().to_string_lossy().to_string();
                if project_name == "_global" {
                    continue;
                }

                let index_path = entry.path().join("embeddings.json");
                if !index_path.exists() {
                    continue;
                }

                if let Ok(results) = search_project_filtered(
                    &config.memory_dir,
                    &project_name,
                    query,
                    &provider,
                    top_k,
                    &filter,
                    Some(&llm_client),
                    verbose,
                )
                .await
                {
                    for (score, text, cat) in results {
                        if score >= threshold {
                            all_results.push((score, text, cat, project_name.clone()));
                        }
                    }
                }
            }

            // Sort by score
            all_results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            all_results.truncate(top_k);

            println!(
                "{} Semantic search results for '{}':\n",
                "Search".green().bold(),
                query
            );

            for (score, text, cat, proj) in all_results {
                println!(
                    "  {} [{}:{}] ({:.1}%)",
                    ">".green(),
                    proj.dimmed(),
                    cat.cyan(),
                    score * 100.0
                );
                if verbose {
                    println!("    similarity: {:.4}", score);
                }
                println!("    {}\n", truncate_text(&text, 150));
            }
        }

        // Track usage
        let tracker = crate::analytics::EventTracker::new(&config.memory_dir);
        let _ = tracker.track(crate::analytics::UsageEvent {
            timestamp: chrono::Utc::now(),
            event_type: crate::analytics::EventType::SemanticSearch,
            project: project
                .map(|s| s.to_string())
                .unwrap_or_else(|| "all".to_string()),
            query: Some(query.to_string()),
            category: category.map(|s| s.to_string()),
            results_count: None,
            session_id: None,
            tokens_consumed: None,
        });

        Ok(())
    })
}

/// Search a project's embedding index with an optional filter.
/// When `llm_client` is provided, uses HyDE (Hypothetical Document Embedding) to improve recall.
#[allow(clippy::too_many_arguments)]
async fn search_project_filtered(
    memory_dir: &std::path::Path,
    project: &str,
    query: &str,
    provider: &embeddings::EmbeddingProvider,
    top_k: usize,
    filter: &SearchFilter,
    llm_client: Option<&LlmClient>,
    verbose: bool,
) -> Result<Vec<(f32, String, String)>> {
    use embeddings::store::EmbeddingStore;

    let index_path = memory_dir
        .join("knowledge")
        .join(project)
        .join("embeddings.json");

    if !index_path.exists() {
        return Err(MemoryError::Config(
            "No embedding index found. Run 'engram embed' first.".into(),
        ));
    }

    let store = EmbeddingStore::load(&index_path)?;

    // HyDE: generate a hypothetical document that would answer the query, then embed that
    let embed_text = if let Some(client) = llm_client {
        match client
            .chat(
                crate::llm::prompts::SYSTEM_HYDE_GENERATOR,
                &crate::llm::prompts::hyde_prompt(query),
            )
            .await
        {
            Ok(hypothetical) => {
                if verbose {
                    println!("  HyDE: {}", hypothetical.trim().chars().take(120).collect::<String>());
                }
                format!("{}\n\nQuery: {}", hypothetical.trim(), query)
            }
            Err(_) => query.to_string(), // Fall back to raw query on LLM failure
        }
    } else {
        query.to_string()
    };

    let query_embedding = provider.embed(&embed_text).await?;
    let results = store.search_filtered(&query_embedding, top_k, filter);

    Ok(results
        .into_iter()
        .map(|(score, chunk)| (score, chunk.text.clone(), chunk.metadata.category.clone()))
        .collect())
}

fn truncate_text(text: &str, max_len: usize) -> String {
    let cleaned = text.replace('\n', " ").trim().to_string();
    if cleaned.len() <= max_len {
        cleaned
    } else {
        format!("{}...", &cleaned[..max_len - 3])
    }
}
