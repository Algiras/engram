// Registry Management - Discovery and tracking of knowledge pack sources

use crate::error::{MemoryError, Result};
use crate::hive::pack::KnowledgePack;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A registry of knowledge packs (similar to package registry)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub name: String,
    pub url: String,
    pub last_updated: Option<DateTime<Utc>>,
}

/// Storage for all registries
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RegistryStore {
    pub registries: Vec<Registry>,
}

/// Manager for registry operations
pub struct RegistryManager {
    hive_dir: PathBuf,
}

impl Registry {
    /// Create a new registry from a URL
    /// Supports both full Git URLs and GitHub shorthand (owner/repo)
    pub fn from_url(url: &str) -> Result<Self> {
        let normalized_url = Self::normalize_url(url)?;
        let name = Self::extract_name(&normalized_url)?;

        Ok(Registry {
            name,
            url: normalized_url,
            last_updated: None,
        })
    }

    /// Normalize URL (convert GitHub shorthand to full URL)
    fn normalize_url(url: &str) -> Result<String> {
        if url.contains("://") {
            // Already a full URL
            Ok(url.to_string())
        } else if url.contains('/') {
            // GitHub shorthand: owner/repo
            Ok(format!("https://github.com/{}.git", url))
        } else {
            Err(MemoryError::Config(format!(
                "Invalid registry URL: {}. Expected either full Git URL or GitHub shorthand (owner/repo)",
                url
            )))
        }
    }

    /// Extract registry name from URL
    fn extract_name(url: &str) -> Result<String> {
        let name = url
            .trim_end_matches(".git")
            .split('/')
            .last()
            .ok_or_else(|| MemoryError::Config(format!("Could not extract name from URL: {}", url)))?
            .to_string();

        if name.is_empty() {
            return Err(MemoryError::Config(format!(
                "Empty registry name from URL: {}",
                url
            )));
        }

        Ok(name)
    }

    /// Get the local path where this registry is cloned
    pub fn local_path(&self, hive_dir: &Path) -> PathBuf {
        hive_dir.join("registries").join(&self.name)
    }

    /// Check if registry is cloned locally
    pub fn is_cloned(&self, hive_dir: &Path) -> bool {
        self.local_path(hive_dir).join(".git").exists()
    }
}

impl RegistryStore {
    /// Load registry store from disk
    pub fn load(hive_dir: &Path) -> Result<Self> {
        let path = hive_dir.join("registries.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let store: RegistryStore = serde_json::from_str(&content)
                .map_err(|e| MemoryError::Config(format!("Invalid registries.json: {}", e)))?;
            Ok(store)
        } else {
            Ok(RegistryStore::default())
        }
    }

    /// Save registry store to disk
    pub fn save(&self, hive_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(hive_dir)?;
        let path = hive_dir.join("registries.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Add a registry
    pub fn add(&mut self, registry: Registry) -> Result<()> {
        // Check for duplicates
        if self.registries.iter().any(|r| r.name == registry.name) {
            return Err(MemoryError::Config(format!(
                "Registry '{}' already exists",
                registry.name
            )));
        }

        self.registries.push(registry);
        Ok(())
    }

    /// Remove a registry by name
    pub fn remove(&mut self, name: &str) -> Result<()> {
        let initial_len = self.registries.len();
        self.registries.retain(|r| r.name != name);

        if self.registries.len() == initial_len {
            return Err(MemoryError::Config(format!(
                "Registry '{}' not found",
                name
            )));
        }

        Ok(())
    }

    /// Get a registry by name
    pub fn get(&self, name: &str) -> Option<&Registry> {
        self.registries.iter().find(|r| r.name == name)
    }

    /// Get a mutable reference to a registry by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Registry> {
        self.registries.iter_mut().find(|r| r.name == name)
    }

    /// List all registries
    pub fn list(&self) -> &[Registry] {
        &self.registries
    }
}

impl RegistryManager {
    /// Create a new registry manager
    pub fn new(memory_dir: &Path) -> Self {
        Self {
            hive_dir: memory_dir.join("hive"),
        }
    }

    /// Add a registry
    pub fn add(&self, url: &str) -> Result<Registry> {
        let registry = Registry::from_url(url)?;

        let mut store = RegistryStore::load(&self.hive_dir)?;
        store.add(registry.clone())?;
        store.save(&self.hive_dir)?;

        // Clone the registry
        self.clone_registry(&registry)?;

        Ok(registry)
    }

    /// Remove a registry
    pub fn remove(&self, name: &str) -> Result<()> {
        let mut store = RegistryStore::load(&self.hive_dir)?;
        let registry = store
            .get(name)
            .ok_or_else(|| MemoryError::Config(format!("Registry '{}' not found", name)))?
            .clone();

        store.remove(name)?;
        store.save(&self.hive_dir)?;

        // Remove local clone
        let local_path = registry.local_path(&self.hive_dir);
        if local_path.exists() {
            std::fs::remove_dir_all(&local_path)?;
        }

        Ok(())
    }

    /// List all registries
    pub fn list(&self) -> Result<Vec<Registry>> {
        let store = RegistryStore::load(&self.hive_dir)?;
        Ok(store.list().to_vec())
    }

