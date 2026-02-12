# Claude Memory Core Registry

Official registry of core knowledge packs for claude-memory.

## Available Packs

### claude-memory-core
Meta-knowledge about the claude-memory system itself.

**Categories:** Patterns, Solutions, Workflows
**Version:** 1.0.0

**What's Included:**
- Knowledge extraction and management workflows
- Hive Mind architecture and usage
- Common problems and solutions
- Best practices and patterns
- Complete setup and daily workflows

**Install:**
```bash
claude-memory hive registry add /path/to/claude-memory/registry
claude-memory hive install claude-memory-core
```

## Using This Registry

### Local Development

```bash
# Add the registry (from repo root)
cd /path/to/claude-memory
claude-memory hive registry add ./registry

# Browse available packs
claude-memory hive browse

# Install the core pack
claude-memory hive install claude-memory-core

# View the knowledge
claude-memory recall <your-project>
```

### From GitHub (Once Published)

```bash
# Add the registry
claude-memory hive registry add Algiras/claude-memory

# Or full URL
claude-memory hive registry add https://github.com/Algiras/claude-memory

# Install packs
claude-memory hive install claude-memory-core
```

## Pack Structure

Each pack in this registry follows the standard structure:

```
pack-name/
  .pack/
    manifest.json          # Pack metadata
  knowledge/
    patterns.md           # Reusable patterns
    solutions.md          # Problem-solution pairs
    workflows.md          # Step-by-step workflows
    decisions.md          # (Optional) Architectural decisions
  graph/                  # (Optional) Knowledge graph
    knowledge_graph.json
  README.md              # Pack documentation
```

## Contributing

To add a pack to this registry:

1. Create a new directory with the pack name
2. Add `.pack/manifest.json` with complete metadata
3. Add knowledge files in `knowledge/` directory
4. Add a README.md documenting the pack
5. Test locally: `claude-memory hive install <pack-name>`
6. Submit a PR

### Pack Naming Conventions

- Use lowercase-with-dashes: `rust-patterns`, `typescript-best-practices`
- Be specific: `react-hooks-patterns` not just `react`
- Indicate scope: `backend-security`, `frontend-performance`

### Quality Guidelines

- **Clear categories**: Use appropriate PackCategory values
- **Useful keywords**: Help users discover your pack
- **Session blocks**: Use proper format with timestamps and TTL
- **Practical content**: Focus on actionable knowledge
- **No secrets**: Run secret detection before committing

## License

MIT - All packs in this registry are MIT licensed unless otherwise specified in their manifest.
