use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::{MemoryError, Result};

fn pid_file(config: &Config) -> PathBuf {
    config.memory_dir.join("daemon.pid")
}

fn log_file(config: &Config) -> PathBuf {
    config.memory_dir.join("daemon.log")
}

fn cfg_file(config: &Config) -> PathBuf {
    config.memory_dir.join("daemon.cfg")
}

fn read_pid(config: &Config) -> Option<u32> {
    let path = pid_file(config);
    let contents = fs::read_to_string(&path).ok()?;
    contents.trim().parse().ok()
}

#[cfg(unix)]
fn is_running(pid: u32) -> bool {
    // Send signal 0 — checks existence without killing
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(not(unix))]
fn is_running(_pid: u32) -> bool {
    false
}

#[derive(Serialize, Deserialize)]
struct DaemonCfg {
    interval: u64,
    provider: Option<String>,
}

fn write_daemon_cfg(config: &Config, interval: u64, provider: Option<&str>) -> Result<()> {
    let cfg = DaemonCfg {
        interval,
        provider: provider.map(|s| s.to_string()),
    };
    let json = serde_json::to_string(&cfg)
        .map_err(|e| MemoryError::Io(std::io::Error::other(e.to_string())))?;
    fs::write(cfg_file(config), json).map_err(MemoryError::Io)
}

fn read_daemon_cfg(config: &Config) -> Option<DaemonCfg> {
    let contents = fs::read_to_string(cfg_file(config)).ok()?;
    serde_json::from_str(&contents).ok()
}

/// Rotate log file: if > max_lines, keep only the last keep_lines lines.
fn rotate_log_if_needed(log_path: &PathBuf, max_lines: usize, keep_lines: usize) {
    let contents = match fs::read_to_string(log_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let lines: Vec<&str> = contents.lines().collect();
    if lines.len() > max_lines {
        let start = lines.len().saturating_sub(keep_lines);
        let truncated = lines[start..].join("\n") + "\n";
        let _ = fs::write(log_path, truncated);
    }
}

/// Count active (non-expired) knowledge blocks across all 6 category files for a project.
fn count_active_blocks(knowledge_dir: &std::path::Path) -> usize {
    use crate::extractor::knowledge::{parse_session_blocks, partition_by_expiry};
    const CATEGORIES: &[&str] = &[
        "decisions.md",
        "solutions.md",
        "patterns.md",
        "bugs.md",
        "insights.md",
        "questions.md",
    ];
    let mut count = 0;
    for cat in CATEGORIES {
        if let Ok(content) = fs::read_to_string(knowledge_dir.join(cat)) {
            let (_, blocks) = parse_session_blocks(&content);
            let (active, _) = partition_by_expiry(blocks);
            count += active.len();
        }
    }
    count
}

pub fn cmd_daemon_start(config: &Config, interval: u64, provider: Option<&str>) -> Result<()> {
    // Check if already running
    if let Some(pid) = read_pid(config) {
        if is_running(pid) {
            println!(
                "{} Daemon already running (PID {})",
                "engram:".cyan().bold(),
                pid
            );
            return Ok(());
        }
    }

    let log_path = log_file(config);

    let mut cmd = Command::new("engram");
    cmd.arg("daemon").arg("run");
    cmd.arg("--interval").arg(interval.to_string());
    if let Some(p) = provider {
        cmd.arg("--provider").arg(p);
    }

    // Detach: redirect stdout/stderr to log file, no stdin
    let log_file_handle = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(MemoryError::Io)?;

    let child = cmd
        .stdin(Stdio::null())
        .stdout(log_file_handle.try_clone().map_err(MemoryError::Io)?)
        .stderr(log_file_handle)
        .spawn()
        .map_err(MemoryError::Io)?;

    let pid = child.id();

    // Write PID file
    fs::write(pid_file(config), pid.to_string()).map_err(MemoryError::Io)?;

    // Persist config so status/TUI can read it back
    write_daemon_cfg(config, interval, provider)?;

    // Detach from child (don't wait)
    drop(child);

    // Verify the daemon actually started
    thread::sleep(Duration::from_millis(500));
    if !is_running(pid) {
        let _ = fs::remove_file(pid_file(config));
        return Err(MemoryError::Io(std::io::Error::other(format!(
            "Daemon failed to start (PID {} no longer running). Check logs: {}",
            pid,
            log_path.display()
        ))));
    }

    println!("{} Daemon started (PID {})", "engram:".cyan().bold(), pid);
    println!("  Interval: every {} minutes", interval);
    println!("  Logs:     {}", log_path.display());
    println!("  Stop:     {}", "engram daemon stop".yellow());

    Ok(())
}

pub fn cmd_daemon_stop(config: &Config) -> Result<()> {
    let pid = match read_pid(config) {
        Some(p) => p,
        None => {
            println!("{} Daemon is not running", "engram:".cyan().bold());
            return Ok(());
        }
    };

    if !is_running(pid) {
        println!(
            "{} Daemon was not running (stale PID {})",
            "engram:".cyan().bold(),
            pid
        );
        let _ = fs::remove_file(pid_file(config));
        return Ok(());
    }

    // Kill the entire process group (daemon + any running ingest child)
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), libc::SIGTERM);
    };
    #[cfg(not(unix))]
    {
        // On non-Unix, kill by PID directly (no process group support)
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output();
    }

    // Wait up to 5s for it to exit
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if !is_running(pid) {
            break;
        }
    }

    #[cfg(unix)]
    if is_running(pid) {
        unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
    }

    let _ = fs::remove_file(pid_file(config));
    println!("{} Daemon stopped (PID {})", "engram:".cyan().bold(), pid);
    Ok(())
}

