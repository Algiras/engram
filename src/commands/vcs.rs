use colored::Colorize;

use crate::error::Result;
use crate::vcs::{MemoryVcs, CATEGORIES};

fn memory_dir() -> Result<std::path::PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| {
        crate::error::MemoryError::Config("Could not determine home directory".into())
    })?;
    Ok(home.join("memory"))
}

fn resolve_project(project: Option<&str>) -> Result<String> {
    match project {
        Some(p) => Ok(p.to_string()),
        None => std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .ok_or_else(|| {
                crate::error::MemoryError::Config(
                    "Could not determine project name from current directory".into(),
                )
            }),
    }
}

// ── Init ──────────────────────────────────────────────────────────────────

pub fn cmd_mem_init(project: Option<&str>) -> Result<()> {
    let project = resolve_project(project)?;
    let mem_dir = memory_dir()?;
    let vcs = MemoryVcs::new(&mem_dir, &project);

    if vcs.is_initialized() {
        println!(
            "{} VCS already initialized for '{}'.",
            "Info:".cyan(),
            project
        );
        return Ok(());
    }

    vcs.init()?;
    println!(
        "{} Initialized VCS for '{}' at {}",
        "Done!".green().bold(),
        project,
        vcs.vcs_dir().display()
    );
    Ok(())
}

// ── Status ────────────────────────────────────────────────────────────────

pub fn cmd_mem_status(project: Option<&str>) -> Result<()> {
    let project = resolve_project(project)?;
    let mem_dir = memory_dir()?;
    let vcs = MemoryVcs::new(&mem_dir, &project);
    let status = vcs.status()?;

    // Header
    if status.detached_head {
        println!(
            "HEAD detached at {}",
            status.current_branch[..status.current_branch.len().min(8)].yellow()
        );
    } else {
        println!("On branch {}", status.current_branch.cyan().bold());
    }
    match &status.head_hash {
        Some(h) => println!("HEAD: {}", &h[..h.len().min(8)].yellow()),
        None => println!("{}", "(no commits yet)".dimmed()),
    }
    println!();

    // Staged
    if !status.staged.is_empty() {
        println!("{}", "Changes staged for commit:".green().bold());
        println!(
            "  {}",
            "(use \"engram mem commit <project> -m <msg>\" to commit)".dimmed()
        );
        for s in &status.staged {
            println!(
                "    {} {} [{}] ({}) {}",
                "staged:".green(),
                s.session_id.cyan(),
                s.categories.join(", ").dimmed(),
                s.timestamp.dimmed(),
                s.preview.chars().take(60).collect::<String>().dimmed()
            );
        }
        println!();
    }

    // Unstaged new
    if !status.unstaged_new.is_empty() {
        println!("{}", "Changes not staged for commit:".yellow().bold());
        println!(
            "  {}",
            "(use \"engram mem stage <project> <id>...\" to stage, or \"engram mem commit <project> -a -m <msg>\" to commit all)".dimmed()
        );
        for s in &status.unstaged_new {
            println!(
                "    {} {} [{}] ({}) {}",
                "new:".yellow(),
                s.session_id.cyan(),
                s.categories.join(", ").dimmed(),
                s.timestamp.dimmed(),
                s.preview.chars().take(60).collect::<String>().dimmed()
            );
        }
        println!();
    }

    // Removed from HEAD
    if !status.unstaged_removed.is_empty() {
        println!("{}", "Deleted since last commit:".red().bold());
        for id in &status.unstaged_removed {
            println!("    {} {}", "deleted:".red(), id.cyan());
        }
        println!();
    }

    if status.staged.is_empty()
        && status.unstaged_new.is_empty()
        && status.unstaged_removed.is_empty()
    {
        println!("{}", "Nothing to commit — working tree is clean.".dimmed());
    }

    Ok(())
}

// ── Stage ─────────────────────────────────────────────────────────────────

pub fn cmd_mem_stage(project: &str, sessions: &[String], all: bool) -> Result<()> {
    let mem_dir = memory_dir()?;
    let vcs = MemoryVcs::new(&mem_dir, project);

    if all {
        let n = vcs.stage_all_new()?;
        if n == 0 {
            println!("{} Nothing new to stage.", "Info:".cyan());
        } else {
            println!("{} Staged {} new session(s).", "Done!".green().bold(), n);
        }
        return Ok(());
    }

    if sessions.is_empty() {
        eprintln!(
            "{} Provide session IDs or use --all to stage all new sessions.",
            "Error:".red()
        );
        return Ok(());
    }

    let ids: Vec<&str> = sessions.iter().map(|s| s.as_str()).collect();
    let n = vcs.stage_sessions(&ids)?;
    println!("{} Staged {} session(s).", "Done!".green().bold(), n);
    Ok(())
}

