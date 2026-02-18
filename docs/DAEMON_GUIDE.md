# Engram Daemon Guide

The engram daemon runs continuously in the background, automatically ingesting new sessions at a regular interval — no manual `engram ingest` needed.

## Quick Start

```bash
# Start with default 15-minute interval
engram daemon start

# Start with custom interval
engram daemon start --interval 30

# Check status
engram daemon status

# Watch logs live
engram daemon logs --follow

# Stop
engram daemon stop
```

## Commands

### `engram daemon start [OPTIONS]`

Starts the ingest daemon as a detached background process.

| Option | Default | Description |
|--------|---------|-------------|
| `--interval <N>` | `15` | Poll interval in minutes |
| `--provider <NAME>` | system default | LLM provider (`anthropic`, `openai`, `ollama`) |

The daemon:
- Writes its PID to `~/memory/daemon.pid`
- Logs all output to `~/memory/daemon.log`
- Runs `engram ingest` on every interval tick
- Survives terminal closure (detached process)

### `engram daemon stop`

Sends SIGTERM to the daemon process. Waits up to 5 seconds for clean shutdown, then sends SIGKILL if needed. Removes the PID file.

### `engram daemon status`

Shows whether the daemon is running, its PID, and the log file path.

### `engram daemon logs [OPTIONS]`

| Option | Default | Description |
|--------|---------|-------------|
| `-l, --lines <N>` | `50` | Number of recent lines to show |
| `-f, --follow` | off | Stream new log lines as they appear |

## TUI Integration

From the TUI (`engram tui`), press `D` to open the Daemon screen:

| Key | Action |
|-----|--------|
| `s` | Start daemon (uses current interval) |
| `x` | Stop daemon |
| `+` / `-` | Adjust interval (minutes) |
| `r` | Reload status and logs |
| `j` / `k` | Scroll log output |
| `q` / Esc | Return to browser |

The interval shown in the title bar is the value that will be used when pressing `s` to start.

## Files

| File | Purpose |
|------|---------|
| `~/memory/daemon.pid` | PID of running daemon (removed on stop) |
| `~/memory/daemon.log` | All daemon output — ingest runs, errors, timing |

## When to Use the Daemon vs. Hooks

| | Daemon | Session-End Hook |
|---|--------|-----------------|
| **Trigger** | Every N minutes | After each Claude session |
| **Backlog** | Catches all unprocessed sessions | Only new sessions |
| **Setup** | `engram daemon start` | `engram hooks install` |
| **Best for** | Initial catchup, always-on ingestion | Lightweight per-session sync |

Both can run together — the hook keeps things current while the daemon handles any sessions the hook missed.

## Example: First-Time Setup

```bash
# 1. Install session-end hook (future sessions)
engram hooks install

# 2. Start daemon to process the existing backlog (695 sessions etc.)
engram daemon start --interval 10

# 3. Watch it work
engram daemon logs --follow

# 4. Once backlog is done, optionally stop and rely on hooks only
engram daemon stop
```
