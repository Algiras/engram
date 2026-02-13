// Hive Mind - Distributed Memory Sharing System
//
// Stage 1: Read-Only Hive
// - Registry management (add, list, update registries)
// - Pack installation (clone from Git, copy to local)
// - Knowledge discovery (browse, search packs)
// - Integration with recall/search (union of local + installed)

pub mod installer;
pub mod pack;
pub mod registry;
pub mod security;

// Re-export commonly used types
pub use installer::PackInstaller;
pub use pack::{Author, KnowledgePack, PackCategory, PrivacyPolicy};
pub use registry::RegistryManager;
pub use security::SecretDetector;

/// Get aggregated knowledge from all installed packs (full content).
pub fn get_installed_pack_knowledge(memory_dir: &std::path::Path) -> crate::Result<String> {
    let installer = PackInstaller::new(memory_dir);
    let knowledge_dirs = installer.get_installed_knowledge_dirs()?;

    if knowledge_dirs.is_empty() {
        return Ok(String::new());
    }

    let mut combined = String::new();

    for (pack_name, knowledge_dir) in knowledge_dirs {
        combined.push_str(&format!("## From pack: {}\n\n", pack_name));

        for category in &[
            "patterns.md",
            "solutions.md",
            "decisions.md",
            "preferences.md",
        ] {
            let file_path = knowledge_dir.join(category);
            if file_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    if !content.trim().is_empty() {
                        combined.push_str(&format!("### {}\n\n", category.replace(".md", "")));
                        combined.push_str(&content);
                        combined.push_str("\n\n");
                    }
                }
            }
        }
    }

    Ok(combined)
}
