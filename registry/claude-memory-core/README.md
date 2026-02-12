# Claude Memory Core Knowledge Pack

A meta-knowledge pack containing comprehensive information about how claude-memory's knowledge system works.

## What's Inside

This pack teaches you about claude-memory itself:

### Patterns
- Knowledge extraction workflow (ingest → extract → synthesize)
- Four-category knowledge taxonomy (decisions, solutions, patterns, preferences)
- Session block format and TTL management
- Hive Mind distributed knowledge sharing architecture
- Knowledge graph semantic relationships
- Reinforcement learning system

### Solutions
- Fixing missing or stale context.md
- Cleaning up expired knowledge entries
- Resolving "project not found" errors
- Debugging hook execution issues
- Fixing memory injection problems
- Configuring LLM provider authentication
- Troubleshooting registry cloning
- Making installed pack knowledge appear in recall

### Workflows
- Initial setup for new users
- Daily development workflow
- Creating and publishing knowledge packs
- Installing and using community packs
- Managing multiple projects
- Debugging knowledge issues
- Migrating to a new machine

## Installation

```bash
# Add this registry
cd /path/to/claude-memory
claude-memory hive registry add ./registry

# Or from GitHub (once published)
claude-memory hive registry add Algiras/claude-memory

# Install the pack
claude-memory hive install claude-memory-core

# Use the knowledge
claude-memory recall <your-project>
```

## Purpose

This pack serves multiple purposes:

1. **Self-Documentation**: Claude-memory documents itself using its own system
2. **Reference Implementation**: Shows how to structure a knowledge pack
3. **Onboarding**: Helps new users understand the system
4. **Dogfooding**: Tests the hive system with real-world content

## Meta-Knowledge

This is a meta-knowledge pack - it contains knowledge about knowledge management. By installing this pack, you gain understanding of:

- How knowledge extraction works
- How to organize and categorize knowledge
- How to share knowledge via the hive system
- How to troubleshoot common issues
- Best practices and workflows

## Contributing

This pack evolves with the project. To contribute:

1. Identify gaps in the documentation
2. Add new patterns, solutions, or workflows
3. Update existing entries with new insights
4. Submit PR to the claude-memory repository

## License

MIT - Same as claude-memory

## Learn More

- Main Repository: https://github.com/Algiras/claude-memory
- Documentation: See knowledge files in this pack
- Issues: https://github.com/Algiras/claude-memory/issues
