# TUI User Guide

The `claude-memory` TUI provides an interactive interface for browsing and managing your conversation memory.

## Launching

```bash
claude-memory tui
```

## Features

### 🔍 Fuzzy Search (NEW!)

The TUI now includes intelligent fuzzy matching for searches:

- **Flexible matching**: Type partial words, skip letters, make typos
- **Scored results**: Best matches appear first
- **Real-time**: Results update as you type

**Examples:**
- `nmcp` matches "nile-cag" and "MCP"
- `clmem` matches "claude-memory"
- `tuifix` matches "TUI fixes"

### 📊 Three-Screen Layout

1. **Browser** (default): Two-panel view
   - Left panel: Projects list
   - Right panel: Sessions/knowledge files

2. **Viewer**: Full-screen markdown preview

3. **Search Mode**: Interactive fuzzy search

## Keyboard Shortcuts

### Navigation
| Key | Action |
|-----|--------|
| `j` / `↓` | Move down |
| `k` / `↑` | Move up |
| `h` / `←` / `Shift+Tab` | Move to left panel |
| `l` / `→` / `Tab` | Move to right panel |

### Search
| Key | Action |
|-----|--------|
| `/` | Enter search mode |
| `Esc` | Exit search mode |
| `Enter` | Jump to first match and exit search |
| `n` | Jump to next match |
| `N` | Jump to previous match |
| `Backspace` | Delete character |

### Actions
| Key | Action |
|-----|--------|
| `Enter` / `e` | Open viewer for selected item |
| `d` | Delete selected item (with confirmation) |
| `r` | Reload/refresh data |
| `q` / `Ctrl+C` | Quit |

### Viewer Mode
| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down |
| `k` / `↑` | Scroll up |
| `d` / `PageDown` | Scroll down one page |
| `u` / `PageUp` | Scroll up one page |
| `g` / `Home` | Go to top |
| `G` / `End` | Go to bottom |
| `Esc` / `q` | Return to browser |

## Visual Indicators

### Colors
- **Cyan**: Currently selected item
- **Yellow**: Search matches
- **Green**: Project names
- **White**: Regular text
- **Red**: Delete confirmations

### Status Line
Shows search query, match count, and current position:
```
Search: clmem (5 matches, showing 1/5)
```

## Tips

### Efficient Searching
1. Press `/` to enter search mode
2. Start typing a fuzzy pattern (e.g., "nmcp" for "nile-cag MCP")
3. Press `n` to cycle through matches
4. Press `Enter` to jump to current match

### Browsing Large Projects
- Use fuzzy search instead of scrolling
- Example: In a project with 100 sessions, type `/sess23` to jump to session-23

### Managing Memory
- Delete old sessions with `d` key
- Reload after external changes with `r`
- Preview before deleting in viewer mode

## Fuzzy Search Algorithm

The TUI uses the **Skim fuzzy matcher** (same as `fzf`), which:
- Allows typos and partial matches
- Scores matches by relevance
- Handles non-contiguous characters
- Prioritizes word boundaries

**Match Quality:**
- Character distance matters less
- Word boundaries boost score
- Consecutive characters score higher

## Examples

### Search for Recent Sessions
```
/ → type "recent" → n → n → Enter
```

### Find All Knowledge Files
```
/ → type "know" → n to cycle through decisions.md, patterns.md, etc.
```

### Quick Project Switch
```
/ → type first letters of project → Enter
```

## Troubleshooting

### "No memory directory found"
Run `claude-memory ingest` first to populate memory.

### Search not finding expected results
- Check spelling (fuzzy matching helps but has limits)
- Try shorter queries (2-3 chars minimum)
- Use `r` to reload if files changed externally

### TUI looks broken
- Ensure terminal supports colors: `echo $TERM` (should be xterm-256color or similar)
- Try resizing terminal window

### Performance Issues
If the TUI is slow with many projects:
- Use fuzzy search instead of scrolling
- Consider archiving old projects
- Run `claude-memory forget` to clean up old sessions

## Integration with Other Features

### After Deleting
Deleted sessions are moved to system trash. To permanently remove:
```bash
claude-memory forget <project> --purge
```

### Viewing Fresh Content
The viewer shows the markdown as-is. For regenerated context:
```bash
claude-memory regen <project>
```

Then refresh the TUI with `r`.

### Combining with CLI
Use CLI for bulk operations, TUI for browsing:
```bash
# Extract knowledge
claude-memory ingest --provider gemini

# Browse results
claude-memory tui

# Search from CLI if needed
claude-memory search "query"
```

## Advanced Usage

### Custom Keybindings
Currently keybindings are hardcoded. To customize, edit `src/tui/mod.rs` and look for the `handle_*_keys` functions.

### Search Patterns
The fuzzy matcher recognizes:
- **Camel case**: "pD" matches "projectDetails"
- **Word boundaries**: "n-c" matches "nile-cag"
- **Paths**: "src/ui" matches "src/tui/ui.rs"

## Future Improvements

Planned features:
- [ ] Markdown rendering with syntax highlighting
- [ ] Split-pane mode (browse + view simultaneously)
- [ ] Bookmarks/favorites
- [ ] Export selected items
- [ ] Graph view of knowledge relationships

## Feedback

Found a bug or have a suggestion? [Open an issue](https://github.com/Algiras/claude-memory/issues)
