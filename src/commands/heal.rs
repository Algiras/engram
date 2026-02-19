use colored::Colorize;

use crate::commands::hooks::cmd_hooks_install;
use crate::config::Config;
use crate::error::{MemoryError, Result};
use crate::health;

/// Detect and repair all engram issues: hook drift, stale context, missing embeddings.
pub fn cmd_heal(config: &Config, check_only: bool) -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| MemoryError::Config("Could not determine home directory".into()))?;

    println!("{}", "🩺 Engram Self-Heal".green().bold());
    println!("{}", "=".repeat(60));
    println!();

    // --- 1. Hook integrity check ---
    println!("{}", "🔗 Hook Integrity".cyan().bold());
    let hook_issues = health::check_hooks_health(&home);
    if hook_issues.is_empty() {
        println!("   {} All hooks registered\n", "✓".green());
    } else {
        for issue in &hook_issues {
            println!("   {} {}", "✗".red(), issue.description);
        }
        if !check_only {
            print!("   Reinstalling hooks... ");
            match cmd_hooks_install() {
                Ok(()) => println!("{}", "ok".green()),
                Err(e) => println!("{}: {}", "error".red(), e),
            }
        } else {
            println!("   💡 Run {} to fix", "engram hooks install".cyan());
        }
        println!();
    }

    // --- 2. Per-project health check ---
    let knowledge_dir = config.memory_dir.join("knowledge");
    let projects: Vec<String> = std::fs::read_dir(&knowledge_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name != crate::config::GLOBAL_DIR)
        .collect();

    if projects.is_empty() {
        println!("   {} No projects found\n", "ℹ".cyan());
    } else {
        println!("{}", "📊 Project Health".cyan().bold());
        for project in &projects {
            let report = health::check_project_health(&config.memory_dir, project)?;

            if report.issues.is_empty() {
                println!("   {} {} — healthy", "✓".green(), project.bold());
                continue;
            }

            let auto_fixable: Vec<_> = report.issues.iter().filter(|i| i.auto_fixable).collect();
            println!(
                "   {} {} — {} issue(s) ({} auto-fixable)",
                "!".yellow(),
                project.bold(),
                report.issues.len(),
                auto_fixable.len()
            );
            for issue in &report.issues {
                println!("     {} {}", "•".yellow(), issue.description);
            }

            if !check_only && !auto_fixable.is_empty() {
                print!("     Fixing... ");
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;
                let fixed =
                    rt.block_on(health::auto_fix_issues(config, project, &report.issues))?;
                for fix in &fixed {
                    println!("     {} {}", "✓".green(), fix);
                }
                if fixed.is_empty() {
                    println!("{}", "nothing fixed".dimmed());
                }
            }
        }
        println!();
    }

    if check_only {
        println!("💡 Run {} to apply fixes", "engram heal".cyan());
    } else {
        println!("{}", "✓ Heal complete".green().bold());
    }

    Ok(())
}
