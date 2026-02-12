// Hive Mind - Distributed Memory Sharing System
//
// Stage 1: Read-Only Hive
// - Registry management (add, list, update registries)
// - Pack installation (clone from Git, copy to local)
// - Knowledge discovery (browse, search packs)
// - Integration with recall/search (union of local + installed)

pub mod pack;
pub mod registry;
pub mod installer;

// Re-export commonly used types
pub use pack::{Author, KnowledgePack, PackCategory, PrivacyPolicy};
pub use registry::{Registry, RegistryManager};
pub use installer::{InstalledPack, PackInstaller};
