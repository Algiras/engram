# TUI Features (v0.3.0)

## Overview

The claude-memory TUI provides an interactive interface for browsing conversations, managing knowledge, and monitoring system health. This document describes all available screens and keyboard shortcuts.

## Screens

### 1. Browser (Main Screen)
**Access**: Default on launch

Two-panel view for browsing your memory:
- **Left panel**: Project list
- **Right panel**: Sessions and knowledge files

**Features**:
- Fuzzy search (`/`)
- Delete conversations (`d`)
- Navigate to other screens
- View conversation details

### 2. Viewer
**Access**: `Enter` on a session/knowledge file

Markdown viewer with vim-style navigation:
- Scrolling (j/k, Page Up/Down, g/G)
- Full content display
- Code syntax highlighting (via markdown)

### 3. Packs Browser
**Access**: `p` from Browser

Browse and manage Hive Mind knowledge packs:
- View installed packs
- Update packs (`u`)
- Uninstall packs (`d`)
- Search packs (`/`)
- View pack details (`Enter`)

### 4. Pack Detail
**Access**: `Enter` from Packs screen

Detailed information about a knowledge pack:
- Metadata (version, author, registry)
- Categories and keywords
- Knowledge statistics
- Content preview

### 5. Learning Dashboard ⭐ NEW
**Access**: `L` from Browser

View reinforcement learning metrics:
- Health score (0-100)
- Average query time
- Stale knowledge percentage
- Storage size
- Adaptation success rate
- Convergence status
- Metrics improvement over time

**Actions**:
- `r` - Reload dashboard

### 6. Analytics Viewer ⭐ NEW
**Access**: `A` from Browser

Usage analytics and insights:
- Total events (configurable days)
- Most active project
- Event type distribution
- Top knowledge (most accessed)
- Stale knowledge (least accessed)
- Usage trend graph
- Recent event log (last 20 events)

**Actions**:
- `+` / `-` - Increase/decrease time window (±7 days)
- `r` - Reload analytics

### 7. Health Check ⭐ NEW
**Access**: `H` from Browser

Project health diagnostics:
- Health score (0-100)
- Status (Excellent/Good/Fair/Poor/Critical)
- Issues by severity:
  - **Critical**: Breaks functionality
  - **Warning**: Degrades performance
  - **Info**: Could be better
- Auto-fixable issues with commands
- Recommendations

**Actions**:
- `r` - Reload health check

### 8. Help ⭐ NEW
**Access**: `?` from anywhere

Keyboard shortcuts reference:
- All available key bindings
- Screen navigation map
- Version information

## Keyboard Shortcuts

### Global
- `q` - Quit application
- `Ctrl+C` - Force quit
- `Esc` - Go back to previous screen
- `?` - Show help

### Browser Screen
**Navigation**:
- `j` / `k` / `↓` / `↑` - Move cursor up/down
- `h` / `l` / `←` / `→` / `Tab` - Switch between panels
- `Enter` - View selected item

**Search**:
- `/` - Enter search mode
- `n` - Next search match
- `N` - Previous search match

**Actions**:
- `d` - Delete selected item (confirms before deletion)
- `p` - Go to Packs screen
- `L` - Go to Learning Dashboard
- `A` - Go to Analytics Viewer
- `H` - Go to Health Check

### Viewer/Detail Screens
**Scrolling**:
- `j` / `k` / `↓` / `↑` - Line by line
- `Space` / `PageDown` - Page down
- `PageUp` - Page up
- `g` / `Home` - Go to top
- `G` / `End` - Go to bottom

**Actions**:
- `q` / `Esc` - Return to previous screen

### Packs Screen
**Navigation**:
- `j` / `k` / `↓` / `↑` - Select pack
- `Enter` - View pack details

**Search**:
- `/` - Search packs
- `n` / `N` - Next/previous match

**Actions**:
- `u` - Update selected pack
- `d` - Uninstall selected pack (requires confirmation)
- `r` - Reload pack list
- `Esc` - Return to Browser

### Analytics Screen
- `+` - Increase time window by 7 days
- `-` - Decrease time window by 7 days
- `r` - Reload analytics data

### Learning/Health Screens
- `r` - Reload data
- Scrolling works same as Viewer

## Data Loading Strategy

### Lazy Loading
Most screens load data only when accessed:
- **Browser**: Loaded on startup, cached in memory
- **Learning**: Loaded when pressing `L`
- **Analytics**: Loaded when pressing `A`
- **Health**: Loaded when pressing `H`

### Refresh
- Press `r` in any data screen to reload
- Browser tree auto-refreshes after deletions
- No auto-refresh (memory is append-only)

## Tips & Tricks

### Efficient Navigation
1. Use `/` to search across projects quickly
2. Press `n` repeatedly to jump through matches
3. Use `L`/`A`/`H` from Browser for instant insights
4. Press `?` anytime if you forget shortcuts

### Search Patterns
- Fuzzy matching: Type partial words
- Case-insensitive: `project` matches `Project`
- Highlights: Yellow background for current project
- Match indicator: Magenta for search hits

### Performance
- Large projects: Search is fast (indexed)
- Learning data: May take 1-2s to load first time
- Analytics: Filter by days to reduce load time
- Health checks: Can be slow for large projects

### Troubleshooting
1. **No data in Learning/Analytics**: Run `claude-memory ingest` first
2. **Empty Health Check**: Project needs knowledge directory
3. **Packs not showing**: Run `claude-memory hive install <pack>`
4. **Search not working**: Ensure directories exist in `~/memory/`

## Architecture Notes

### Screen State Management
Each screen maintains its own state:
- Scroll position preserved within session
- Search state isolated per screen
- Data cached until explicit reload

### Rendering
- **ratatui** v0.29 for TUI framework
- **crossterm** v0.28 for terminal control
- **fuzzy-matcher** for search
- Vim-style keybindings for familiarity

### Integration with CLI
TUI calls the same underlying functions as CLI:
- `learning::dashboard::display_dashboard()`
- `analytics::insights::generate_insights()`
- `health::check_project_health()`
- `hive::PackInstaller` for pack management

## Future Enhancements

### Planned (Not Yet Implemented)
- [ ] Graph Visualization (ASCII tree)
- [ ] Export Dialog (Markdown/JSON/HTML)
- [ ] Consolidation Review (duplicate detection)
- [ ] Diff Viewer (knowledge evolution)
- [ ] Semantic Search (embeddings-based)
- [ ] Inbox Review (promote extracted knowledge)

### Under Consideration
- [ ] Live tail mode (follow new sessions)
- [ ] Multi-project dashboard
- [ ] Inline knowledge editing
- [ ] Git sync status indicator
- [ ] LLM provider status
- [ ] MCP server health monitor

---

**Version**: 0.3.0  
**Documentation**: See [TUI_GUIDE.md](TUI_GUIDE.md) for detailed usage  
**Enhancement Plan**: See [TUI_ENHANCEMENT.md](TUI_ENHANCEMENT.md) for roadmap
