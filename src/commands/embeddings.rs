use crate::config::Config;
use crate::embeddings;
use crate::error::{MemoryError, Result};
use colored::Colorize;

pub fn cmd_embed(config: &Config, project: &str, provider_override: Option<&str>) -> Result<()> {
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
            match prov {
                "openai" => {
                    let key = std::env::var("OPENAI_API_KEY")
                        .map_err(|_| MemoryError::Config("OPENAI_API_KEY not set".into()))?;
                    embeddings::EmbeddingProvider::OpenAI { api_key: key }
                }
                "gemini" => {
                    let key = std::env::var("GEMINI_API_KEY")
                        .map_err(|_| MemoryError::Config("GEMINI_API_KEY not set".into()))?;
                    embeddings::EmbeddingProvider::Gemini { api_key: key }
                }
                "ollama" => embeddings::EmbeddingProvider::OllamaLocal,
                _ => return Err(MemoryError::Config(format!("Unknown provider: {}", prov))),
            }
        } else {
            embeddings::EmbeddingProvider::from_env()?
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

pub fn cmd_search_semantic(
    config: &Config,
    query: &str,
    project: Option<&str>,
    top_k: usize,
    threshold: f32,
) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        let provider = embeddings::EmbeddingProvider::from_env()?;

        if let Some(proj) = project {
            // Search specific project
            let results = embeddings::search::SemanticSearch::search(
                &config.memory_dir,
                proj,
                query,
                &provider,
                top_k,
            )
            .await?;

            println!(
                "{} Semantic search results for '{}':\n",
                "Search".green().bold(),
                query
            );

            for (score, text, category) in results {
                if score >= threshold {
                    println!(
                        "  {} [{}] ({:.1}%)",
                        ">".green(),
                        category.cyan(),
                        score * 100.0
                    );
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

                if let Ok(results) = embeddings::search::SemanticSearch::search(
                    &config.memory_dir,
                    &project_name,
                    query,
                    &provider,
                    top_k,
                )
                .await
                {
                    for (score, text, category) in results {
                        if score >= threshold {
                            all_results.push((score, text, category, project_name.clone()));
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

            for (score, text, category, proj) in all_results {
                println!(
                    "  {} [{}:{}] ({:.1}%)",
                    ">".green(),
                    proj.dimmed(),
                    category.cyan(),
                    score * 100.0
                );
                println!("    {}\n", truncate_text(&text, 150));
            }
        }

        Ok(())
    })
}

fn truncate_text(text: &str, max_len: usize) -> String {
    let cleaned = text.replace('\n', " ").trim().to_string();
    if cleaned.len() <= max_len {
        cleaned
    } else {
        format!("{}...", &cleaned[..max_len - 3])
    }
}
