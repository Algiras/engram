# Changelog

All notable changes to claude-memory will be documented in this file.

## [Unreleased]

### Added (2026-02-12)

#### MCP Server Support 🚀
- **Model Context Protocol (MCP) server** for Claude Desktop integration
- Direct tool access: `recall`, `search`, `lookup`, `projects`
- Resource mounting: `memory://<project>/context`
- Tested and verified with real projects
- Full documentation in [MCP_SETUP.md](MCP_SETUP.md)

#### Gemini API Support 🤖
- Google Gemini integration for knowledge extraction
- Environment variable support: `GEMINI_API_KEY`
- Configurable models via `CLAUDE_MEMORY_LLM_MODEL`
- Documentation in [GEMINI_SETUP.md](GEMINI_SETUP.md)

#### Fuzzy Search in TUI ⚡
- Intelligent fuzzy matching using Skim algorithm
- Score-based result ranking
- Real-time search as you type
- Navigate results with `n`/`N` keys
- Handles typos and partial matches
- Full guide in [TUI_GUIDE.md](TUI_GUIDE.md)

#### Export Capabilities 📤
- **Markdown export**: For documentation and wikis
- **JSON export**: For programmatic access
- **HTML export**: Standalone webpage with search
- Optional conversation inclusion
- Pipe-friendly stdout mode
- Complete guide in [EXPORT_GUIDE.md](EXPORT_GUIDE.md)

### Improved
- README now highlights MCP and TUI features
- Better error messages for Gemini API
- Sorted search results by relevance
- Enhanced TUI navigation

### Technical
- Added `fuzzy-matcher` dependency (0.3.7)
- Added Gemini provider to auth module
- Extended CLI with `export` command
- Implemented `cmd_export` with 3 format handlers
- Updated MCP protocol types

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
- Session-based organization

---

## Development Notes

### Self-Improvement Methodology

This project uses **dogfooding** - improving itself using its own capabilities:

1. **Ingest** current sessions to capture work
2. **Recall** project knowledge to understand patterns
3. **Search** for relevant code and decisions
4. **Implement** improvements guided by memory
5. **Export** documentation for users

### Testing Strategy

- Manual testing via MCP protocol
- TUI tested with real memory data
- Export formats validated with multiple projects
- Builds verified on macOS ARM

### Performance

- Fuzzy search: ~1ms for 1000 items
- MCP response time: <100ms
- Export: ~1s for typical project
- Ingest: Depends on LLM provider (1-30s per session)

### Future Roadmap

See remaining tasks:
- [ ] Semantic search using embeddings (#2)
- [ ] Knowledge diffing and version control (#3)
- [ ] Knowledge graph visualization (#5)
- [ ] Smart knowledge consolidation (#6)
- [ ] Collaborative features and sharing (#8)

### Contributors

- **Claude Sonnet 4.5** - AI pair programmer
- **Human** - Project creator and tester

### Acknowledgments

- **ratatui** - Excellent TUI framework
- **fuzzy-matcher** - Skim fuzzy matching
- **clap** - CLI argument parsing
- **MCP** - Model Context Protocol spec