// ── Commit ────────────────────────────────────────────────────────────────

pub fn cmd_mem_commit(
    project: &str,
    message: &str,
    all_new: bool,
    session_id: Option<&str>,
) -> Result<()> {
    let mem_dir = memory_dir()?;
    let vcs = MemoryVcs::new(&mem_dir, project);

    let explicit_ids = session_id.map(|id| vec![id.to_string()]);

    let commit = vcs.commit(message, explicit_ids, all_new)?;

    println!(
        "{} {} (branch: {})",
        "commit".yellow(),
        commit.hash.cyan().bold(),
        commit.branch.green()
    );
    println!(
        "Date:   {}",
        commit
            .timestamp
            .format("%Y-%m-%d %H:%M:%S UTC")
            .to_string()
            .dimmed()
    );
    println!();
    println!("    {}", message);
    println!();
    println!("    {} session(s) in this commit", commit.session_ids.len());
    Ok(())
}

// ── Log ───────────────────────────────────────────────────────────────────

pub fn cmd_mem_log(project: &str, limit: usize, verbose: bool, grep: Option<&str>) -> Result<()> {
    let mem_dir = memory_dir()?;
    let vcs = MemoryVcs::new(&mem_dir, project);
    let commits = vcs.log(None, limit, grep)?;

    if commits.is_empty() {
        println!("{}", "(no commits yet)".dimmed());
        return Ok(());
    }

    let head_hash = vcs.head_hash()?;
    for commit in &commits {
        let is_head = head_hash.as_deref() == Some(&commit.hash);
        let head_marker = if is_head {
            format!(" {}", "(HEAD)".cyan())
        } else {
            String::new()
        };
        println!(
            "{} {}{}",
            "commit".yellow(),
            commit.hash.cyan().bold(),
            head_marker
        );
        println!(
            "Branch: {}  Date: {}",
            commit.branch.green(),
            commit
                .timestamp
                .format("%Y-%m-%d %H:%M UTC")
                .to_string()
                .dimmed()
        );
        if let Some(parent) = &commit.parent {
            println!("Parent: {}", &parent[..parent.len().min(8)].dimmed());
        }
        println!();
        println!("    {}", commit.message);
        println!();

        if verbose {
            println!("    Sessions ({}):", commit.session_ids.len());
            for id in &commit.session_ids {
                println!("      - {}", id.cyan());
            }
            println!("    Categories:");
            for (cat, hash) in &commit.category_hashes {
                println!("      {} {}", cat.dimmed(), hash.dimmed());
            }
            println!();
        } else {
            println!(
                "    {} session(s): {}",
                commit.session_ids.len(),
                commit
                    .session_ids
                    .iter()
                    .take(3)
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
                    + if commit.session_ids.len() > 3 {
                        " ..."
                    } else {
                        ""
                    }
            );
            println!();
        }
    }

    Ok(())
}

// ── Branch ────────────────────────────────────────────────────────────────

pub fn cmd_mem_branch(project: &str, create: Option<&str>, delete: Option<&str>) -> Result<()> {
    let mem_dir = memory_dir()?;
    let vcs = MemoryVcs::new(&mem_dir, project);

    if let Some(name) = create {
        vcs.create_branch(name, None)?;
        println!(
            "{} Created branch '{}'.",
            "Done!".green().bold(),
            name.cyan()
        );
        return Ok(());
    }

    if let Some(name) = delete {
        vcs.delete_branch(name)?;
        println!(
            "{} Deleted branch '{}'.",
            "Done!".green().bold(),
            name.cyan()
        );
        return Ok(());
    }

    // List branches
    let branches = vcs.list_branches()?;
    if branches.is_empty() {
        println!("{}", "(no branches yet)".dimmed());
    } else {
        for b in &branches {
            let marker = if b.is_current { "* " } else { "  " };
            let hash_str = b
                .hash
                .as_deref()
                .map(|h| format!(" ({})", &h[..h.len().min(8)]))
                .unwrap_or_default();
            if b.is_current {
                println!(
                    "{}{}{}",
                    marker.green().bold(),
                    b.name.green().bold(),
                    hash_str.dimmed()
                );
            } else {
                println!("{}{}{}", marker, b.name, hash_str.dimmed());
            }
        }
    }
    Ok(())
}

