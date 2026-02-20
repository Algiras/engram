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

    /// Search with text query (requires provider to generate embedding).
    /// When `hybrid=true`, combines dense + BM25 via Reciprocal Rank Fusion.
    pub async fn search_text(
        &self,
        query: &str,
        provider: &super::EmbeddingProvider,
        top_k: usize,
    ) -> Result<Vec<(f32, &EmbeddedChunk)>> {
        let query_embedding = provider.embed(query).await?;
        Ok(self.hybrid_search(&query_embedding, query, top_k))
    }

    /// BM25 lexical search over chunk texts.
    ///
    /// Standard BM25 with k1=1.5, b=0.75. Tokenises on whitespace + punctuation
    /// (lowercase). Returns up to `top_k` chunks sorted by descending score,
    /// only including those whose BM25 score > 0 (i.e. at least one query term
    /// appears in the chunk).
    pub fn bm25_search(&self, query: &str, top_k: usize) -> Vec<(f32, &EmbeddedChunk)> {
        if self.chunks.is_empty() {
            return vec![];
        }

        let k1: f32 = 1.5;
        let b: f32 = 0.75;

        // Tokenise helper: split on non-alphanumeric, lowercase, skip stop words
        let tokenise = |text: &str| -> Vec<String> {
            text.split(|c: char| !c.is_alphanumeric() && c != '_')
                .filter(|t| t.len() > 1)
                .map(|t| t.to_lowercase())
                .collect()
        };

        let query_terms: Vec<String> = tokenise(query);
        if query_terms.is_empty() {
            return vec![];
        }

        // Pre-tokenise all chunks
        let tokenised: Vec<Vec<String>> = self.chunks.iter().map(|c| tokenise(&c.text)).collect();
        let n = tokenised.len() as f32;
        let avg_dl = tokenised.iter().map(|t| t.len()).sum::<usize>() as f32 / n;

        // IDF per query term: log((N - df + 0.5) / (df + 0.5) + 1)
        let idf: Vec<f32> = query_terms
            .iter()
            .map(|term| {
                let df = tokenised.iter().filter(|t| t.contains(term)).count() as f32;
                ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
            })
            .collect();

        // Score each chunk
        let mut scored: Vec<(f32, &EmbeddedChunk)> = self
            .chunks
            .iter()
            .enumerate()
            .filter_map(|(i, chunk)| {
                let tokens = &tokenised[i];
                let dl = tokens.len() as f32;
                let score: f32 = query_terms
                    .iter()
                    .zip(idf.iter())
                    .map(|(term, &idf_val)| {
                        let tf = tokens
                            .iter()
                            .filter(|t| t.as_str() == term.as_str())
                            .count() as f32;
                        idf_val * (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * dl / avg_dl))
                    })
                    .sum();
                if score > 0.0 {
                    Some((score, chunk))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(top_k).collect()
    }

    /// Hybrid search: combine dense (embedding) + BM25 via Reciprocal Rank Fusion.
    ///
    /// RRF score = 1/(k+rank_dense) + 1/(k+rank_bm25)  where k=60 (standard).
    /// Returns top_k chunks by fused score. Requires an already-computed
    /// query embedding to avoid an extra async call.
    pub fn hybrid_search(
        &self,
        query_embedding: &[f32],
        query: &str,
        top_k: usize,
    ) -> Vec<(f32, &EmbeddedChunk)> {
        const K: f32 = 60.0;
        let candidate_k = (top_k * 3).max(30);

        // Dense ranking
        let dense = self.search(query_embedding, candidate_k);
        // BM25 ranking
        let bm25 = self.bm25_search(query, candidate_k);

        // Build chunk-id → RRF score map
        let mut rrf: std::collections::HashMap<&str, f32> = std::collections::HashMap::new();
        for (rank, (_, chunk)) in dense.iter().enumerate() {
            *rrf.entry(chunk.id.as_str()).or_insert(0.0) += 1.0 / (K + rank as f32 + 1.0);
        }
        for (rank, (_, chunk)) in bm25.iter().enumerate() {
            *rrf.entry(chunk.id.as_str()).or_insert(0.0) += 1.0 / (K + rank as f32 + 1.0);
        }

        // Collect, sort by RRF score
        let id_to_chunk: std::collections::HashMap<&str, &EmbeddedChunk> =
            self.chunks.iter().map(|c| (c.id.as_str(), c)).collect();

        let mut results: Vec<(f32, &EmbeddedChunk)> = rrf
            .into_iter()
            .filter_map(|(id, score)| id_to_chunk.get(id).map(|c| (score, *c)))
            .collect();

        results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        results.into_iter().take(top_k).collect()
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