    /// Update a registry (git pull)
    pub fn update(&self, name: &str) -> Result<()> {
        let mut store = RegistryStore::load(&self.hive_dir)?;
        let registry = store
            .get(name)
            .ok_or_else(|| MemoryError::Config(format!("Registry '{}' not found", name)))?
            .clone();

        let local_path = registry.local_path(&self.hive_dir);

        if !registry.is_cloned(&self.hive_dir) {
            // Clone if not present
            self.clone_registry(&registry)?;
        } else {
            // Pull updates
            self.pull_registry(&local_path)?;
        }

        // Update timestamp
        if let Some(reg) = store.get_mut(name) {
            reg.last_updated = Some(Utc::now());
        }
        store.save(&self.hive_dir)?;

        Ok(())
    }

    /// Clone a registry from Git
    fn clone_registry(&self, registry: &Registry) -> Result<()> {
        let local_path = registry.local_path(&self.hive_dir);
        std::fs::create_dir_all(local_path.parent().unwrap())?;

        let status = std::process::Command::new("git")
            .args(&[
                "clone",
                "--depth", "1",  // Shallow clone for speed
                &registry.url,
                local_path.to_str().unwrap(),
            ])
            .status()?;

        if !status.success() {
            return Err(MemoryError::Config(format!(
                "Failed to clone registry from {}",
                registry.url
            )));
        }

        Ok(())
    }

    /// Pull updates from a registry
    fn pull_registry(&self, local_path: &Path) -> Result<()> {
        let status = std::process::Command::new("git")
            .args(&["pull", "--ff-only"])
            .current_dir(local_path)
            .status()?;

        if !status.success() {
            return Err(MemoryError::Config(format!(
                "Failed to pull updates from registry at {}",
                local_path.display()
            )));
        }

        Ok(())
    }

    /// Discover packs in a registry
    pub fn discover_packs(&self, registry_name: &str) -> Result<Vec<KnowledgePack>> {
        let store = RegistryStore::load(&self.hive_dir)?;
        let registry = store
            .get(registry_name)
            .ok_or_else(|| MemoryError::Config(format!("Registry '{}' not found", registry_name)))?;

        let local_path = registry.local_path(&self.hive_dir);
        if !local_path.exists() {
            return Err(MemoryError::Config(format!(
                "Registry '{}' not cloned. Run update first.",
                registry_name
            )));
        }

        let mut packs = Vec::new();

        // Scan for pack directories (contain .pack/manifest.json)
        for entry in std::fs::read_dir(&local_path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() && path.join(".pack/manifest.json").exists() {
                match KnowledgePack::load(&path) {
                    Ok(pack) => packs.push(pack),
                    Err(e) => {
                        eprintln!("Warning: Failed to load pack at {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(packs)
    }

    /// Search packs across all registries
    pub fn search_packs(&self, query: &str) -> Result<HashMap<String, Vec<KnowledgePack>>> {
        let registries = self.list()?;
        let mut results = HashMap::new();

        for registry in registries {
            let packs = self.discover_packs(&registry.name)?;
            let matches: Vec<KnowledgePack> = packs
                .into_iter()
                .filter(|p| p.matches_keyword(query))
                .collect();

            if !matches.is_empty() {
                results.insert(registry.name.clone(), matches);
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_registry_from_url() {
        // Full URL
        let reg = Registry::from_url("https://github.com/user/repo.git").unwrap();
        assert_eq!(reg.name, "repo");
        assert_eq!(reg.url, "https://github.com/user/repo.git");

        // GitHub shorthand
        let reg = Registry::from_url("user/repo").unwrap();
        assert_eq!(reg.name, "repo");
        assert_eq!(reg.url, "https://github.com/user/repo.git");

        // Invalid
        assert!(Registry::from_url("invalid").is_err());
    }

    #[test]
    fn test_registry_store_crud() {
        let temp_dir = TempDir::new().unwrap();
        let hive_dir = temp_dir.path();

        let mut store = RegistryStore::default();
        let reg = Registry::from_url("user/repo").unwrap();

        // Add
        store.add(reg.clone()).unwrap();
        assert_eq!(store.list().len(), 1);

        // Duplicate add
        assert!(store.add(reg.clone()).is_err());

        // Get
        assert!(store.get("repo").is_some());
        assert!(store.get("nonexistent").is_none());

        // Save and load
        store.save(hive_dir).unwrap();
        let loaded = RegistryStore::load(hive_dir).unwrap();
        assert_eq!(loaded.list().len(), 1);

        // Remove
        let mut store = loaded;
        store.remove("repo").unwrap();
        assert_eq!(store.list().len(), 0);

        // Remove nonexistent
        assert!(store.remove("nonexistent").is_err());
    }

    #[test]
    fn test_registry_manager() {
        let temp_dir = TempDir::new().unwrap();
        let memory_dir = temp_dir.path();

        let manager = RegistryManager::new(memory_dir);

        // Initially empty
        let registries = manager.list().unwrap();
        assert_eq!(registries.len(), 0);

        // Add registry (note: this would fail without actual git repo)
        // So we just test the structure
    }
}
