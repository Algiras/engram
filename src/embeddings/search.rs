use super::{chunk_text, ChunkMetadata, EmbeddedChunk, EmbeddingProvider, EmbeddingStore};
use crate::error::Result;
use crate::extractor::knowledge::{parse_session_blocks, partition_by_expiry};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Maximum chars for a single session block before sub-chunking.
const MAX_SESSION_CHUNK: usize = 1200;

pub struct SemanticSearch;

impl SemanticSearch {
    /// Build embedding index for a project.
    ///
    /// Uses session-aware chunking: each session block in a knowledge file gets
    /// its own embedding with its `session_id` set. Blocks larger than
    /// `MAX_SESSION_CHUNK` are sub-chunked (preserving the same session_id).
    /// context.md is still chunked with the legacy char-based chunker since it
    /// is a synthesised narrative without session blocks.
    pub async fn build_index(
        memory_dir: &Path,
        project: &str,
        provider: &EmbeddingProvider,
    ) -> Result<EmbeddingStore> {
        let knowledge_dir = memory_dir.join("knowledge").join(project);
        let index_path = knowledge_dir.join("embeddings.json");

        let mut store = EmbeddingStore::new(index_path);

        // ── context.md: char-based chunking (synthesised narrative, no sessions) ──
        let context_path = knowledge_dir.join("context.md");
        if context_path.exists() {
            let content = std::fs::read_to_string(&context_path)?;
            if !content.trim().is_empty() {
                let chunks = chunk_text(&content, 1000);
                let embeddings = provider.embed_batch(&chunks).await?;
                for (text, embedding) in chunks.into_iter().zip(embeddings) {
                    let chunk_id = generate_chunk_id(&text);
                    store.add_chunk(EmbeddedChunk {
                        id: chunk_id,
                        text,
                        embedding,
                        metadata: ChunkMetadata {
                            project: project.to_string(),
                            category: "context".to_string(),
                            session_id: None,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                        },
                    });
                }
            }
        }

        // ── Category files: session-aware chunking ─────────────────────────────
        for (cat, file) in crate::config::CATEGORIES
            .iter()
            .zip(crate::config::CATEGORY_FILES.iter())
        {
            let path = knowledge_dir.join(file);
            if !path.exists() {
                continue;
            }

            let content = std::fs::read_to_string(&path)?;
            if content.trim().is_empty() {
                continue;
            }

            let (_preamble, blocks) = parse_session_blocks(&content);
            let (active, _expired) = partition_by_expiry(blocks);

            if active.is_empty() {
                continue;
            }

            // Build (session_id, text) pairs — sub-chunk long blocks
            let mut pairs: Vec<(String, String)> = Vec::new();
            for block in active {
                let text = block.content.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                if text.len() <= MAX_SESSION_CHUNK {
                    pairs.push((block.session_id.clone(), text));
                } else {
                    // Sub-chunk large blocks — all sub-chunks share the session_id
                    for sub in chunk_text(&text, MAX_SESSION_CHUNK) {
                        pairs.push((block.session_id.clone(), sub));
                    }
                }
            }

            if pairs.is_empty() {
                continue;
            }

            let texts: Vec<String> = pairs.iter().map(|(_, t)| t.clone()).collect();
            let embeddings = provider.embed_batch(&texts).await?;

            for ((session_id, text), embedding) in pairs.into_iter().zip(embeddings) {
                let chunk_id = generate_chunk_id(&text);
                store.add_chunk(EmbeddedChunk {
                    id: chunk_id,
                    text,
                    embedding,
                    metadata: ChunkMetadata {
                        project: project.to_string(),
                        category: cat.to_string(),
                        session_id: Some(session_id),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    },
                });
            }
        }

        store.save()?;

        Ok(store)
    }

    /// Search semantically similar content
    pub async fn search(
        memory_dir: &Path,
        project: &str,
        query: &str,
        provider: &EmbeddingProvider,
        top_k: usize,
    ) -> Result<Vec<(f32, String, String)>> {
        let index_path = memory_dir
            .join("knowledge")
            .join(project)
            .join("embeddings.json");

        if !index_path.exists() {
            return Err(crate::error::MemoryError::Config(
                "No embedding index found. Run 'engram embed' first.".into(),
            ));
        }

        let store = EmbeddingStore::load(&index_path)?;
        let results = store.search_text(query, provider, top_k).await?;

        Ok(results
            .into_iter()
            .map(|(score, chunk)| (score, chunk.text.clone(), chunk.metadata.category.clone()))
            .collect())
    }
}

fn generate_chunk_id(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())[..16].to_string()
}
