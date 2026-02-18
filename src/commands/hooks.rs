use std::path::{Path, PathBuf};

use colored::Colorize;

use crate::error::{self, Result};
use crate::extractor;
use crate::parser;

// ── Helpers (also used by cmd_inject in core.rs) ────────────────────────

pub(crate) fn find_claude_project_dir(
    claude_projects_dir: &Path,
    project_name: &str,
) -> Result<Option<PathBuf>> {
    if !claude_projects_dir.exists() {
        return Ok(None);
    }

    for entry in std::fs::read_dir(claude_projects_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let decoded = parser::discovery::decode_project_name(&dir_name);
        if decoded == project_name {
            return Ok(Some(entry.path()));
        }
    }

    Ok(None)
}

/// Build a lightweight context string from raw knowledge files (no LLM).
/// Used as fallback when context.md doesn't exist but knowledge files do.
/// Returns None if no knowledge files exist or all are empty/expired.
pub(crate) fn build_raw_context(project: &str, project_knowledge_dir: &Path) -> Option<String> {
    use extractor::knowledge::{parse_session_blocks, partition_by_expiry, reconstruct_blocks};

    let read_and_filter = |path: &Path| -> String {
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let (preamble, blocks) = parse_session_blocks(&raw);
        let (active, _) = partition_by_expiry(blocks);
        reconstruct_blocks(&preamble, &active)
    };

    let decisions = read_and_filter(&project_knowledge_dir.join("decisions.md"));
    let solutions = read_and_filter(&project_knowledge_dir.join("solutions.md"));
    let patterns = read_and_filter(&project_knowledge_dir.join("patterns.md"));

    if decisions.trim().is_empty() && solutions.trim().is_empty() && patterns.trim().is_empty() {
        return None;
    }

    let mut out = format!("# {} - Project Context (raw, not synthesized)\n\n", project);

    if !decisions.trim().is_empty() {
        out.push_str(&decisions);
        out.push_str("\n\n");
    }
    if !solutions.trim().is_empty() {
        out.push_str(&solutions);
        out.push_str("\n\n");
    }
    if !patterns.trim().is_empty() {
        out.push_str(&patterns);
        out.push_str("\n\n");
    }

    Some(out)
}

// ── Hooks commands ──────────────────────────────────────────────────────

const HOOK_SCRIPT: &str = include_str!("../../hooks/engram-hook.sh");
const INJECT_SCRIPT: &str = include_str!("../../hooks/inject-context.sh");
const SESSION_END_SCRIPT: &str = include_str!("../../hooks/session-end-hook.sh");

pub fn cmd_hooks_install() -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;

    let hooks_dir = home.join(".claude").join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    // Write hook scripts
    let hook_path = hooks_dir.join("engram-hook.sh");
    std::fs::write(&hook_path, HOOK_SCRIPT)?;
    set_executable(&hook_path)?;

    let inject_path = hooks_dir.join("inject-context.sh");
    std::fs::write(&inject_path, INJECT_SCRIPT)?;
    set_executable(&inject_path)?;

    let session_end_path = hooks_dir.join("session-end-hook.sh");
    std::fs::write(&session_end_path, SESSION_END_SCRIPT)?;
    set_executable(&session_end_path)?;

    // Update settings.json
    let settings_path = home.join(".claude").join("settings.json");
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({})
    };

    let hooks = settings
        .as_object_mut()
        .ok_or_else(|| error::MemoryError::Config("settings.json is not an object".into()))?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    // Add SessionStart hook for inject
    add_hook_entry(hooks, "SessionStart", &inject_path.to_string_lossy())?;

    // Add PostToolUse hook for auto-ingest
    add_hook_entry(hooks, "PostToolUse", &hook_path.to_string_lossy())?;

    // Add SessionEnd hook for full knowledge extraction
    add_hook_entry(hooks, "Stop", &session_end_path.to_string_lossy())?;

    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;

    println!("{} Hooks installed:", "Done!".green().bold());
    println!("  {} -> {}", "SessionStart".cyan(), inject_path.display());
    println!("  {} -> {}", "PostToolUse".cyan(), hook_path.display());
    println!("  {} -> {}", "Stop".cyan(), session_end_path.display());
    println!(
        "\n  Settings updated: {}",
        settings_path.display().to_string().dimmed()
    );

    Ok(())
}

