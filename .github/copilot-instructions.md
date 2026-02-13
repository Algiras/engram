# Copilot Instructions for engram

This project is a conversation memory system for Claude Code, archiving sessions, extracting structured knowledge via LLMs, and enabling full-text search and recall of project context.

## Architecture Overview
- **src/**: Main Rust source. Key modules:
  - `extractor/`: Analytics, knowledge extraction, and session parsing
  - `llm/`: LLM client integration and prompt handling
  - `parser/`: Conversation parsing and JSONL handling
  - `renderer/`: Markdown rendering for archival
  - `tui/`: Terminal UI components
  - `auth/`: LLM provider authentication and management
  - `state.rs`, `config.rs`, `error.rs`: Core app state, config, and error handling
- **hooks/**: Shell scripts for session archiving and context injection
- **skills/**: Markdown guides for memory integration

## Key Workflows
- **Build:** Use `cargo build` or `cargo install --path .` for local builds
- **Install:** Run `install.sh` or use the provided curl command
- **Run:** Main commands:
  - `engram ingest [--skip-knowledge]` (archive, extract knowledge)
  - `engram search <query>` (full-text search)
  - `engram recall <project>` (show project context)
  - `engram projects` (list projects)
- **LLM Provider Setup:**
  - `engram auth login [--provider ...]`
  - Credentials stored in `~/.config/engram/auth.json`
  - Environment variables override provider selection

## Project Conventions
- **Knowledge extraction** defaults to Ollama unless Anthropic/OpenAI keys are set
- **Session files**: JSONL format, parsed and archived as markdown
- **Output structure**: See `~/memory/` for conversations, summaries, knowledge, analytics
- **Hooks**: Use `hooks/engram-hook.sh` for PostToolUse automation (see README example)
- **Error handling**: Centralized in `error.rs`, propagate via `Result` and custom error types
- **Config**: Managed in `config.rs`, supports env vars and file-based settings

## Integration Points
- **LLM Providers**: Anthropic, OpenAI, Ollama (local)
- **External scripts**: `hooks/` for session and context automation
- **Environment variables**: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `ENGRAM_LLM_ENDPOINT`, `ENGRAM_LLM_MODEL`

## Examples
- To archive conversations: `engram ingest --skip-knowledge`
- To extract knowledge: `engram ingest`
- To search memory: `engram search "authentication"`
- To recall project context: `engram recall my-project`

## References
- See `README.md` for install, commands, and workflow details
- See `src/` for main logic and module boundaries
- See `hooks/` for shell integration

---
_Review and update these instructions as the project evolves. If any section is unclear or incomplete, please provide feedback for improvement._
