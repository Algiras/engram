use super::{chunk_text, ChunkMetadata, EmbeddedChunk, EmbeddingProvider, EmbeddingStore};
use crate::error::Result;
use sha2::{Digest, Sha256};
use std::path::Path;

pub struct SemanticSearch;

impl SemanticSearch {
    /// Build embedding index for a project
    pub async fn build_index(
        memory_dir: &Path,
        project: &str,
        provider: &EmbeddingProvider,
    ) -> Result<EmbeddingStore> {
        let knowledge_dir = memory_dir.join("knowledge").join(project);
        let index_path = knowledge_dir.join("embeddings.json");

        let mut store = EmbeddingStore::new(index_path);

        // Read knowledge files — context.md first, then all canonical categories
        let mut files: Vec<(&str, &str)> = vec![("context", "context.md")];
        for (cat, file) in crate::config::CATEGORIES
            .iter()
            .zip(crate::config::CATEGORY_FILES.iter())
        {
            files.push((cat, file));
        }

        for (category, filename) in &files {
            let path = knowledge_dir.join(filename);
            if !path.exists() {
                continue;
            }

            let content = std::fs::read_to_string(&path)?;
            if content.trim().is_empty() {
                continue;
            }

            // Chunk the content (max 1000 chars per chunk)
            let chunks = chunk_text(&content, 1000);

            // Generate embeddings in batch
            let chunk_texts: Vec<String> = chunks.to_vec();
            let embeddings = provider.embed_batch(&chunk_texts).await?;

            // Store chunks with embeddings
            for (text, embedding) in chunk_texts.into_iter().zip(embeddings) {
                let chunk_id = generate_chunk_id(&text);

                store.add_chunk(EmbeddedChunk {
                    id: chunk_id,
                    text,
                    embedding,
                    metadata: ChunkMetadata {
                        project: project.to_string(),
                        category: category.to_string(),
                        session_id: None,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    },
                });
            }
        }

        // Save store
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
