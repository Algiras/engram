use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::Result;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Manifest {
    /// Map of file path -> SHA-256 hash of the file at time of processing
    pub processed: HashMap<String, String>,
}

impl Manifest {
    /// Load manifest from disk, or return default if not found
    pub fn load(memory_dir: &Path) -> Result<Self> {
        let path = Self::manifest_path(memory_dir);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let manifest: Manifest = serde_json::from_str(&content).unwrap_or_default();
            Ok(manifest)
        } else {
            Ok(Manifest::default())
        }
    }

    /// Save manifest to disk
    pub fn save(&self, memory_dir: &Path) -> Result<()> {
        let path = Self::manifest_path(memory_dir);
        std::fs::create_dir_all(memory_dir)?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Check if a file has already been processed (and hasn't changed)
    pub fn is_processed(&self, path: &Path) -> bool {
        let key = path.to_string_lossy().to_string();
        match self.processed.get(&key) {
            Some(stored_hash) => {
                // Quick check: compare file size/hash
                match hash_file(path) {
                    Ok(current_hash) => &current_hash == stored_hash,
                    Err(_) => false,
                }
            }
            None => false,
        }
    }

    /// Mark a file as processed with its current hash
    pub fn mark_processed(&mut self, path: &Path) -> Result<()> {
        let key = path.to_string_lossy().to_string();
        let hash = hash_file(path)?;
        self.processed.insert(key, hash);
        Ok(())
    }

    /// Number of processed sessions
    pub fn processed_count(&self) -> usize {
        self.processed.len()
    }

    fn manifest_path(memory_dir: &Path) -> PathBuf {
        memory_dir.join("_manifest.json")
    }
}

/// Compute SHA-256 hash of a file
fn hash_file(path: &Path) -> Result<String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
