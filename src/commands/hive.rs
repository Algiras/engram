use std::path::{Path, PathBuf};

use colored::Colorize;

use crate::cli::{HiveCommand, PackCommand, RegistryCommand};
use crate::error::{MemoryError, Result};
use crate::hive;

// ── Hive commands ───────────────────────────────────────────────────────

pub fn cmd_hive(command: HiveCommand) -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| MemoryError::Config("Could not determine home directory".into()))?;
    let memory_dir = home.join("memory");

    match command {
        HiveCommand::Registry { command } => cmd_hive_registry(command, &memory_dir),
        HiveCommand::Pack { command } => cmd_hive_pack(command, &memory_dir),
        HiveCommand::Install {
            pack,
            registry,
            scope,
        } => cmd_hive_install(&pack, registry.as_deref(), &scope, &memory_dir),
        HiveCommand::Uninstall { pack } => cmd_hive_uninstall(&pack, &memory_dir),
        HiveCommand::List => cmd_hive_list(&memory_dir),
        HiveCommand::Update { pack } => cmd_hive_update(pack.as_deref(), &memory_dir),
        HiveCommand::Browse { category, keyword } => {
            cmd_hive_browse(category.as_deref(), keyword.as_deref(), &memory_dir)
        }
        HiveCommand::Search { query } => cmd_hive_search(&query, &memory_dir),
    }
}

fn cmd_hive_registry(command: RegistryCommand, memory_dir: &Path) -> Result<()> {
    use hive::RegistryManager;

    let manager = RegistryManager::new(memory_dir);

    match command {
        RegistryCommand::Add { url } => {
            println!("{} Adding registry: {}", "→".blue(), url);
            let registry = manager.add(&url)?;
            println!(
                "{} Registry '{}' added successfully",
                "✓".green(),
                registry.name
            );
            println!("  URL: {}", registry.url);
        }
        RegistryCommand::Remove { name } => {
            println!("{} Removing registry: {}", "→".blue(), name);
            manager.remove(&name)?;
            println!("{} Registry '{}' removed", "✓".green(), name);
        }
        RegistryCommand::List => {
            let registries = manager.list()?;
            if registries.is_empty() {
                println!("No registries configured.");
                println!("\nAdd a registry with:");
                println!("  engram hive registry add owner/repo");
                return Ok(());
            }

            println!("Knowledge Pack Registries:\n");
            for reg in registries {
                println!("  {} {}", "●".blue(), reg.name.bold());
                println!("    URL: {}", reg.url);
                if let Some(updated) = reg.last_updated {
                    println!("    Last updated: {}", updated.format("%Y-%m-%d %H:%M:%S"));
                }
                println!();
            }
        }
        RegistryCommand::Update { name } => {
            if let Some(name) = name {
                println!("{} Updating registry: {}", "→".blue(), name);
                manager.update(&name)?;
                println!("{} Registry '{}' updated", "✓".green(), name);
            } else {
                println!("{} Updating all registries", "→".blue());
                let registries = manager.list()?;
                for reg in registries {
                    print!("  {} {}... ", "→".blue(), reg.name);
                    manager.update(&reg.name)?;
                    println!("{}", "✓".green());
                }
                println!("\n{} All registries updated", "✓".green());
            }
        }
    }

    Ok(())
}

