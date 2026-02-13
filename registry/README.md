# Claude Memory Core Registry

Official registry of core knowledge packs for engram.

## Available Packs

### engram-core
Meta-knowledge about the engram system itself.

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
engram hive registry add /path/to/engram/registry
engram hive install engram-core
```

## Using This Registry

### Local Development

```bash
# Add the registry (from repo root)
cd /path/to/engram
engram hive registry add ./registry

# Browse available packs
engram hive browse

# Install the core pack
engram hive install engram-core

# View the knowledge
engram recall <your-project>
```

### From GitHub (Once Published)

```bash
# Add the registry
engram hive registry add Algiras/engram

# Or full URL
engram hive registry add https://github.com/Algiras/engram

# Install packs
engram hive install engram-core
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
5. Test locally: `engram hive install <pack-name>`
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
