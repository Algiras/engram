# TUI Enhancement Plan

## Current State Analysis

### Existing Screens (4)
1. **Browser** - 2-panel view (projects + sessions/knowledge files)
2. **Viewer** - Markdown content display
3. **Packs** - Hive mind pack browser
4. **PackDetail** - Individual pack details

### Existing Features
- Fuzzy search (`/` key)
- Navigation (hjkl, arrows, Tab)
- Pack management (update, uninstall)
- Delete confirmation dialogs
- Vim-style scrolling

## Missing Features Map

### High Priority (Core Visibility)

#### 1. Learning Dashboard (`L` key)
**Why**: Reinforcement learning is a major feature but invisible in TUI
- **Data**: `LearningState` from `src/learning/progress.rs`
- **Functions**: `display_dashboard()` in `src/learning/dashboard.rs`
- **Display**:
  - Health score + trend
  - Avg query time
  - Stale knowledge percentage
  - Storage size
  - Adaptation success rate
  - Convergence status

#### 2. Analytics Viewer (`A` key)
**Why**: Usage insights exist but only accessible via CLI
- **Data**: `UsageEvent` from `src/analytics/tracker.rs`
- **Functions**: `generate_insights()` in `src/analytics/insights.rs`
- **Display**:
  - Total events (last 30d configurable)
  - Most active project
  - Event type distribution
  - Top knowledge (most accessed)
  - Stale knowledge (least accessed)
  - Usage trend graph (ASCII sparkline)

#### 3. Health Check (`H` key)
**Why**: Critical for maintenance, currently CLI-only
- **Data**: `HealthReport` from `src/health.rs`
- **Functions**: `check_project_health()`
- **Display**:
  - Health score 0-100
  - Status: Excellent/Good/Fair/Poor/Critical
  - Issues by severity (Critical/Warning/Info)
  - Auto-fixable issues marked
  - Recommendations list
  - Quick fix button (`F` key)

#### 4. Quick Actions Menu (`?` key)
**Why**: Discoverability - users don't know what commands exist
- **Display**:
  - Keyboard shortcuts reference
  - Available screens
  - Current context actions
  - Version info

### Medium Priority (Power Features)

#### 5. Graph Visualization (`G` key from Browser)
**Why**: Knowledge graph is powerful but hidden
- **Data**: Graph from `src/graph/builder.rs`
- **Functions**: `render_ascii_tree()` in `src/graph/viz.rs`
- **Display**:
  - ASCII tree visualization
  - Root concept selector
  - Depth traversal controls
  - Hub identification
  - Path finder (concept A → concept B)

#### 6. Export Dialog (`E` key from Browser)
**Why**: Export exists but no interactive UI
- **Options**:
  - Format: Markdown / JSON / HTML
  - Include conversations checkbox
  - Output file picker
  - Progress indicator

#### 7. Consolidation Review (`C` key from Browser)
**Why**: Duplicate detection needs review workflow
- **Data**: Duplicate clusters
- **Display**:
  - Similar knowledge pairs
  - Similarity score
  - Side-by-side diff
  - Merge/Keep/Skip actions

### Low Priority (Nice to Have)

#### 8. Diff Viewer (`D` key from Browser)
- Show knowledge evolution over time
- Version selector
- Category filter

#### 9. Semantic Search Mode (`S` key)
- Alternative to fuzzy search
- Uses embeddings
- Shows similarity scores

#### 10. Inbox Review (from Browser)
- Review extracted candidates
- Promote/reject workflow
- TTL editor

## UI Design Principles

### Navigation Model
```
Browser (main) ←→ Viewer
    ↓
Packs ←→ PackDetail
    ↓
[NEW] Learning (L)
[NEW] Analytics (A)  
[NEW] Health (H)
[NEW] Graph (G)
[NEW] Help (?)
```

### Key Bindings
- **Global**: `q` quit, `Esc` back, `?` help
- **Browser**: `p` packs, `L` learning, `A` analytics, `H` health, `G` graph, `E` export, `C` consolidate
- **Viewer**: vim keys (j/k/g/G/space/PageUp/PageDown)
- **All lists**: `/` search, `n`/`N` next/prev match, `r` reload

