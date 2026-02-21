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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::chunk_text;

    // ── chunk_text (legacy char-based) ─────────────────────────────────────

    #[test]
    fn test_chunk_text_splits_on_paragraphs() {
        let text = "Para 1.\n\nPara 2.\n\nPara 3.";
        let chunks = chunk_text(text, 1000);
        // All fit in one chunk
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_chunk_text_splits_at_max_size() {
        let long_para = "x".repeat(800);
        let text = format!("{}\n\n{}", long_para, long_para);
        let chunks = chunk_text(&text, 1000);
        // Each 800-char para is below 1000, but together they exceed it
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].len() <= 1000);
    }

    #[test]
    fn test_chunk_text_empty_input() {
        let chunks = chunk_text("", 1000);
        assert!(chunks.is_empty());
    }

    // ── session-aware MAX_SESSION_CHUNK constant ───────────────────────────

    #[test]
    fn test_max_session_chunk_constant() {
        // Ensure sub-chunking threshold is larger than typical session content
        assert!(MAX_SESSION_CHUNK >= 800);
        assert!(MAX_SESSION_CHUNK <= 2000);
    }

    // ── session block sub-chunking ─────────────────────────────────────────

    #[test]
    fn test_short_session_is_single_chunk() {
        let text = "Short session content.".to_string();
        assert!(text.len() <= MAX_SESSION_CHUNK);
        // A single chunk — no sub-chunking needed
        let chunks: Vec<_> = if text.len() <= MAX_SESSION_CHUNK {
            vec![text.clone()]
        } else {
            chunk_text(&text, MAX_SESSION_CHUNK)
        };
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_long_session_is_sub_chunked() {
        // Create a session whose content clearly exceeds MAX_SESSION_CHUNK
        let long_text = format!("{}\n\n{}", "y".repeat(700), "z".repeat(700));
        assert!(long_text.len() > MAX_SESSION_CHUNK);
        let sub_chunks = chunk_text(&long_text, MAX_SESSION_CHUNK);
        assert!(sub_chunks.len() >= 2);
        for chunk in &sub_chunks {
            assert!(chunk.len() <= MAX_SESSION_CHUNK);
        }
    }
}
