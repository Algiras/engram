pub mod provider;
pub mod search;
pub mod store;

pub use provider::EmbeddingProvider;
pub use store::EmbeddingStore;

/// Standard embedding dimension (OpenAI ada-002, all-MiniLM-L6-v2, etc.)
pub const EMBEDDING_DIM: usize = 384;

/// A text chunk with its embedding
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EmbeddedChunk {
    pub id: String,
    pub text: String,
    pub embedding: Vec<f32>,
    pub metadata: ChunkMetadata,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChunkMetadata {
    pub project: String,
    pub category: String, // decisions, solutions, patterns, etc.
    pub session_id: Option<String>,
    pub timestamp: String,
}

/// Cosine similarity between two vectors
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let magnitude_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let magnitude_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if magnitude_a == 0.0 || magnitude_b == 0.0 {
        return 0.0;
    }

    dot_product / (magnitude_a * magnitude_b)
}

/// Chunk text into smaller pieces for embedding
pub fn chunk_text(text: &str, max_chunk_size: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let paragraphs: Vec<&str> = text.split("\n\n").collect();

    let mut current_chunk = String::new();

    for paragraph in paragraphs {
        if current_chunk.len() + paragraph.len() > max_chunk_size && !current_chunk.is_empty() {
            chunks.push(current_chunk.clone());
            current_chunk.clear();
        }

        if !current_chunk.is_empty() {
            current_chunk.push_str("\n\n");
        }
        current_chunk.push_str(paragraph);
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}