pub fn cmd_hooks_uninstall() -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;

    let hooks_dir = home.join(".claude").join("hooks");

    // Delete hook scripts
    let hook_path = hooks_dir.join("engram-hook.sh");
    let inject_path = hooks_dir.join("inject-context.sh");
    let session_end_path = hooks_dir.join("session-end-hook.sh");

    let mut removed = Vec::new();
    if hook_path.exists() {
        std::fs::remove_file(&hook_path)?;
        removed.push("engram-hook.sh");
    }
    if inject_path.exists() {
        std::fs::remove_file(&inject_path)?;
        removed.push("inject-context.sh");
    }
    if session_end_path.exists() {
        std::fs::remove_file(&session_end_path)?;
        removed.push("session-end-hook.sh");
    }

    // Update settings.json
    let settings_path = home.join(".claude").join("settings.json");
    if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        let mut settings: serde_json::Value = serde_json::from_str(&content)?;

        if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
            for (_key, entries) in hooks.iter_mut() {
                if let Some(arr) = entries.as_array_mut() {
                    arr.retain(|entry| {
                        // Check nested hooks array for engram commands
                        let entry_str = serde_json::to_string(entry).unwrap_or_default();
                        !entry_str.contains("engram")
                    });
                }
            }
        }

        std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
    }

    if removed.is_empty() {
        println!("{} No hooks were installed.", "Note:".yellow());
    } else {
        println!("{} Hooks uninstalled:", "Done!".green().bold());
        for name in &removed {
            println!("  Removed {}", name);
        }
        println!("  Settings updated.");
    }

    Ok(())
}

pub fn cmd_hooks_status() -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;

    let hooks_dir = home.join(".claude").join("hooks");
    let hook_path = hooks_dir.join("engram-hook.sh");
    let inject_path = hooks_dir.join("inject-context.sh");
    let session_end_path = hooks_dir.join("session-end-hook.sh");

    println!("{}", "Engram Hooks Status".green().bold());
    println!("{}", "=".repeat(50));

    let check = |path: &Path, name: &str, event: &str| {
        if path.exists() {
            println!("  {} {} ({})", "installed".green(), name, event.cyan());
        } else {
            println!(
                "  {} {} ({})",
                "not installed".yellow(),
                name,
                event.dimmed()
            );
        }
    };

    check(&inject_path, "inject-context.sh", "SessionStart");
    check(&hook_path, "engram-hook.sh", "PostToolUse");
    check(&session_end_path, "session-end-hook.sh", "Stop");

    // Check settings.json
    let settings_path = home.join(".claude").join("settings.json");
    if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        let has_hooks = content.contains("engram");
        if has_hooks {
            println!(
                "\n  Settings: {} entries found in {}",
                "engram".cyan(),
                settings_path.display().to_string().dimmed()
            );
        } else {
            println!(
                "\n  Settings: {} in {}",
                "no engram entries".yellow(),
                settings_path.display().to_string().dimmed()
            );
        }
    } else {
        println!(
            "\n  Settings: {}",
            "~/.claude/settings.json not found".yellow()
        );
    }

    Ok(())
}

/// Add a hook entry to a hook event array in settings.json, idempotently.
fn add_hook_entry(hooks: &mut serde_json::Value, event: &str, command: &str) -> Result<()> {
    let event_hooks = hooks
        .as_object_mut()
        .ok_or_else(|| error::MemoryError::Config("hooks is not an object".into()))?
        .entry(event)
        .or_insert_with(|| serde_json::json!([]));

    let arr = event_hooks
        .as_array_mut()
        .ok_or_else(|| error::MemoryError::Config(format!("hooks.{} is not an array", event)))?;

    // Check if already installed (look for "engram" in any entry's command)
    let already_installed = arr.iter().any(|entry| {
        let entry_str = serde_json::to_string(entry).unwrap_or_default();
        entry_str.contains("engram")
    });

    if !already_installed {
        arr.push(serde_json::json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": command
            }]
        }));
    }

    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}
