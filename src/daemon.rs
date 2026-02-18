use std::fs;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use colored::Colorize;

use crate::config::Config;
use crate::error::{MemoryError, Result};

fn pid_file(config: &Config) -> PathBuf {
    config.memory_dir.join("daemon.pid")
}

fn log_file(config: &Config) -> PathBuf {
    config.memory_dir.join("daemon.log")
}

fn read_pid(config: &Config) -> Option<u32> {
    let path = pid_file(config);
    let contents = fs::read_to_string(&path).ok()?;
    contents.trim().parse().ok()
}

fn is_running(pid: u32) -> bool {
    // Send signal 0 — checks existence without killing
    unsafe { libc::kill(pid as i32, 0) == 0 }
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
    let engram_bin = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("engram"));

    let mut cmd = Command::new(&engram_bin);
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
        .map_err(|e| MemoryError::Io(e))?;

    let child = cmd
        .stdin(Stdio::null())
        .stdout(log_file_handle.try_clone().map_err(|e| MemoryError::Io(e))?)
        .stderr(log_file_handle)
        .spawn()
        .map_err(|e| MemoryError::Io(e))?;

    let pid = child.id();

    // Write PID file
    fs::write(pid_file(config), pid.to_string()).map_err(|e| MemoryError::Io(e))?;

    // Detach from child (don't wait)
    drop(child);

    println!(
        "{} Daemon started (PID {})",
        "engram:".cyan().bold(),
        pid
    );
    println!("  Interval: every {} minutes", interval);
    println!("  Logs:     {}", log_path.display());
    println!(
        "  Stop:     {}",
        "engram daemon stop".yellow()
    );

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
        println!("{} Daemon was not running (stale PID {})", "engram:".cyan().bold(), pid);
        let _ = fs::remove_file(pid_file(config));
        return Ok(());
    }

    // Kill the entire process group (daemon + any running ingest child)
    unsafe {
        libc::kill(-(pid as i32), libc::SIGTERM);
    };

    // Wait up to 5s for it to exit
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if !is_running(pid) {
            break;
        }
    }

    if is_running(pid) {
        unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
    }

    let _ = fs::remove_file(pid_file(config));
    println!("{} Daemon stopped (PID {})", "engram:".cyan().bold(), pid);
    Ok(())
}

pub fn cmd_daemon_status(config: &Config) -> Result<()> {
    match read_pid(config) {
        Some(pid) if is_running(pid) => {
            println!(
                "{} {} (PID {})",
                "engram daemon:".cyan().bold(),
                "running".green().bold(),
                pid
            );
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
        println!("{} No log file found. Has the daemon been started?", "engram:".yellow());
        return Ok(());
    }

    if follow {
        // Tail -f style
        let file = fs::File::open(&log_path).map_err(|e| MemoryError::Io(e))?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();

        // Seek to end - last N lines
        let all_lines: Vec<String> = BufReader::new(
            fs::File::open(&log_path).map_err(|e| MemoryError::Io(e))?
        )
        .lines()
        .filter_map(|l| l.ok())
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
        let all_lines: Vec<String> = BufReader::new(
            fs::File::open(&log_path).map_err(|e| MemoryError::Io(e))?
        )
        .lines()
        .filter_map(|l| l.ok())
        .collect();

        let start = all_lines.len().saturating_sub(lines);
        for l in &all_lines[start..] {
            println!("{}", l);
        }
    }

    Ok(())
}

/// The actual long-running daemon loop — called internally via `engram daemon run`
pub fn cmd_daemon_run(config: &Config, interval_mins: u64, provider: Option<&str>) -> Result<()> {
    use chrono::Local;

    let log = |msg: &str| {
        println!("[{}] {}", Local::now().format("%Y-%m-%d %H:%M:%S"), msg);
    };

    // Create a new process group so SIGTERM to -pgid kills daemon + children
    unsafe { libc::setpgid(0, 0) };

    log("Engram daemon started");
    log(&format!("  Interval: {} minutes", interval_mins));
    log(&format!("  Memory dir: {}", config.memory_dir.display()));

    let interval = Duration::from_secs(interval_mins * 60);

    // Write our own PID (in case start didn't, e.g. direct invocation)
    let pid = std::process::id();
    let _ = fs::write(pid_file(config), pid.to_string());

    loop {
        log("Running ingest...");

        let engram_bin = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("engram"));
        let mut cmd = Command::new(&engram_bin);
        cmd.arg("ingest");
        if let Some(p) = provider {
            cmd.arg("--provider").arg(p);
        }

        match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                for line in stdout.lines() {
                    log(&format!("  {}", line));
                }
                for line in stderr.lines() {
                    log(&format!("  [err] {}", line));
                }
                if output.status.success() {
                    log("Ingest complete");
                } else {
                    log("Ingest exited with error");
                }
            }
            Err(e) => {
                log(&format!("Failed to run ingest: {}", e));
            }
        }

        log(&format!("Sleeping {} minutes...", interval_mins));
        thread::sleep(interval);
    }
}