// ── Checkout ──────────────────────────────────────────────────────────────

pub fn cmd_mem_checkout(project: &str, target: &str, dry_run: bool, force: bool) -> Result<()> {
    let mem_dir = memory_dir()?;
    let vcs = MemoryVcs::new(&mem_dir, project);
    let result = vcs.checkout(target, dry_run, force)?;

    if dry_run {
        println!("{} Dry run — no changes applied.", "Dry run:".cyan());
        println!(
            "  Would switch from {} → {}",
            result.previous_branch.yellow(),
            result.new_ref.cyan()
        );
        println!(
            "  Blocks to add: {}, blocks to remove: {}",
            result.blocks_added.to_string().green(),
            result.blocks_removed.to_string().red()
        );
        if !result.conflicts.is_empty() {
            println!(
                "  Conflicts (target wins): {}",
                result.conflicts.join(", ").red()
            );
        }
    } else {
        println!(
            "{} Switched from {} → {}",
            "Done!".green().bold(),
            result.previous_branch.yellow(),
            result.new_ref.cyan()
        );
        println!(
            "  {} block(s) added, {} block(s) removed",
            result.blocks_added.to_string().green(),
            result.blocks_removed.to_string().red()
        );
        if !result.conflicts.is_empty() {
            println!(
                "{} {} session(s) had conflicts (target version kept): {}",
                "Warning:".yellow(),
                result.conflicts.len(),
                result.conflicts.join(", ").yellow()
            );
        }
        if result.blocks_added > 0 || result.blocks_removed > 0 {
            println!(
                "  {}",
                "context.md invalidated — run 'engram regen' to rebuild.".dimmed()
            );
        }
    }
    Ok(())
}

// ── Show ──────────────────────────────────────────────────────────────────

pub fn cmd_mem_show(project: &str, target: Option<&str>, category: Option<&str>) -> Result<()> {
    let mem_dir = memory_dir()?;
    let vcs = MemoryVcs::new(&mem_dir, project);

    // Resolve target: default to HEAD
    let resolved = match target {
        Some(t) => t.to_string(),
        None => match vcs.head_hash()? {
            Some(h) => h,
            None => {
                println!("{}", "(no commits yet)".dimmed());
                return Ok(());
            }
        },
    };

    let output = vcs.show(&resolved, category)?;

    // Re-print with some color on the first line (commit hash)
    for (i, line) in output.lines().enumerate() {
        if i == 0 {
            // "commit <hash> — branch: <branch>"
            if let Some(rest) = line.strip_prefix("commit ") {
                let parts: Vec<&str> = rest.splitn(2, " — ").collect();
                if parts.len() == 2 {
                    println!(
                        "{} {} — {}",
                        "commit".yellow(),
                        parts[0].cyan().bold(),
                        parts[1].dimmed()
                    );
                    continue;
                }
            }
        }
        if line.starts_with("──") {
            println!("{}", line.green().bold());
        } else if line.starts_with("  [") {
            // session block line: "  [id] (ts)  preview"
            println!("{}", line);
        } else {
            println!("{}", line);
        }
    }
    Ok(())
}

// ── Diff ──────────────────────────────────────────────────────────────────

pub fn cmd_mem_diff(
    project: &str,
    from: Option<&str>,
    to: Option<&str>,
    category: Option<&str>,
) -> Result<()> {
    let mem_dir = memory_dir()?;
    let vcs = MemoryVcs::new(&mem_dir, project);

    let from_label = from.unwrap_or("HEAD");
    let to_label = to.unwrap_or("working");
    println!(
        "diff {} {} → {}",
        project.cyan(),
        from_label.yellow(),
        to_label.yellow()
    );
    if let Some(cat) = category {
        println!("category filter: {}", cat.dimmed());
    } else {
        println!("categories: {}", CATEGORIES.join(", ").dimmed());
    }
    println!();

    let output = vcs.diff(from, to, category)?;
    print!("{}", output);
    Ok(())
}