pub fn cmd_daemon_status(config: &Config) -> Result<()> {
    let cfg = read_daemon_cfg(config);

    match read_pid(config) {
        Some(pid) if is_running(pid) => {
            println!(
                "{} {} (PID {})",
                "engram daemon:".cyan().bold(),
                "running".green().bold(),
                pid
            );
            if let Some(c) = &cfg {
                println!("  Interval: every {} minutes", c.interval);
                if let Some(p) = &c.provider {
                    println!("  Provider: {}", p);
                }
            }
            println!("  Logs: {}", log_file(config).display());
        }
        Some(pid) => {
            println!(
                "{} {} (stale PID {})",
                "engram daemon:".cyan().bold(),
                "stopped".yellow(),
                pid
            );
            let _ = fs::remove_file(pid_file(config));
        }
        None => {
            println!(
                "{} {}",
                "engram daemon:".cyan().bold(),
                "not running".yellow()
            );
            println!("  Start: {}", "engram daemon start".green());
        }
    }
    Ok(())
}

pub fn cmd_daemon_logs(config: &Config, lines: usize, follow: bool) -> Result<()> {
    let log_path = log_file(config);

    if !log_path.exists() {
        println!(
            "{} No log file found. Has the daemon been started?",
            "engram:".yellow()
        );
        return Ok(());
    }

    if follow {
        // Tail -f style
        let file = fs::File::open(&log_path).map_err(MemoryError::Io)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();

        // Seek to end - last N lines
        let all_lines: Vec<String> =
            BufReader::new(fs::File::open(&log_path).map_err(MemoryError::Io)?)
                .lines()
                .map_while(|l| l.ok())
                .collect();

        let start = all_lines.len().saturating_sub(lines);
        for l in &all_lines[start..] {
            println!("{}", l);
        }

        // Now follow
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => thread::sleep(Duration::from_millis(500)),
                Ok(_) => print!("{}", line),
                Err(e) => return Err(MemoryError::Io(e)),
            }
        }
    } else {
        let all_lines: Vec<String> =
            BufReader::new(fs::File::open(&log_path).map_err(MemoryError::Io)?)
                .lines()
                .map_while(|l| l.ok())
                .collect();

        let start = all_lines.len().saturating_sub(lines);
        for l in &all_lines[start..] {
            println!("{}", l);
        }
    }

    Ok(())
}