fn cmd_hive_install(
    pack: &str,
    registry: Option<&str>,
    _scope: &str,
    memory_dir: &Path,
) -> Result<()> {
    use hive::PackInstaller;

    let installer = PackInstaller::new(memory_dir);

    println!("{} Installing pack: {}", "→".blue(), pack.bold());
    if let Some(reg) = registry {
        println!("  Registry: {}", reg);
    }

    let installed = installer.install(pack, registry)?;

    println!(
        "{} Pack '{}' installed successfully",
        "✓".green(),
        installed.name
    );
    println!("  Version: {}", installed.version);
    println!("  Registry: {}", installed.registry);
    println!(
        "  Installed at: {}",
        installed.installed_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!("  Path: {}", installed.path.display());

    println!("\n💡 Use 'engram recall' to access this pack's knowledge");

    Ok(())
}

fn cmd_hive_uninstall(pack: &str, memory_dir: &Path) -> Result<()> {
    use hive::PackInstaller;

    let installer = PackInstaller::new(memory_dir);

    println!("{} Uninstalling pack: {}", "→".blue(), pack.bold());
    installer.uninstall(pack)?;

    println!("{} Pack '{}' uninstalled successfully", "✓".green(), pack);

    Ok(())
}

fn cmd_hive_list(memory_dir: &Path) -> Result<()> {
    use hive::PackInstaller;

    let installer = PackInstaller::new(memory_dir);
    let packs = installer.list()?;

    if packs.is_empty() {
        println!("No packs installed.");
        println!("\nBrowse available packs with:");
        println!("  engram hive browse");
        println!("\nInstall a pack with:");
        println!("  engram hive install <pack-name>");
        return Ok(());
    }

    println!("Installed Knowledge Packs:\n");
    for pack in packs {
        println!("  {} {}", "●".green(), pack.name.bold());
        println!("    Version: {}", pack.version);
        println!("    Registry: {}", pack.registry);
        println!(
            "    Installed: {}",
            pack.installed_at.format("%Y-%m-%d %H:%M:%S")
        );
        println!("    Path: {}", pack.path.display());
        println!();
    }

    Ok(())
}

fn cmd_hive_update(pack: Option<&str>, memory_dir: &Path) -> Result<()> {
    use hive::PackInstaller;

    let installer = PackInstaller::new(memory_dir);

    if let Some(pack_name) = pack {
        println!("{} Updating pack: {}", "→".blue(), pack_name.bold());
        installer.update(pack_name)?;
        println!("{} Pack '{}' updated successfully", "✓".green(), pack_name);
    } else {
        println!("{} Updating all installed packs", "→".blue());
        let packs = installer.list()?;

        if packs.is_empty() {
            println!("No packs installed.");
            return Ok(());
        }

        for pack in packs {
            print!("  {} {}... ", "→".blue(), pack.name);
            match installer.update(&pack.name) {
                Ok(_) => println!("{}", "✓".green()),
                Err(e) => println!("{} {}", "✗".red(), e),
            }
        }

        println!("\n{} All packs updated", "✓".green());
    }

    Ok(())
}

fn cmd_hive_browse(category: Option<&str>, keyword: Option<&str>, memory_dir: &Path) -> Result<()> {
    use hive::{PackCategory, PackInstaller, RegistryManager};
    use std::str::FromStr;

    let registry_manager = RegistryManager::new(memory_dir);
    let installer = PackInstaller::new(memory_dir);

    let registries = registry_manager.list()?;
    if registries.is_empty() {
        println!("No registries configured.");
        println!("\nAdd a registry with:");
        println!("  engram hive registry add owner/repo");
        return Ok(());
    }

    // Collect all packs from all registries
    let mut all_packs = Vec::new();
    for registry in registries {
        match registry_manager.discover_packs(&registry.name) {
            Ok(packs) => {
                for pack in packs {
                    all_packs.push((registry.name.clone(), pack));
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to discover packs in '{}': {}",
                    registry.name, e
                );
            }
        }
    }

    // Filter by category if specified
    if let Some(cat_str) = category {
        let cat = PackCategory::from_str(cat_str)?;
        all_packs.retain(|(_, pack)| pack.has_category(&cat));
    }

    // Filter by keyword if specified
    if let Some(kw) = keyword {
        all_packs.retain(|(_, pack)| pack.matches_keyword(kw));
    }

    if all_packs.is_empty() {
        println!("No packs found matching criteria.");
        return Ok(());
    }

    // Get installed packs for status display
    let installed_packs = installer.list()?;
    let installed_names: std::collections::HashSet<_> =
        installed_packs.iter().map(|p| p.name.as_str()).collect();

    println!("Available Knowledge Packs:\n");
    for (registry_name, pack) in all_packs {
        let status = if installed_names.contains(pack.name.as_str()) {
            format!("[{}]", "INSTALLED".green())
        } else {
            format!("[{}]", "available".dimmed())
        };

        println!("  {} {} {}", "●".blue(), pack.name.bold(), status);
        println!("    Description: {}", pack.description);
        println!(
            "    Categories: {}",
            pack.categories
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("    Registry: {}", registry_name);
        println!("    Version: {}", pack.version);
        if !pack.keywords.is_empty() {
            println!("    Keywords: {}", pack.keywords.join(", "));
        }
        println!();
    }

    println!("\n💡 Install a pack with:");
    println!("  engram hive install <pack-name>");

    Ok(())
}

fn cmd_hive_search(query: &str, memory_dir: &Path) -> Result<()> {
    use hive::{PackInstaller, RegistryManager};

    let registry_manager = RegistryManager::new(memory_dir);
    let installer = PackInstaller::new(memory_dir);

    println!("{} Searching for: {}", "→".blue(), query.bold());

    let results = registry_manager.search_packs(query)?;

    if results.is_empty() {
        println!("\nNo packs found matching '{}'", query);
        return Ok(());
    }

    // Get installed packs for status display
    let installed_packs = installer.list()?;
    let installed_names: std::collections::HashSet<_> =
        installed_packs.iter().map(|p| p.name.as_str()).collect();

    println!("\nSearch Results:\n");
    for (registry_name, packs) in results {
        println!("From registry '{}':", registry_name.bold());
        for pack in packs {
            let status = if installed_names.contains(pack.name.as_str()) {
                format!("[{}]", "INSTALLED".green())
            } else {
                format!("[{}]", "available".dimmed())
            };

            println!("  {} {} {}", "●".blue(), pack.name.bold(), status);
            println!("    {}", pack.description);
            if !pack.keywords.is_empty() {
                println!("    Keywords: {}", pack.keywords.join(", "));
            }
            println!();
        }
    }

    println!("💡 Install a pack with:");
    println!("  engram hive install <pack-name>");

    Ok(())
}

fn cmd_hive_pack(command: PackCommand, memory_dir: &Path) -> Result<()> {
    match command {
        PackCommand::Create {
            name,
            project,
            description,
            author,
            keywords,
            categories,
            output,
        } => cmd_hive_pack_create(
            &name,
            &project,
            description.as_deref(),
            author.as_deref(),
            keywords.as_deref(),
            categories.as_deref(),
            output.as_deref(),
            memory_dir,
        ),
        PackCommand::Stats { name } => cmd_hive_pack_stats(&name, memory_dir),
        PackCommand::Publish {
            path,
            repo,
            push,
            message,
            skip_security,
        } => cmd_hive_pack_publish(
            &path,
            repo.as_deref(),
            push,
            message.as_deref(),
            skip_security,
        ),
        PackCommand::Validate { path } => cmd_hive_pack_validate(&path),
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_hive_pack_create(
    name: &str,
    project: &str,
    description: Option<&str>,
    author_name: Option<&str>,
    keywords_str: Option<&str>,
    categories_str: Option<&str>,
    output_dir: Option<&str>,
    memory_dir: &Path,
) -> Result<()> {
    use hive::{Author, KnowledgePack, PackCategory, PrivacyPolicy};
    use std::str::FromStr;

    println!("{} Creating knowledge pack: {}", "→".blue(), name.bold());

    // Verify source project exists
    let source_knowledge = memory_dir.join("knowledge").join(project);
    if !source_knowledge.exists() {
        return Err(MemoryError::Config(format!(
            "Project '{}' not found. Run 'ingest' first.",
            project
        )));
    }

    // Determine output directory
    let pack_dir = if let Some(out) = output_dir {
        PathBuf::from(out)
    } else {
        std::env::current_dir()?.join("packs").join(name)
    };

    if pack_dir.exists() {
        return Err(MemoryError::Config(format!(
            "Pack directory already exists: {}",
            pack_dir.display()
        )));
    }

    // Create pack structure
    std::fs::create_dir_all(&pack_dir)?;
    std::fs::create_dir_all(pack_dir.join(".pack"))?;
    std::fs::create_dir_all(pack_dir.join("knowledge"))?;

    // Collect metadata (with prompts if not provided)
    let desc = description
        .map(String::from)
        .unwrap_or_else(|| format!("Knowledge pack from {}", project));

    let author = Author::new(
        author_name
            .map(String::from)
            .unwrap_or_else(|| "Anonymous".to_string()),
    );

    let keywords: Vec<String> = keywords_str
        .map(|s| s.split(',').map(|k| k.trim().to_string()).collect())
        .unwrap_or_default();

    let categories: Vec<PackCategory> = categories_str
        .map(|s| {
            s.split(',')
                .filter_map(|c| PackCategory::from_str(c.trim()).ok())
                .collect()
        })
        .unwrap_or_else(|| vec![PackCategory::Patterns, PackCategory::Solutions]);

    // Create manifest
    let mut pack = KnowledgePack::new(
        name.to_string(),
        desc,
        author,
        format!("https://github.com/user/{}", name),
    );
    pack.keywords = keywords;
    pack.categories = categories.clone();

    // Save manifest
    pack.save(&pack_dir)?;

    // Copy knowledge files based on privacy settings and categories
    let privacy = PrivacyPolicy::default();
    let knowledge_dest = pack_dir.join("knowledge");

    for (category_name, should_include) in [
        ("patterns.md", privacy.share_patterns),
        ("solutions.md", privacy.share_solutions),
        ("decisions.md", privacy.share_decisions),
        ("preferences.md", privacy.share_preferences),
    ] {
        if should_include {
            let source_file = source_knowledge.join(category_name);
            let dest_file = knowledge_dest.join(category_name);

            if source_file.exists() {
                std::fs::copy(&source_file, &dest_file)?;
                println!("  {} Copied {}", "✓".green(), category_name);
            }
        }
    }

    // Scan for secrets
    println!("\n{} Scanning for secrets...", "→".blue());
    let detector = hive::SecretDetector::new()?;
    let secrets = detector.scan_directory(&knowledge_dest)?;

    if !secrets.is_empty() {
        println!("\n{} Secrets detected!", "✗".red().bold());
        println!("\nThe following potential secrets were found:\n");

        for secret in &secrets {
            println!(
                "  {} {}:{}",
                "●".red(),
                secret.file_path,
                secret.line_number
            );
            println!("    Type: {}", secret.pattern_name.yellow());
            println!("    Match: {}", secret.matched_text.dimmed());
            println!();
        }

        println!("{}", "Pack creation blocked for security.".red().bold());
        println!("\nPlease review and remove secrets, then try again.");

        // Clean up
        std::fs::remove_dir_all(&pack_dir)?;

        return Err(MemoryError::Config(format!(
            "{} secret(s) detected",
            secrets.len()
        )));
    }

    println!("  {} No secrets detected", "✓".green());

    // Create README
    let readme_content = format!(
        "# {}\n\n{}\n\n## Installation\n\n```bash\nengram hive install {}\n```\n\n## Contents\n\n",
        name, pack.description, name
    );
    std::fs::write(pack_dir.join("README.md"), readme_content)?;

    println!("\n{} Pack created successfully!", "✓".green());
    println!("  Location: {}", pack_dir.display());
    println!(
        "  Categories: {}",
        categories
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("\n💡 Next steps:");
    println!("  1. Review content: cd {}", pack_dir.display());
    println!("  2. Initialize git: git init && git add . && git commit -m 'Initial pack'");
    println!("  3. Push to GitHub: git remote add origin <url> && git push");
    println!("  4. Share: engram hive registry add <owner>/<repo>");

    Ok(())
}

fn cmd_hive_pack_stats(name: &str, memory_dir: &Path) -> Result<()> {
    use hive::PackInstaller;

    let installer = PackInstaller::new(memory_dir);
    let packs = installer.list()?;

    let pack = packs
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| MemoryError::Config(format!("Pack '{}' not installed", name)))?;

    println!("{} Pack Statistics: {}", "→".blue(), pack.name.bold());
    println!();

    // Load manifest
    let manifest_path = pack.path.join(".pack/manifest.json");
    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&content) {
            println!("  {} {}", "Name:".bold(), pack.name);
            println!("  {} {}", "Version:".bold(), pack.version);
            println!("  {} {}", "Registry:".bold(), pack.registry);

            if let Some(desc) = manifest.get("description").and_then(|v| v.as_str()) {
                println!("  {} {}", "Description:".bold(), desc);
            }
        }
    }

    println!();

    // Knowledge statistics
    let knowledge_dir = pack.path.join("knowledge");
    if knowledge_dir.exists() {
        println!("  {}", "Knowledge:".bold());

        let mut total_entries = 0;
        let mut total_size = 0;

        for category in &[
            "patterns.md",
            "solutions.md",
            "workflows.md",
            "decisions.md",
            "preferences.md",
        ] {
            let file_path = knowledge_dir.join(category);
            if file_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    let entry_count = content.matches("## Session:").count();
                    let size = content.len();

                    total_entries += entry_count;
                    total_size += size;

                    if entry_count > 0 {
                        println!(
                            "    {} {} entries ({} KB)",
                            category.replace(".md", "").cyan(),
                            entry_count,
                            size / 1024
                        );
                    }
                }
            }
        }

        println!();
        println!("  {} {} entries", "Total:".bold(), total_entries);
        println!("  {} {} KB", "Size:".bold(), total_size / 1024);
    }

    println!();
    println!(
        "  {} {}",
        "Installed:".bold(),
        pack.installed_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!("  {} {}", "Path:".bold(), pack.path.display());

    Ok(())
}

fn cmd_hive_pack_publish(
    pack_path: &str,
    repo_url: Option<&str>,
    do_push: bool,
    commit_msg: Option<&str>,
    skip_security: bool,
) -> Result<()> {
    let pack_dir = Path::new(pack_path);

    if !pack_dir.exists() {
        return Err(MemoryError::Config(format!(
            "Pack directory not found: {}",
            pack_path
        )));
    }

    println!("{} Publishing knowledge pack", "→".blue());
    println!("  Path: {}", pack_dir.display());

    // Step 1: Validate pack structure
    println!("\n{} Validating pack structure...", "→".blue());
    validate_pack_structure(pack_dir)?;
    println!("  {} Pack structure valid", "✓".green());

    // Step 2: Load manifest
    let pack = hive::KnowledgePack::load(pack_dir)?;
    println!(
        "  {} Loaded manifest: {} v{}",
        "✓".green(),
        pack.name,
        pack.version
    );

    // Step 3: Security scan (unless skipped)
    if !skip_security {
        println!("\n{} Scanning for secrets...", "→".blue());
        let detector = hive::SecretDetector::new()?;
        let knowledge_dir = pack_dir.join("knowledge");

        if knowledge_dir.exists() {
            let secrets = detector.scan_directory(&knowledge_dir)?;

            if !secrets.is_empty() {
                println!("\n{} Secrets detected!", "✗".red().bold());
                println!("\nThe following potential secrets were found:\n");

                for secret in &secrets {
                    println!(
                        "  {} {}:{}",
                        "●".red(),
                        secret.file_path,
                        secret.line_number
                    );
                    println!("    Type: {}", secret.pattern_name.yellow());
                    println!("    Match: {}", secret.matched_text.dimmed());
                    println!();
                }

                println!("{}", "Publishing blocked for security.".red().bold());
                println!("\nPlease review and remove secrets, then try again.");
                println!(
                    "Use {} to skip this check (NOT RECOMMENDED)",
                    "--skip-security".yellow()
                );

                return Err(MemoryError::Config(format!(
                    "{} secret(s) detected",
                    secrets.len()
                )));
            }

            println!("  {} No secrets detected", "✓".green());
        }
    } else {
        println!("\n{} Skipping security scan", "⚠".yellow().bold());
    }

    // Step 4: Initialize or verify git repo
    println!("\n{} Checking git repository...", "→".blue());

    let is_git_repo = pack_dir.join(".git").exists();

    if !is_git_repo {
        println!("  {} Initializing git repository...", "→".blue());

        let status = std::process::Command::new("git")
            .args(["init"])
            .current_dir(pack_dir)
            .status()?;

        if !status.success() {
            return Err(MemoryError::Config(
                "Failed to initialize git repository".into(),
            ));
        }

        println!("  {} Git repository initialized", "✓".green());

        // Create .gitignore
        std::fs::write(pack_dir.join(".gitignore"), "*.tmp\n*.swp\n.DS_Store\n")?;
    } else {
        println!("  {} Git repository exists", "✓".green());
    }

    // Step 5: Commit changes
    println!("\n{} Committing changes...", "→".blue());

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(pack_dir)
        .status()?;

    let default_msg = format!("Update {} v{}", pack.name, pack.version);
    let message = commit_msg.unwrap_or(&default_msg);

    let commit_status = std::process::Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(pack_dir)
        .status()?;

    if commit_status.success() {
        println!("  {} Changes committed", "✓".green());
    } else {
        println!("  {} No changes to commit", "ℹ".cyan());
    }

    // Step 6: Set up remote if provided
    if let Some(url) = repo_url {
        println!("\n{} Setting up remote repository...", "→".blue());

        // Check if remote exists
        let has_remote = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(pack_dir)
            .status()?
            .success();

        if !has_remote {
            println!("  {} Adding remote: {}", "→".blue(), url);

            let status = std::process::Command::new("git")
                .args(["remote", "add", "origin", url])
                .current_dir(pack_dir)
                .status()?;

            if !status.success() {
                return Err(MemoryError::Config("Failed to add git remote".into()));
            }

            println!("  {} Remote added", "✓".green());
        } else {
            println!("  {} Remote already configured", "✓".green());
        }
    }

    // Step 7: Push if requested
    if do_push {
        println!("\n{} Pushing to remote...", "→".blue());

        let status = std::process::Command::new("git")
            .args(["push", "-u", "origin", "HEAD"])
            .current_dir(pack_dir)
            .status()?;

        if !status.success() {
            return Err(MemoryError::Config(
                "Failed to push to remote. Check git remote configuration.".into(),
            ));
        }

        println!("  {} Pushed successfully", "✓".green());
    }

    // Step 8: Tag version
    println!("\n{} Creating version tag...", "→".blue());

    let tag = format!("v{}", pack.version);
    let tag_status = std::process::Command::new("git")
        .args(["tag", "-a", &tag, "-m", &format!("Release {}", tag)])
        .current_dir(pack_dir)
        .status()?;

    if tag_status.success() {
        println!("  {} Tagged as {}", "✓".green(), tag.cyan());

        if do_push {
            std::process::Command::new("git")
                .args(["push", "origin", &tag])
                .current_dir(pack_dir)
                .status()?;
            println!("  {} Tag pushed", "✓".green());
        }
    }

    println!("\n{} Pack published successfully!", "✓".green().bold());
    println!("\n💡 Share your pack:");
    if let Some(url) = repo_url {
        println!("  Users can install with:");
        println!("  {}", format!("engram hive registry add {}", url).cyan());
    } else {
        println!("  1. Push to GitHub: git push -u origin main");
        println!("  2. Share the repository URL");
        println!("  3. Users can add: engram hive registry add <owner>/<repo>");
    }

    Ok(())
}

fn cmd_hive_pack_validate(pack_path: &str) -> Result<()> {
    let pack_dir = Path::new(pack_path);

    println!("{} Validating pack: {}", "→".blue(), pack_dir.display());
    println!();

    validate_pack_structure(pack_dir)?;

    println!("{} Pack is valid!", "✓".green().bold());

    Ok(())
}

fn validate_pack_structure(pack_dir: &Path) -> Result<()> {
    // Check 1: Directory exists
    if !pack_dir.exists() {
        return Err(MemoryError::Config(format!(
            "Pack directory not found: {}",
            pack_dir.display()
        )));
    }

    // Check 2: Manifest exists and is valid
    let manifest_path = pack_dir.join(".pack/manifest.json");
    if !manifest_path.exists() {
        return Err(MemoryError::Config(
            "Missing .pack/manifest.json file".into(),
        ));
    }

    let _pack = hive::KnowledgePack::load(pack_dir)?;

    // Check 3: Knowledge directory exists
    let knowledge_dir = pack_dir.join("knowledge");
    if !knowledge_dir.exists() {
        return Err(MemoryError::Config("Missing knowledge/ directory".into()));
    }

    // Check 4: At least one knowledge file exists
    let has_knowledge = [
        "patterns.md",
        "solutions.md",
        "workflows.md",
        "decisions.md",
        "preferences.md",
    ]
    .iter()
    .any(|f| knowledge_dir.join(f).exists());

    if !has_knowledge {
        return Err(MemoryError::Config(
            "No knowledge files found in knowledge/ directory".into(),
        ));
    }

    // Check 5: README exists
    if !pack_dir.join("README.md").exists() {
        println!("  {} README.md missing (recommended)", "⚠".yellow());
    }

    // Check 6: Categories match available knowledge
    let mut found_categories = Vec::new();
    for (file, category) in [
        ("patterns.md", "Patterns"),
        ("solutions.md", "Solutions"),
        ("workflows.md", "Workflows"),
        ("decisions.md", "Decisions"),
        ("preferences.md", "Preferences"),
    ] {
        if knowledge_dir.join(file).exists() {
            found_categories.push(category);
        }
    }

    println!("  {} Manifest valid", "✓".green());
    println!("  {} Knowledge directory exists", "✓".green());
    println!(
        "  {} Found categories: {}",
        "✓".green(),
        found_categories.join(", ")
    );

    Ok(())
}
