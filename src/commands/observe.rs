use crate::error::{self, Result};
use std::io::Read;

/// Read PostToolUse JSON from stdin and append a lightweight observation record.
///
/// Called by the updated engram-hook.sh on every interesting tool use.
/// This is designed to be very fast (no LLM) — it just appends a JSONL record
/// to ~/memory/observations/<project>/YYYY-MM-DD.jsonl.
pub fn cmd_observe(project: Option<&str>) -> Result<()> {
    // Read stdin (PostToolUse hook provides JSON)
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let json: serde_json::Value = match serde_json::from_str(input.trim()) {
        Ok(v) => v,
        Err(_) => {
            // Non-JSON input (e.g. empty) — silently exit
            return Ok(());
        }
    };

    let tool_name = json.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");

    // Only capture interesting tools
    let interesting = matches!(tool_name, "Edit" | "Write" | "Task" | "Bash" | "MultiEdit");
    if !interesting {
        return Ok(());
    }

    // Extract file path from tool_input
    let file_path = json
        .get("tool_input")
        .and_then(|inp| inp.get("file_path").and_then(|v| v.as_str()))
        .or_else(|| {
            json.get("tool_input")
                .and_then(|inp| inp.get("command").and_then(|v| v.as_str()))
        })
        .unwrap_or("")
        .to_string();

    let session_id = json
        .get("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Determine project: passed via --project flag, or from CWD basename
    let project_name = project.map(|s| s.to_string()).unwrap_or_else(|| {
        std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .unwrap_or_else(|| "unknown".to_string())
    });

    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let ts = chrono::Utc::now().to_rfc3339();

    let obs_dir = home.join("memory").join("observations").join(&project_name);
    std::fs::create_dir_all(&obs_dir)?;

    let record = serde_json::json!({
        "ts": ts,
        "tool": tool_name,
        "file": file_path,
        "session": session_id,
    });

    let obs_path = obs_dir.join(format!("{}.jsonl", today));
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&obs_path)?;
    writeln!(f, "{}", record)?;

    Ok(())
}