/// Check if engram hooks are registered in settings.json; reinstall if missing.
fn heal_hooks_if_needed(log: &dyn Fn(&str)) {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let settings_path = home.join(".claude").join("settings.json");
    let registered = settings_path
        .exists()
        .then(|| std::fs::read_to_string(&settings_path).ok())
        .flatten()
        .map(|c| c.contains("engram"))
        .unwrap_or(false);

    if !registered {
        log("hooks drift detected — reinstalling engram hooks");
        let status = Command::new("engram").args(["hooks", "install"]).status();
        match status {
            Ok(s) if s.success() => log("  hooks install — ok"),
            Ok(_) => log("  hooks install — exited with error"),
            Err(e) => log(&format!("  hooks install — failed to spawn: {}", e)),
        }
    }
}

/// The actual long-running daemon loop — called internally via `engram daemon run`
pub fn cmd_daemon_run(config: &Config, interval_mins: u64, provider: Option<&str>) -> Result<()> {
    use chrono::Local;

    let log = |msg: &str| {
        println!("[{}] {}", Local::now().format("%Y-%m-%d %H:%M:%S"), msg);
    };

    // Create a new process group so SIGTERM to -pgid kills daemon + children
    #[cfg(unix)]
    unsafe {
        libc::setpgid(0, 0);
    }

    log("Engram daemon started");
    log(&format!("  Interval: {} minutes", interval_mins));
    log(&format!("  Memory dir: {}", config.memory_dir.display()));

    let interval = Duration::from_secs(interval_mins * 60);
    let timeout = Duration::from_secs(7200); // 2 hours

    // Write our own PID (in case start didn't, e.g. direct invocation)
    let pid = std::process::id();
    let _ = fs::write(pid_file(config), pid.to_string());

    let log_path = log_file(config);

    loop {
        heal_hooks_if_needed(&log);
        log("Running ingest...");

        let mut cmd = Command::new("engram");
        cmd.arg("ingest");
        if let Some(p) = provider {
            cmd.arg("--provider").arg(p);
        }

        match cmd.spawn() {
            Ok(mut child) => {
                let start = std::time::Instant::now();
                let exit_status = loop {
                    match child.try_wait() {
                        Ok(Some(status)) => break Some(status),
                        Ok(None) => {
                            if start.elapsed() >= timeout {
                                log("Ingest timed out after 2 hours — killing child process");
                                let _ = child.kill();
                                break None;
                            }
                            thread::sleep(Duration::from_secs(1));
                        }
                        Err(e) => {
                            log(&format!("Error waiting for ingest: {}", e));
                            break None;
                        }
                    }
                };

                let ingest_ok = matches!(exit_status, Some(ref s) if s.success());
                match exit_status {
                    Some(s) if s.success() => log("Ingest complete"),
                    Some(_) => log("Ingest exited with error"),
                    None => log("Ingest killed (timeout or wait error)"),
                }

                // After a successful ingest, refresh MEMORY.md for every known project
                if ingest_ok {
                    let knowledge_dir = config.memory_dir.join("knowledge");
                    let projects: Vec<String> = fs::read_dir(&knowledge_dir)
                        .into_iter()
                        .flatten()
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .filter(|name| name != crate::config::GLOBAL_DIR)
                        .collect();

                    if projects.is_empty() {
                        log("Inject: no projects found, skipping");
                    } else {
                        log(&format!("Injecting {} project(s)...", projects.len()));
                        for project in &projects {
                            let mut inject_cmd = Command::new("engram");
                            inject_cmd.arg("inject").arg(project);
                            match inject_cmd.output() {
                                Ok(out) if out.status.success() => {
                                    log(&format!("  inject {} — ok", project));
                                }
                                Ok(out) => {
                                    let stderr =
                                        String::from_utf8_lossy(&out.stderr).trim().to_string();
                                    log(&format!("  inject {} — error: {}", project, stderr));
                                }
                                Err(e) => {
                                    log(&format!("  inject {} — failed to spawn: {}", project, e));
                                }
                            }
                        }
                    }

                    // Distillation: prune projects that have accumulated too many blocks
                    let distill_threshold = crate::config::DISTILL_THRESHOLD;
                    let distill_stale_days = crate::config::DISTILL_STALE_DAYS;
                    for project in &projects {
                        let knowledge_dir = config.memory_dir.join("knowledge").join(project);
                        let active_count = count_active_blocks(&knowledge_dir);
                        if active_count > distill_threshold {
                            log(&format!(
                                "  distill {} ({} blocks > {} threshold)",
                                project, active_count, distill_threshold
                            ));
                            let mut forget_cmd = Command::new("engram");
                            forget_cmd.args([
                                "forget",
                                project,
                                "--stale",
                                &format!("{}d", distill_stale_days),
                                "--auto",
                            ]);
                            match forget_cmd.output() {
                                Ok(out) if out.status.success() => {
                                    log(&format!("    forget --stale {}d — ok", distill_stale_days))
                                }
                                Ok(out) => {
                                    let stderr =
                                        String::from_utf8_lossy(&out.stderr).trim().to_string();
                                    log(&format!("    forget --stale — error: {}", stderr));
                                }
                                Err(e) => log(&format!("    forget --stale — spawn failed: {}", e)),
                            }
                            let mut regen_cmd = Command::new("engram");
                            regen_cmd.args(["regen", project]);
                            match regen_cmd.output() {
                                Ok(out) if out.status.success() => {
                                    log(&format!("    regen {} — ok", project))
                                }
                                Ok(out) => {
                                    let stderr =
                                        String::from_utf8_lossy(&out.stderr).trim().to_string();
                                    log(&format!("    regen {} — error: {}", project, stderr));
                                }
                                Err(e) => {
                                    log(&format!("    regen {} — spawn failed: {}", project, e))
                                }
                            }
                        }
                    }

                    // Doctor pass: fix stale context / missing embeddings per project
                    for project in &projects {
                        let mut doctor_cmd = Command::new("engram");
                        doctor_cmd.args(["doctor", project, "--fix"]);
                        match doctor_cmd.output() {
                            Ok(out) if out.status.success() => {
                                log(&format!("  doctor {} — ok", project));
                            }
                            Ok(out) => {
                                let stderr =
                                    String::from_utf8_lossy(&out.stderr).trim().to_string();
                                log(&format!("  doctor {} — error: {}", project, stderr));
                            }
                            Err(e) => {
                                log(&format!("  doctor {} — failed to spawn: {}", project, e));
                            }
                        }
                    }

                    // Quality reflection pass: log memory quality scores per project
                    let mut total_score = 0u32;
                    let mut scored = 0u32;
                    for project in &projects {
                        let project_knowledge_dir =
                            config.memory_dir.join("knowledge").join(project);
                        if let Some(q) = crate::commands::reflect::compute_project_quality(
                            &project_knowledge_dir,
                            project,
                        ) {
                            let label = match q.quality_score {
                                90..=100 => "Excellent",
                                75..=89 => "Good",
                                50..=74 => "Fair",
                                _ => "Poor",
                            };
                            log(&format!(
                                "  quality {} — {}/100 ({}) — {} entries, {}% stale",
                                project, q.quality_score, label, q.total_entries, q.stale_pct
                            ));
                            total_score += q.quality_score as u32;
                            scored += 1;
                        }
                    }
                    if scored > 0 {
                        log(&format!(
                            "  avg quality {}/100 across {} project(s)",
                            total_score / scored,
                            scored
                        ));
                    }
                }
            }
            Err(e) => {
                log(&format!("Failed to run ingest: {}", e));
            }
        }

        // Rotate log if needed (> 5000 lines -> keep last 2500)
        rotate_log_if_needed(&log_path, 5000, 2500);

        log(&format!("Sleeping {} minutes...", interval_mins));
        thread::sleep(interval);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_test_config(dir: &TempDir) -> Config {
        Config {
            memory_dir: dir.path().to_path_buf(),
            claude_projects_dir: dir.path().to_path_buf(),
            llm: crate::auth::providers::ResolvedProvider {
                provider: crate::auth::providers::Provider::Anthropic,
                endpoint: "https://api.anthropic.com".to_string(),
                model: "claude-haiku-4-5-20251001".to_string(),
                api_key: None,
            },
        }
    }

    #[test]
    fn test_read_pid_missing_file() {
        let dir = TempDir::new().unwrap();
        let config = make_test_config(&dir);
        assert_eq!(read_pid(&config), None);
    }

    #[test]
    fn test_read_pid_invalid_content() {
        let dir = TempDir::new().unwrap();
        let config = make_test_config(&dir);
        fs::write(pid_file(&config), "not-a-number").unwrap();
        assert_eq!(read_pid(&config), None);
    }

    #[test]
    fn test_is_running_current_process() {
        // Our own PID is definitely running
        let our_pid = std::process::id();
        assert!(is_running(our_pid));
    }

    #[test]
    fn test_is_running_exited_process() {
        // Spawn a short-lived process, wait for it, then verify it's gone
        let mut child = Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        // After wait(), the process has exited and been reaped — is_running should be false
        assert!(!is_running(pid));
    }

    #[test]
    fn test_log_rotation_truncates_large_log() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("daemon.log");

        // Write 6000 lines
        let mut f = fs::File::create(&log_path).unwrap();
        for i in 0..6000usize {
            writeln!(f, "line {}", i).unwrap();
        }
        drop(f);

        rotate_log_if_needed(&log_path, 5000, 2500);

        let contents = fs::read_to_string(&log_path).unwrap();
        let line_count = contents.lines().count();
        assert!(
            line_count <= 2500,
            "expected <= 2500 lines after rotation, got {}",
            line_count
        );
        // Verify the last lines are preserved (not the first)
        assert!(
            contents.contains("line 5999"),
            "last line should be preserved"
        );
    }

    #[test]
    fn test_log_rotation_skips_small_log() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("daemon.log");

        // Write only 100 lines — below threshold
        let mut f = fs::File::create(&log_path).unwrap();
        for i in 0..100usize {
            writeln!(f, "line {}", i).unwrap();
        }
        drop(f);

        rotate_log_if_needed(&log_path, 5000, 2500);

        let contents = fs::read_to_string(&log_path).unwrap();
        assert_eq!(
            contents.lines().count(),
            100,
            "small log should not be truncated"
        );
    }

    #[test]
    fn test_daemon_cfg_roundtrip() {
        let dir = TempDir::new().unwrap();
        let config = make_test_config(&dir);

        write_daemon_cfg(&config, 30, Some("anthropic")).unwrap();
        let cfg = read_daemon_cfg(&config).unwrap();
        assert_eq!(cfg.interval, 30);
        assert_eq!(cfg.provider.as_deref(), Some("anthropic"));
    }

    #[test]
    fn test_daemon_cfg_no_provider() {
        let dir = TempDir::new().unwrap();
        let config = make_test_config(&dir);

        write_daemon_cfg(&config, 15, None).unwrap();
        let cfg = read_daemon_cfg(&config).unwrap();
        assert_eq!(cfg.interval, 15);
        assert!(cfg.provider.is_none());
    }

    #[test]
    fn test_read_daemon_cfg_missing_returns_none() {
        let dir = TempDir::new().unwrap();
        let config = make_test_config(&dir);
        assert!(read_daemon_cfg(&config).is_none());
    }
}