### Visual Consistency
- **Headers**: Cyan bold with `=` separator
- **Status**: Green ✓, Yellow warning, Red ✗
- **Metrics**: Show current + delta + sparklines
- **Focus**: Yellow highlight for selected item
- **Match**: Magenta background for search hits

## Implementation Plan

### Phase 1: Infrastructure (1-2 hrs)
- [ ] Extend `Screen` enum with new variants
- [ ] Add data structures to `App` struct
- [ ] Create loading functions in `data.rs`

### Phase 2: Learning Dashboard (2-3 hrs)
- [ ] `Screen::Learning` enum variant
- [ ] `render_learning()` in `ui.rs`
- [ ] `handle_learning_keys()` in `mod.rs`
- [ ] Load `LearningState` for current project
- [ ] Display metrics + trends + recommendations

### Phase 3: Analytics Viewer (2-3 hrs)
- [ ] `Screen::Analytics` enum variant
- [ ] `render_analytics()` in `ui.rs`
- [ ] Load events from tracker
- [ ] Generate insights
- [ ] ASCII sparkline for trend
- [ ] Detailed event log (paginated)

### Phase 4: Health Check (2-3 hrs)  
- [ ] `Screen::Health` enum variant
- [ ] `render_health()` in `ui.rs`
- [ ] Load health report
- [ ] Color-coded severity
- [ ] Auto-fix integration (call CLI commands)
- [ ] Real-time progress updates

### Phase 5: Help Menu (1 hr)
- [ ] `Screen::Help` enum variant
- [ ] `render_help()` in `ui.rs`
- [ ] Static keyboard shortcuts table
- [ ] Version + build info
- [ ] Quick tips

### Phase 6: Graph Viz (3-4 hrs)
- [ ] `Screen::Graph` enum variant
- [ ] Integrate ASCII tree renderer
- [ ] Root concept picker
- [ ] Depth controls
- [ ] Path finder UI

### Phase 7: Export/Consolidate (2-3 hrs each)
- [ ] Modal dialogs for actions
- [ ] Form inputs (file picker, threshold slider)
- [ ] Progress indicators
- [ ] Success/error messages

## Data Loading Strategy

### On-Demand vs Cached
- **Browser tree**: Load on startup, cache in `App`
- **Learning state**: Load on `L` key press
- **Analytics events**: Load on `A` key press (configurable days)
- **Health report**: Load on `H` key press (can be slow)
- **Graph**: Load on `G` key press (expensive)

### Refresh Strategy
- `r` key: Reload current screen data
- Auto-refresh: Not needed (memory is append-only)
- Background loading: Not implemented (blocking is fine for TUI)

## Testing Plan

### Manual Test Cases
1. Browser → Learning → check metrics display
2. Browser → Analytics → verify event counts
3. Browser → Health → run auto-fix
4. Pack screen → unaffected by new screens
5. Search → works across all screens
6. Help → shows all new shortcuts

### Integration Points
- Ensure CLI commands still work
- Verify no breaking changes to existing screens
- Test terminal resize handling
- Test with no data (empty projects)

## Future Enhancements

### Interactive Features
- [ ] Live tail mode (follow new sessions)
- [ ] Multi-project view (dashboard of all projects)
- [ ] Inline editing (add/edit knowledge)
- [ ] Conflict resolution (merge mode)

### Performance
- [ ] Lazy loading for large trees
- [ ] Virtual scrolling for long lists
- [ ] Incremental search results

### Integrations
- [ ] Git sync status indicator
- [ ] LLM provider status (auth check)
- [ ] MCP server health

## Success Metrics

### User Experience
- All CLI features accessible in TUI (except MCP server)
- ≤ 2 keypresses to reach any feature from Browser
- Help menu discoverable (`?` key)
- No modal dialogs block workflow (Esc to dismiss)

### Code Quality
- Consistent patterns with existing screens
- No code duplication in rendering logic
- Error handling for all data loading
- Graceful degradation (no panics)

## References
- Current TUI: `/src/tui/`
- Learning: `/src/learning/dashboard.rs`
- Analytics: `/src/analytics/insights.rs`
- Health: `/src/health.rs`
- Graph: `/src/graph/viz.rs`
