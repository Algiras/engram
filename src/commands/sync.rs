use colored::Colorize;

use crate::config::Config;
use crate::error::{MemoryError, Result};
use crate::sync;

pub fn cmd_sync_push(
    config: &Config,
    project: &str,
    gist_id: Option<&str>,
    description: &str,
) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        let client = sync::GistClient::from_env()?;
        let files = sync::read_knowledge_files(&config.memory_dir, project)?;

        if files.is_empty() {
            eprintln!(
                "{} No knowledge found for '{}'. Run 'ingest' first.",
                "Not found:".yellow(),
                project
            );
            return Ok(());
        }

        let gist = if let Some(id) = gist_id {
            println!("{} Updating gist {}...", "Syncing".green().bold(), id);
            client.update_gist(id, Some(description), files).await?
        } else {
            println!("{} Creating new private gist...", "Syncing".green().bold());
            client.create_gist(description, files).await?
        };

        println!(
            "{} Pushed {} knowledge to gist",
            "Done!".green().bold(),
            project
        );
        println!("  Gist ID:  {}", gist.id.cyan());
        println!("  URL:      {}", gist.html_url.cyan());
        println!("\nTo pull on another machine:");
        println!(
            "  {}",
            format!("engram sync pull {} {}", project, gist.id).cyan()
        );

        Ok(())
    })
}

pub fn cmd_sync_pull(config: &Config, project: &str, gist_id: &str, force: bool) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        let client = sync::GistClient::from_env()?;

        println!("{} Fetching gist {}...", "Syncing".green().bold(), gist_id);
        let gist = client.get_gist(gist_id).await?;

        let knowledge_dir = config.memory_dir.join("knowledge").join(project);
        if knowledge_dir.exists() && !force {
            eprintln!(
                "{} Knowledge already exists for '{}'. Use --force to overwrite.",
                "Warning:".yellow(),
                project
            );
            return Ok(());
        }

        sync::write_knowledge_files(&config.memory_dir, project, &gist.files)?;

        println!(
            "{} Pulled {} knowledge from gist",
            "Done!".green().bold(),
            project
        );
        println!("  {} files synced", gist.files.len());
        println!("\nView with:");
        println!("  {}", format!("engram recall {}", project).cyan());

        Ok(())
    })
}

pub fn cmd_sync_list(_config: &Config, project: &str) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        let client = sync::GistClient::from_env()?;

        println!(
            "{} Listing gists for '{}'...",
            "Searching".green().bold(),
            project
        );
        let gists = client.list_gists().await?;

        let matching: Vec<_> = gists
            .iter()
            .filter(|g| g.description.contains(project) || g.files.contains_key("metadata.json"))
            .collect();

        if matching.is_empty() {
            println!("{} No gists found for '{}'", "Not found:".yellow(), project);
            return Ok(());
        }

        println!("\n{} gist(s) found:\n", matching.len());
        for gist in matching {
            println!("  {} {}", "ID:".cyan(), gist.id);
            println!("  {} {}", "Description:".cyan(), gist.description);
            println!("  {} {}", "URL:".cyan(), gist.html_url);
            println!("  {} {} file(s)", "Files:".cyan(), gist.files.len());
            println!("  {} {}", "Private:".cyan(), !gist.public);
            println!();
        }

        Ok(())
    })
}

pub fn cmd_sync_clone(config: &Config, gist_id: &str, project: &str) -> Result<()> {
    cmd_sync_pull(config, project, gist_id, false)
}

