// Pack Installation - Installing and managing knowledge packs

use crate::error::{MemoryError, Result};
use crate::hive::pack::KnowledgePack;
use crate::hive::registry::RegistryManager;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Metadata for an installed pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPack {
    pub name: String,
    pub registry: String,
    pub version: String,
    pub installed_at: DateTime<Utc>,
    pub path: PathBuf,
}

/// Storage for installed packs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InstalledPackStore {
    pub packs: Vec<InstalledPack>,
}

/// Manager for pack installation operations
pub struct PackInstaller {
    hive_dir: PathBuf,
    packs_dir: PathBuf,
}

impl InstalledPackStore {
    /// Load installed packs from disk
    pub fn load(hive_dir: &Path) -> Result<Self> {
        let path = hive_dir.join("installed_packs.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let store: InstalledPackStore = serde_json::from_str(&content)
                .map_err(|e| MemoryError::Config(format!("Invalid installed_packs.json: {}", e)))?;
            Ok(store)
        } else {
            Ok(InstalledPackStore::default())
        }
    }

    /// Save installed packs to disk
    pub fn save(&self, hive_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(hive_dir)?;
        let path = hive_dir.join("installed_packs.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    /// Add an installed pack
    pub fn add(&mut self, pack: InstalledPack) -> Result<()> {
        // Check for duplicates
        if self.packs.iter().any(|p| p.name == pack.name) {
            return Err(MemoryError::Config(format!(
                "Pack '{}' is already installed",
                pack.name
            )));
        }

        self.packs.push(pack);
        Ok(())
    }

    /// Remove an installed pack
    pub fn remove(&mut self, name: &str) -> Result<()> {
        let initial_len = self.packs.len();
        self.packs.retain(|p| p.name != name);

        if self.packs.len() == initial_len {
            return Err(MemoryError::Config(format!("Pack '{}' not found", name)));
        }

        Ok(())
    }

    /// Get an installed pack by name
    pub fn get(&self, name: &str) -> Option<&InstalledPack> {
        self.packs.iter().find(|p| p.name == name)
    }

    /// Check if a pack is installed
    pub fn is_installed(&self, name: &str) -> bool {
        self.packs.iter().any(|p| p.name == name)
    }

    /// List all installed packs
    pub fn list(&self) -> &[InstalledPack] {
        &self.packs
    }
}

impl PackInstaller {
    /// Create a new pack installer
    pub fn new(memory_dir: &Path) -> Self {
        Self {
            hive_dir: memory_dir.join("hive"),
            packs_dir: memory_dir.join("packs/installed"),
        }
    }

    /// Install a pack from a registry
    pub fn install(&self, pack_name: &str, registry_name: Option<&str>) -> Result<InstalledPack> {
        let registry_manager = RegistryManager::new(self.hive_dir.parent().unwrap());

        // Find the pack in registries
        let (pack, found_registry) = self.find_pack(pack_name, registry_name, &registry_manager)?;

        // Check if already installed
        let mut store = InstalledPackStore::load(&self.hive_dir)?;
        if store.is_installed(&pack.name) {
            return Err(MemoryError::Config(format!(
                "Pack '{}' is already installed. Use 'update' to update it.",
                pack.name
            )));
        }

        // Create installation directory
        let pack_dir = self.packs_dir.join(&pack.name);
        std::fs::create_dir_all(&pack_dir)?;

        // Copy knowledge files from registry
        let registry_pack_dir = found_registry.local_path(&self.hive_dir).join(&pack.name);
        self.copy_pack_content(&registry_pack_dir, &pack_dir)?;

        // Record installation
        let installed_pack = InstalledPack {
            name: pack.name.clone(),
            registry: found_registry.name.clone(),
            version: pack.version.clone(),
            installed_at: Utc::now(),
            path: pack_dir,
        };

        store.add(installed_pack.clone())?;
        store.save(&self.hive_dir)?;

        Ok(installed_pack)
    }

    /// Uninstall a pack
    pub fn uninstall(&self, pack_name: &str) -> Result<()> {
        let mut store = InstalledPackStore::load(&self.hive_dir)?;

        // Get pack info
        let pack = store
            .get(pack_name)
            .ok_or_else(|| MemoryError::Config(format!("Pack '{}' not installed", pack_name)))?
            .clone();

        // Remove from store
        store.remove(pack_name)?;
        store.save(&self.hive_dir)?;

        // Remove pack directory
        if pack.path.exists() {
            std::fs::remove_dir_all(&pack.path)?;
        }

        Ok(())
    }

    /// List installed packs
    pub fn list(&self) -> Result<Vec<InstalledPack>> {
        let store = InstalledPackStore::load(&self.hive_dir)?;
        Ok(store.list().to_vec())
    }

    /// Update an installed pack
    pub fn update(&self, pack_name: &str) -> Result<()> {
        let store = InstalledPackStore::load(&self.hive_dir)?;

        // Get installed pack info
        let installed = store
            .get(pack_name)
            .ok_or_else(|| MemoryError::Config(format!("Pack '{}' not installed", pack_name)))?
            .clone();

        // Update the registry first
        let registry_manager = RegistryManager::new(self.hive_dir.parent().unwrap());
        registry_manager.update(&installed.registry)?;

        // Get latest pack info from registry
        let registry_store = crate::hive::registry::RegistryStore::load(&self.hive_dir)?;
        let registry = registry_store.get(&installed.registry).ok_or_else(|| {
            MemoryError::Config(format!("Registry '{}' not found", installed.registry))
        })?;

        let registry_pack_dir = registry.local_path(&self.hive_dir).join(&installed.name);

        if !registry_pack_dir.exists() {
            return Err(MemoryError::Config(format!(
                "Pack '{}' no longer exists in registry '{}'",
                pack_name, installed.registry
            )));
        }

        // Copy updated content
        self.copy_pack_content(&registry_pack_dir, &installed.path)?;

        Ok(())
    }

    /// Find a pack in registries
    fn find_pack(
        &self,
        pack_name: &str,
        registry_name: Option<&str>,
        registry_manager: &RegistryManager,
    ) -> Result<(KnowledgePack, crate::hive::registry::Registry)> {
        if let Some(reg_name) = registry_name {
            // Search in specific registry
            let packs = registry_manager.discover_packs(reg_name)?;
            let pack = packs
                .into_iter()
                .find(|p| p.name == pack_name)
                .ok_or_else(|| {
                    MemoryError::Config(format!(
                        "Pack '{}' not found in registry '{}'",
                        pack_name, reg_name
                    ))
                })?;

            let registry_store = crate::hive::registry::RegistryStore::load(&self.hive_dir)?;
            let registry = registry_store
                .get(reg_name)
                .ok_or_else(|| MemoryError::Config(format!("Registry '{}' not found", reg_name)))?
                .clone();

            Ok((pack, registry))
        } else {
            // Search across all registries
            let registries = registry_manager.list()?;
            for registry in registries {
                if let Ok(packs) = registry_manager.discover_packs(&registry.name) {
                    if let Some(pack) = packs.into_iter().find(|p| p.name == pack_name) {
                        return Ok((pack, registry));
                    }
                }
            }

            Err(MemoryError::Config(format!(
                "Pack '{}' not found in any registry",
                pack_name
            )))
        }
    }

    /// Copy pack content from source to destination
    fn copy_pack_content(&self, source: &Path, dest: &Path) -> Result<()> {
        // Copy .pack directory (manifest)
        let source_pack_meta = source.join(".pack");
        let dest_pack_meta = dest.join(".pack");
        if source_pack_meta.exists() {
            self.copy_dir_recursive(&source_pack_meta, &dest_pack_meta)?;
        }

        // Copy knowledge directory
        let source_knowledge = source.join("knowledge");
        let dest_knowledge = dest.join("knowledge");
        if source_knowledge.exists() {
            self.copy_dir_recursive(&source_knowledge, &dest_knowledge)?;
        }

        // Copy graph directory
        let source_graph = source.join("graph");
        let dest_graph = dest.join("graph");
        if source_graph.exists() {
            self.copy_dir_recursive(&source_graph, &dest_graph)?;
        }

        // Copy README if exists
        let source_readme = source.join("README.md");
        let dest_readme = dest.join("README.md");
        if source_readme.exists() {
            std::fs::copy(&source_readme, &dest_readme)?;
        }

        Ok(())
    }

    /// Recursively copy a directory
    fn copy_dir_recursive(&self, source: &Path, dest: &Path) -> Result<()> {
        std::fs::create_dir_all(dest)?;

        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            let source_path = entry.path();
            let dest_path = dest.join(entry.file_name());

            if source_path.is_dir() {
                self.copy_dir_recursive(&source_path, &dest_path)?;
            } else {
                std::fs::copy(&source_path, &dest_path)?;
            }
        }

        Ok(())
    }

    /// Get installed pack knowledge directories
    pub fn get_installed_knowledge_dirs(&self) -> Result<HashMap<String, PathBuf>> {
        let store = InstalledPackStore::load(&self.hive_dir)?;
        let mut dirs = HashMap::new();

        for pack in store.list() {
            let knowledge_dir = pack.path.join("knowledge");
            if knowledge_dir.exists() {
                dirs.insert(pack.name.clone(), knowledge_dir);
            }
        }

        Ok(dirs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_installed_pack_store_crud() {
        let temp_dir = TempDir::new().unwrap();
        let hive_dir = temp_dir.path();

        let mut store = InstalledPackStore::default();
        let pack = InstalledPack {
            name: "test-pack".to_string(),
            registry: "test-registry".to_string(),
            version: "1.0.0".to_string(),
            installed_at: Utc::now(),
            path: PathBuf::from("/tmp/test"),
        };

        // Add
        store.add(pack.clone()).unwrap();
        assert_eq!(store.list().len(), 1);
        assert!(store.is_installed("test-pack"));

        // Duplicate add
        assert!(store.add(pack.clone()).is_err());

        // Get
        assert!(store.get("test-pack").is_some());
        assert!(store.get("nonexistent").is_none());

        // Save and load
        store.save(hive_dir).unwrap();
        let loaded = InstalledPackStore::load(hive_dir).unwrap();
        assert_eq!(loaded.list().len(), 1);

        // Remove
        let mut store = loaded;
        store.remove("test-pack").unwrap();
        assert_eq!(store.list().len(), 0);
    }

    #[test]
    fn test_pack_installer() {
        let temp_dir = TempDir::new().unwrap();
        let memory_dir = temp_dir.path();

        let installer = PackInstaller::new(memory_dir);

        // Initially no packs
        let packs = installer.list().unwrap();
        assert_eq!(packs.len(), 0);
    }
}
