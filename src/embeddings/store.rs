use super::{cosine_similarity, EmbeddedChunk};
use crate::error::Result;
use std::path::Path;

/// Optional pre-filters applied before cosine scoring
pub struct SearchFilter {
    /// Only include chunks with timestamp >= this value
    pub since: Option<chrono::DateTime<chrono::Utc>>,
    /// Only include chunks whose category exactly matches this string
    pub category: Option<String>,
    /// Only include chunks whose session_id contains this substring
    pub file_hint: Option<String>,
}

impl SearchFilter {
    pub fn empty() -> Self {
        SearchFilter {
            since: None,
            category: None,
            file_hint: None,
        }
    }

    fn matches(&self, chunk: &EmbeddedChunk) -> bool {
        if let Some(ref since) = self.since {
            // Parse chunk timestamp; skip chunk if parse fails (treat as old)
            let ts = chrono::DateTime::parse_from_rfc3339(&chunk.metadata.timestamp)
                .map(|t| t.with_timezone(&chrono::Utc))
                .unwrap_or(chrono::DateTime::<chrono::Utc>::MIN_UTC);
            if ts < *since {
                return false;
            }
        }
        if let Some(ref cat) = self.category {
            if &chunk.metadata.category != cat {
                return false;
            }
        }
        if let Some(ref hint) = self.file_hint {
            let matches_session = chunk
                .metadata
                .session_id
                .as_deref()
                .map(|s| s.contains(hint.as_str()))
                .unwrap_or(false);
            // Also check text content for file path hints
            let matches_text = chunk.text.contains(hint.as_str());
            if !matches_session && !matches_text {
                return false;
            }
        }
        true
    }
}

/// In-memory embedding store with persistence
pub struct EmbeddingStore {
    pub chunks: Vec<EmbeddedChunk>,
    pub index_path: std::path::PathBuf,
}

impl EmbeddingStore {
    /// Create new empty store
    pub fn new(index_path: std::path::PathBuf) -> Self {
        Self {
            chunks: Vec::new(),
            index_path,
        }
    }

    /// Load from file if exists, otherwise create new
    pub fn load_or_create(index_path: std::path::PathBuf) -> Self {
        if index_path.exists() {
            Self::load(&index_path).unwrap_or_else(|_| Self::new(index_path))
        } else {
            Self::new(index_path)
        }
    }

    /// Load from JSON file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let chunks: Vec<EmbeddedChunk> = serde_json::from_str(&content)?;
        Ok(Self {
            chunks,
            index_path: path.to_path_buf(),
        })
    }

    /// Save to JSON file
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.index_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&self.chunks)?;
        std::fs::write(&self.index_path, json)?;
        Ok(())
    }

    /// Add a chunk to the store
    pub fn add_chunk(&mut self, chunk: EmbeddedChunk) {
        self.chunks.push(chunk);
    }

    /// Find most similar chunks to a query embedding
    pub fn search(&self, query_embedding: &[f32], top_k: usize) -> Vec<(f32, &EmbeddedChunk)> {
        let mut results: Vec<(f32, &EmbeddedChunk)> = self
            .chunks
            .iter()
            .map(|chunk| {
                let similarity = cosine_similarity(query_embedding, &chunk.embedding);
                (similarity, chunk)
            })
            .collect();

        // Sort by similarity (descending)
        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        results.into_iter().take(top_k).collect()
    }

    /// Find most similar chunks matching the filter pre-conditions
    pub fn search_filtered(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        filter: &SearchFilter,
    ) -> Vec<(f32, &EmbeddedChunk)> {
        let mut results: Vec<(f32, &EmbeddedChunk)> = self
            .chunks
            .iter()
            .filter(|chunk| filter.matches(chunk))
            .map(|chunk| {
                let similarity = cosine_similarity(query_embedding, &chunk.embedding);
                (similarity, chunk)
            })
            .collect();

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        results.into_iter().take(top_k).collect()
    }

    /// Search with text query (requires provider to generate embedding)
    pub async fn search_text(
        &self,
        query: &str,
        provider: &super::EmbeddingProvider,
        top_k: usize,
    ) -> Result<Vec<(f32, &EmbeddedChunk)>> {
        let query_embedding = provider.embed(query).await?;
        Ok(self.search(&query_embedding, top_k))
    }

    /// Get statistics
    pub fn stats(&self) -> StoreStats {
        let mut stats = StoreStats {
            total_chunks: self.chunks.len(),
            by_category: std::collections::HashMap::new(),
            by_project: std::collections::HashMap::new(),
        };

        for chunk in &self.chunks {
            *stats
                .by_category
                .entry(chunk.metadata.category.clone())
                .or_insert(0) += 1;
            *stats
                .by_project
                .entry(chunk.metadata.project.clone())
                .or_insert(0) += 1;
        }

        stats
    }
}

pub struct StoreStats {
    pub total_chunks: usize,
    pub by_category: std::collections::HashMap<String, usize>,
    pub by_project: std::collections::HashMap<String, usize>,
}