pub fn cmd_sync_history(gist_id: &str, version: Option<&str>) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        let client = sync::GistClient::from_env()?;

        if let Some(ver) = version {
            // Show specific version
            println!("{} Fetching version {}...", "Loading".green().bold(), ver);
            let gist = client.get_gist_version(gist_id, ver).await?;

            println!("\n{} Version {}", "Gist:".green().bold(), ver);
            println!("{}", "=".repeat(60));
            println!("Description: {}", gist.description);
            println!("Files: {}", gist.files.len());
            println!("\nFiles in this version:");
            for (filename, file) in &gist.files {
                let size = file.content.as_ref().map(|c| c.len()).unwrap_or(0);
                println!("  {} ({} bytes)", filename.cyan(), size);
            }
        } else {
            // Show history
            println!(
                "{} Fetching history for {}...",
                "Loading".green().bold(),
                gist_id
            );
            let history = client.get_gist_history(gist_id).await?;

            if history.is_empty() {
                println!("{} No history found", "Not found:".yellow());
                return Ok(());
            }

            println!("\n{} Version History", "Gist:".green().bold());
            println!("{}", "=".repeat(60));
            println!("{} versions found\n", history.len());

            for (i, entry) in history.iter().enumerate() {
                let user = entry
                    .user
                    .as_ref()
                    .map(|u| u.login.as_str())
                    .unwrap_or("unknown");
                let time = chrono::DateTime::parse_from_rfc3339(&entry.committed_at)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|_| entry.committed_at.clone());

                println!(
                    "  {} {} ({})",
                    format!("{}.", i + 1).dimmed(),
                    entry.version.cyan(),
                    time.dimmed()
                );
                println!("     By: {}", user);

                if let Some(ref status) = entry.change_status {
                    if let (Some(add), Some(del)) = (status.additions, status.deletions) {
                        println!(
                            "     Changes: {} {} ",
                            format!("+{}", add).green(),
                            format!("-{}", del).red()
                        );
                    }
                }
                println!();
            }

            println!("\nTo view a specific version:");
            println!(
                "  {}",
                format!("engram sync history {} --version <version>", gist_id).cyan()
            );
            println!("\nTo restore a version:");
            println!(
                "  {}",
                format!(
                    "engram sync pull <project> {}",
                    history.first().unwrap().version
                )
                .cyan()
            );
        }

        Ok(())
    })
}

pub fn cmd_sync_push_repo(
    config: &Config,
    project: &str,
    repo: &str,
    message: Option<&str>,
    push_remote: bool,
) -> Result<()> {
    let expanded = shellexpand::tilde(repo);
    let repo_path = std::path::PathBuf::from(expanded.as_ref());

    println!(
        "{} Syncing {} to git repo {}...",
        "Pushing".green().bold(),
        project,
        repo_path.display()
    );

    sync::push_to_git_repo(
        &config.memory_dir,
        project,
        &repo_path,
        message,
        push_remote,
    )?;

    println!(
        "{} Pushed {} knowledge to {}",
        "Done!".green().bold(),
        project,
        repo_path.display()
    );

    if push_remote {
        println!("  Changes pushed to remote");
    }

    Ok(())
}

pub fn cmd_sync_pull_repo(
    config: &Config,
    project: &str,
    repo: &str,
    fetch_remote: bool,
    branch: &str,
) -> Result<()> {
    let expanded = shellexpand::tilde(repo);
    let repo_path = std::path::PathBuf::from(expanded.as_ref());

    println!(
        "{} Syncing {} from git repo {}...",
        "Pulling".green().bold(),
        project,
        repo_path.display()
    );

    sync::pull_from_git_repo(
        &config.memory_dir,
        project,
        &repo_path,
        fetch_remote,
        branch,
    )?;

    println!(
        "{} Pulled {} knowledge from {}",
        "Done!".green().bold(),
        project,
        repo_path.display()
    );

    println!("\nView with:");
    println!("  {}", format!("engram recall {}", project).cyan());

    Ok(())
}

pub fn cmd_sync_init_repo(repo: &str) -> Result<()> {
    let expanded = shellexpand::tilde(repo);
    let repo_path = std::path::PathBuf::from(expanded.as_ref());

    println!(
        "{} Initializing git repository at {}...",
        "Creating".green().bold(),
        repo_path.display()
    );

    sync::init_git_repo(&repo_path)?;

    println!("{} Git repository initialized", "Done!".green().bold());
    println!("  Path: {}", repo_path.display().to_string().cyan());
    println!("\nNext steps:");
    println!("  1. {} (optional)", "git remote add origin <url>".dimmed());
    println!(
        "  2. {}",
        format!("engram sync push-repo <project> {}", repo).cyan()
    );

    Ok(())
}
