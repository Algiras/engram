# Changelog

All notable changes to claude-memory will be documented in this file.

## [Unreleased]

## [0.3.5] - 2026-02-19

### Fixed
- **inject: `build_raw_context` and `format_smart_memory` now include bugs/insights/questions** — all 6 knowledge categories are injected into MEMORY.md (previously only decisions/solutions/patterns were visible to Claude)
- **panic: empty category guard in `mcp/server.rs`** — `cat.chars().next().unwrap()` replaced with safe guard for empty strings
- **panic: empty category guard in `cmd_add()`** — same fix applied to `commands/manual.rs`
- **panic: `history.first().unwrap()` in `sync.rs`** — replaced with `.map().unwrap_or("(unknown)")`
- **dedup: `cmd_add()` now replaces instead of appends on duplicate session labels** — matches extractor behaviour via `replace_session_block`
- **Gemini embeddings endpoint changed from `v1beta` to `v1`** — correct stable API endpoint

## [0.3.4] - 2026-02-19

### Added
- **Two-phase reflect** - After storing knowledge, a second LLM call checks new entries against existing ones for contradictions; warnings appended to `reflect` response (advisory only, never blocks storage)
- **`forget --stale --summarize`** - Condenses stale entries into a concise LLM summary block before removing originals instead of deleting outright (MemGPT pattern)
- **Ask screen in TUI** (`A` key) - Interactive RAG Q&A inside the terminal UI: type a question, see the answer, scroll with j/k, clear with C
- **Analytics key changed** from `A` to `N` to free up `A` for the Ask screen

## [0.3.3] - 2026-02-18

### Added (TUI Enhancements)
- **Learning Dashboard** (`L` key) - Interactive view of reinforcement learning metrics, health scores, adaptation success rates, and convergence status
- **Analytics Viewer** (`N` key) - Usage insights with configurable time windows (±7 days), event distribution, top/stale knowledge tracking, and recent event log
- **Health Check** (`H` key) - Project diagnostics with severity-grouped issues, auto-fix commands, and recommendations
- **Help Screen** (`?` key) - Complete keyboard shortcuts reference accessible from any screen
- Lazy loading for all new screens to avoid startup delays
- Consistent vim-style navigation across all screens

### Documentation
- `docs/TUI_ENHANCEMENT.md` - Design document with architecture and future roadmap
- `docs/TUI_FEATURES.md` - Feature reference for all 8 TUI screens
- `docs/TUI_IMPLEMENTATION_SUMMARY.md` - Implementation details
- Updated `docs/TUI_GUIDE.md` and `README.md`

## [0.3.0] - 2026-02-13

### Added

#### Hive Mind: Distributed Knowledge Sharing
- **Knowledge pack system** - Create, publish, install, and share knowledge packs via Git
- **Registry management** - Add Git-based registries (`hive registry add owner/repo`)
- **Pack discovery** - Browse and search packs across registries
- **Pack installation** - Install packs with automatic knowledge integration
- **Pack creation** - Extract and package local knowledge for sharing
- **Pack publishing** - Publish to Git with secret detection and privacy controls
- **HTTPS→SSH fallback** - Automatic SSH retry for private repo cloning
- **Pack health checks** - Doctor command validates installed packs
- **TUI pack browser** - Browse, search, install, and manage packs in TUI
- Full guide in [HIVE_GUIDE.md](docs/HIVE_GUIDE.md)

#### Reinforcement Learning System
- **Q-learning for TTL optimization** - Automatically adjusts knowledge retention
- **Multi-armed bandit for consolidation** - Learns optimal merge strategies
- **Learning signals** - Extracts importance from usage patterns
- **Outcome-based feedback** - Explicit feedback CLI command
- **Learning dashboard** - View metrics, simulate, optimize
- Full guide in [LEARNING_GUIDE.md](docs/LEARNING_GUIDE.md)

#### Knowledge Integration
- `recall` now includes installed pack knowledge
- `lookup` searches across installed packs
- `inject` writes pack knowledge to Claude Code MEMORY.md
- `search` covers installed pack files

### Fixed
- Pack install now correctly copies files from nested registry directories
- Pack discovery scans subdirectories (not just registry root)
- Version tracker test collisions (atomic counter for unique IDs)
- All clippy warnings resolved (tests included)

## [0.2.0] - 2026-02-12

### Added

#### MCP Server Support
- **Model Context Protocol (MCP) server** for Claude Desktop integration
- Direct tool access: `recall`, `search`, `lookup`, `projects`
- Resource mounting: `memory://<project>/context`

#### Gemini API Support
- Google Gemini integration for knowledge extraction
- Environment variable support: `GEMINI_API_KEY`

#### Fuzzy Search in TUI
- Intelligent fuzzy matching using Skim algorithm
- Real-time search as you type

#### Export Capabilities
- Markdown, JSON, and HTML export formats
- Optional conversation inclusion
- Pipe-friendly stdout mode
- Guide in [EXPORT_GUIDE.md](docs/EXPORT_GUIDE.md)

## [0.1.0] - 2026-02-11

### Initial Release

#### Core Features
- Conversation archiving from Claude Code sessions
- Knowledge extraction using LLMs (Anthropic, OpenAI, Ollama)
- Context synthesis and injection
- Full-text regex search
- Interactive TUI for browsing
- Project-based organization

#### Commands
- `ingest`: Parse and extract knowledge
- `search`: Full-text search
- `recall`: Show project context
- `status`: Memory statistics
- `projects`: List all projects
- `tui`: Interactive browser
- `hooks install`: Auto-archival setup
- `auth login`: Provider authentication

#### Architecture
- Multi-provider LLM support
- TTL-based knowledge expiration
- Hook-based Claude Code integration
- Markdown-based storage
